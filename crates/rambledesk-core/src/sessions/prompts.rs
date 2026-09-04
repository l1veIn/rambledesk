use super::*;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SendManagedPromptInput {
    pub session_id: String,
    pub text: String,
}

#[derive(Default)]
pub(super) struct StreamState {
    pub turn_id: Option<String>,
    last: Option<SessionActivity>,
    tools: HashMap<String, SessionActivity>,
}

pub(super) struct SessionEventCollector {
    pub application: SessionApplication,
    pub session_id: String,
    pub instance_id: String,
}

#[async_trait]
impl AgentSessionObserver for SessionEventCollector {
    async fn observe(&self, event: AgentSessionEvent) -> Result<(), AgentDriverError> {
        self.application
            .record_agent_event(&self.session_id, &self.instance_id, event)
            .await
            .map_err(|_| {
                AgentDriverError::new(
                    "Unable to persist agent activity; the connection was stopped",
                )
            })
    }
}

impl SessionApplication {
    pub async fn send_prompt(
        &self,
        input: SendManagedPromptInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        if input.text.trim().is_empty() || input.text.len() > 1_000_000 {
            return Err(SessionError::InvalidInput);
        }
        self.managed_record(&input.session_id).await?;
        let entry = self.entry(&input.session_id).await;
        let lifecycle = entry.lifecycle.lock().await;
        let mut live = entry.live.lock().await;
        if live.runtime.connection != SessionConnectionState::Connected {
            return Err(SessionError::NotConnected);
        }
        if live.runtime.activity != SessionActivityState::Idle {
            return Err(SessionError::Busy);
        }
        let connection = live.connection.clone().ok_or(SessionError::NotConnected)?;
        let instance = live
            .runtime
            .instance_id
            .clone()
            .ok_or(SessionError::NotConnected)?;
        live.runtime.activity = SessionActivityState::Running;
        live.cancelling = false;
        live.runtime.last_error = None;
        drop(live);
        let turn_id = self.ids.new_id();
        *entry.events.lock().await = StreamState {
            turn_id: Some(turn_id.clone()),
            ..Default::default()
        };
        let saved = async {
            self.append_activity(
                &input.session_id,
                Some(&turn_id),
                SessionActivityKind::UserMessage,
                input.text.clone(),
                None,
            )
            .await?;
            self.append_activity(
                &input.session_id,
                Some(&turn_id),
                SessionActivityKind::Status,
                "Turn started".into(),
                None,
            )
            .await
        }
        .await;
        if let Err(error) = saved {
            let mut live = entry.live.lock().await;
            live.runtime.activity = SessionActivityState::Idle;
            live.runtime.last_error = Some(error.to_string());
            self.changed();
            return Err(error);
        }
        let application = self.clone();
        let session_id = input.session_id.clone();
        tokio::spawn(async move {
            let result = connection.prompt(&input.text).await;
            application
                .finish_prompt(&session_id, &instance, &turn_id, result)
                .await;
        });
        drop(lifecycle);
        self.changed();
        self.get_session(ManagedSessionInput {
            session_id: input.session_id,
        })
        .await
    }

    async fn finish_prompt(
        &self,
        session_id: &str,
        instance: &str,
        turn_id: &str,
        result: Result<String, AgentDriverError>,
    ) {
        let entry = self.entry(session_id).await;
        let _events = entry.events.lock().await;
        let mut live = entry.live.lock().await;
        if live.runtime.instance_id.as_deref() != Some(instance) {
            return;
        }
        let (kind, text) = match &result {
            Ok(reason) => (
                SessionActivityKind::Status,
                format!("Turn finished: {reason}"),
            ),
            Err(error) => (SessionActivityKind::Error, error.to_string()),
        };
        // Keep the live turn busy until its terminal activity is durable.
        let persisted = self
            .append_activity(session_id, Some(turn_id), kind, text, None)
            .await;
        live.runtime.activity = SessionActivityState::Idle;
        live.permissions.clear();
        live.cancelling = false;
        if let Err(error) = result {
            live.runtime.last_error = Some(error.to_string());
        }
        if let Err(error) = persisted {
            live.runtime.last_error = Some(error.to_string());
        }
        if live
            .connection
            .as_ref()
            .is_some_and(|connection| connection.is_closed())
        {
            live.runtime.connection = SessionConnectionState::Disconnected;
        }
        drop(live);
        self.changed();
    }

