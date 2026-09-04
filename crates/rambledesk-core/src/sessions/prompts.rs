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
        self.dispatch_prompt(input, None).await
    }

    pub(super) async fn dispatch_prompt(
        &self,
        input: SendManagedPromptInput,
        delivery: Option<FeedbackDelivery>,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        if input.text.trim().is_empty() || input.text.len() > 1_000_000 {
            return Err(SessionError::InvalidInput);
        }
        self.managed_record(&input.session_id).await?;
        let entry = self.entry(&input.session_id).await;
        let lifecycle = entry.lifecycle.lock().await;
        self.require_workable(&input.session_id).await?;
        if self.closing.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(SessionError::ShuttingDown);
        }
        let mut live = entry.live.lock().await;
        if live.runtime.connection != SessionConnectionState::Connected {
            return Err(SessionError::NotConnected);
        }
        if live
            .connection
            .as_ref()
            .is_some_and(|connection| connection.is_closed())
        {
            drop(live);
            self.retire_entry_locked(
                &input.session_id,
                &entry,
                SessionRunEnd::Interrupted,
                Some("Agent connection closed before the prompt was sent"),
            )
            .await?;
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
        let delivery = if let Some(delivery) = delivery {
            let attempt = self.ids.new_id();
            if self
                .deliveries
                .as_ref()
                .ok_or(SessionError::InvalidInput)?
                .claim_delivery(&delivery.request_id, &attempt, &self.clock.now_rfc3339())
                .await?
                .is_none()
            {
                return Err(SessionError::Busy);
            }
            Some((delivery.request_id, attempt))
        } else {
            None
        };
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
            self.begin_turn(&input.session_id, &instance, &turn_id)
                .await?;
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
            drop(live);
            if let (Some(repository), Some((request, attempt))) = (&self.deliveries, &delivery) {
                // No protocol prompt was sent: returning to pending is safe.
                let _ = repository
                    .finish_delivery(
                        request,
                        attempt,
                        FeedbackDeliveryState::Pending,
                        Some("Unable to persist continuation before sending"),
                        &self.clock.now_rfc3339(),
                    )
                    .await;
            }
            let _ = self
                .retire_entry_locked(
                    &input.session_id,
                    &entry,
                    SessionRunEnd::Interrupted,
                    Some("Unable to persist the turn before sending"),
                )
                .await;
            self.changed();
            return Err(error);
        }
        let application = self.clone();
        let session_id = input.session_id.clone();
        tokio::spawn(async move {
            let result = connection.prompt(&input.text).await;
            application
                .finish_prompt(&session_id, &instance, &turn_id, delivery, result)
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
        delivery: Option<(String, String)>,
        result: Result<String, AgentDriverError>,
    ) {
        let Some(entry) = self.entries.lock().await.get(session_id).cloned() else {
            return;
        };
        let mut events = entry.events.lock().await;
        let mut live = entry.live.lock().await;
        let same_instance = live.runtime.instance_id.as_deref() == Some(instance);
        // Persist the attempt outcome even if stop/restart replaced the live entry.
        let delivered = self.finish_feedback_delivery(delivery, &result).await;
        if !same_instance || live.runtime.connection != SessionConnectionState::Connected {
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
        let connection_closed = live
            .connection
            .as_ref()
            .is_some_and(|connection| connection.is_closed());
        let checkpoint = if persisted.is_ok() && !connection_closed {
            self.finish_turn(session_id, instance, turn_id).await
        } else {
            Ok(())
        };
        let interrupted = persisted.is_err() || checkpoint.is_err() || connection_closed;
        if !interrupted {
            events.turn_id = None;
        } else {
            // A lifecycle owner may prevent immediate retirement below. Keep
            // the instance unavailable until the background owner can retry it.
            live.runtime.connection = SessionConnectionState::Disconnected;
        }
        live.runtime.activity = SessionActivityState::Idle;
        live.permissions.clear();
        live.cancelling = false;
        if let Err(error) = result {
            live.runtime.last_error = Some(error.to_string());
        }
        if let Err(error) = persisted {
            live.runtime.last_error = Some(error.to_string());
        }
        if let Err(error) = delivered {
            live.runtime.last_error = Some(error.to_string());
        }
        if let Err(error) = checkpoint {
            live.runtime.last_error = Some(error.to_string());
        }
        drop(live);
        drop(events);
        if interrupted && let Ok(_lifecycle) = entry.lifecycle.try_lock() {
            let current = entry.live.lock().await.runtime.instance_id.as_deref() == Some(instance);
            if current {
                let _ = self
                    .retire_entry_locked(
                        session_id,
                        &entry,
                        SessionRunEnd::Interrupted,
                        Some("Agent turn was interrupted before durable completion"),
                    )
                    .await;
            }
        }
        self.changed();
        self.delivery_wake.notify_one();
    }

    async fn record_agent_event(
        &self,
        session_id: &str,
        instance: &str,
        event: AgentSessionEvent,
    ) -> Result<(), SessionError> {
        let Some(entry) = self.entries.lock().await.get(session_id).cloned() else {
            return Ok(());
        };
        let mut stream = entry.events.lock().await;
        let live = entry.live.lock().await;
        // Backend load can replay old updates. Our durable activity is authoritative;
        // startup replay and late traffic from a replaced instance are not new turns.
        if live.runtime.instance_id.as_deref() != Some(instance)
            || live.runtime.connection != SessionConnectionState::Connected
            || stream.turn_id.is_none()
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