    async fn record_agent_event(
        &self,
        session_id: &str,
        instance: &str,
        event: AgentSessionEvent,
    ) -> Result<(), SessionError> {
        let entry = self.entry(session_id).await;
        let mut stream = entry.events.lock().await;
        let live = entry.live.lock().await;
        // Backend load can replay old updates. Our durable activity is authoritative;
        // startup replay and late traffic from a replaced instance are not new turns.
        if live.runtime.instance_id.as_deref() != Some(instance)
            || live.runtime.connection != SessionConnectionState::Connected
        {
            return Ok(());
        }
        drop(live);
        let (kind, text, tool_call_id, append) = match event {
            AgentSessionEvent::Activity {
                kind,
                text,
                tool_call_id,
                append,
            } => (kind, text, tool_call_id, append),
            AgentSessionEvent::PermissionRequested(permission) => {
                if permission.session_id != session_id {
                    return Err(SessionError::InvalidInput);
                }
                let mut live = entry.live.lock().await;
                if live.cancelling || live.runtime.activity == SessionActivityState::Idle {
                    let connection = live.connection.clone();
                    drop(live);
                    if let Some(connection) = connection {
                        let _ = connection
                            .respond_permission(&permission.request_id, None)
                            .await;
                    }
                    return Ok(());
                }
                if !live
                    .permissions
                    .iter()
                    .any(|pending| pending.request_id == permission.request_id)
                {
                    live.permissions.push(permission.clone());
                }
                live.runtime.activity = SessionActivityState::WaitingPermission;
                drop(live);
                self.append_activity(
                    session_id,
                    stream.turn_id.as_deref(),
                    SessionActivityKind::Status,
                    format!("Permission required: {}", permission.title),
                    None,
                )
                .await?;
                self.session_changed(session_id);
                return Ok(());
            }
        };
        if text.is_empty() {
            return Ok(());
        }
        let existing = if let Some(tool) = &tool_call_id {
            stream.tools.get(tool).cloned()
        } else {
            stream
                .last
                .as_ref()
                .filter(|row| row.kind == kind && row.tool_call_id.is_none())
                .cloned()
        };
        let row = if let Some(mut row) = existing {
            if append {
                if tool_call_id.is_some() {
                    row.text.push('\n');
                }
                row.text.push_str(&text);
            } else {
                row.text = text;
            }
            if row.text.len() > 1_000_000 {
                return Err(SessionError::InvalidInput);
            }
            self.activities
                .update_activity_text(&row.id, session_id, &row.text)
                .await?
        } else {
            self.append_activity(
                session_id,
                stream.turn_id.as_deref(),
                kind,
                text,
                tool_call_id.clone(),
            )
            .await?
        };
        if let Some(tool) = tool_call_id {
            stream.tools.insert(tool, row);
            stream.last = None;
        } else {
            stream.last = Some(row);
        }
        self.session_changed(session_id);
        Ok(())
    }

    pub(super) async fn append_activity(
        &self,
        session_id: &str,
        turn: Option<&str>,
        kind: SessionActivityKind,
        text: String,
        tool_call_id: Option<String>,
    ) -> Result<SessionActivity, SessionError> {
        Ok(self
            .activities
            .append_activity(NewSessionActivity {
                id: self.ids.new_id(),
                session_id: session_id.into(),
                turn_id: turn.map(Into::into),
                kind,
                text,
                tool_call_id,
                created_at: self.clock.now_rfc3339(),
            })
            .await?)
    }
}
