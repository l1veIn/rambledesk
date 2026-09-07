use super::*;
use crate::ApplicationResourceKey;
#[path = "activity_aggregation.rs"]
mod aggregation;
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
    trace: Option<crate::agent_operation_trace::AgentOperationTrace>,
    last: Option<SessionActivity>,
    tools: HashMap<String, SessionActivity>,
}

impl StreamState {
    pub(super) fn release_content(&mut self) {
        // SQLite owns the transcript. Retain only turn attribution while a
        // failed completion/stop waits for recovery; release allocation capacity.
        self.last = None;
        self.tools = HashMap::new();
        if let Some(mut trace) = self.trace.take() {
            trace.finish("interrupted", "runtime_retired");
        }
    }
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

    pub async fn send_prompt_content(
        &self,
        input: SendManagedPromptContentInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        let session_id = input.session_id.clone();
        let blocks = input.into_blocks()?;
        let text = super::prompt_content::prompt_display(&blocks).summary();
        self.dispatch_prompt_inner(
            SendManagedPromptInput { session_id, text },
            None,
            Some(blocks),
        )
        .await
    }

    pub(super) async fn dispatch_prompt(
        &self,
        input: SendManagedPromptInput,
        delivery: Option<FeedbackDelivery>,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        self.dispatch_prompt_inner(input, delivery, None).await
    }

    async fn dispatch_prompt_inner(
        &self,
        input: SendManagedPromptInput,
        delivery: Option<FeedbackDelivery>,
        content: Option<Vec<SessionPromptContent>>,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        let trace = crate::agent_operation_trace::AgentOperationTrace::new(
            "session.send",
            Some(&input.session_id),
        );
        let result = self.dispatch_prompt_traced(input, delivery, content).await;
        trace.result(result, SessionError::diagnostic_code)
    }

    async fn dispatch_prompt_traced(
        &self,
        input: SendManagedPromptInput,
        delivery: Option<FeedbackDelivery>,
        content: Option<Vec<SessionPromptContent>>,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        if input.text.trim().is_empty() || input.text.len() > 1_000_000 {
            return Err(SessionError::InvalidInput);
        }
        let was_prepared = self.managed_record(&input.session_id).await?.is_prepared();
        let entry = self.entry(&input.session_id).await;
        let lifecycle = entry.lifecycle.try_lock().map_err(|_| SessionError::Busy)?;
        self.require_workable(&input.session_id).await?;
        let prepared = self.managed_record(&input.session_id).await?.is_prepared();
        if was_prepared && !prepared {
            return Err(SessionError::Busy);
        }
        if prepared && delivery.is_some() {
            return Err(SessionError::InvalidInput);
        }
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
        if content.as_ref().is_some_and(|blocks| {
            !prompt_content_supported(blocks, &live.runtime.capabilities.prompt)
        }) {
            return Err(SessionError::InvalidInput);
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
        let turn_trace =
            crate::agent_operation_trace::AgentOperationTrace::new("session.turn", Some(&turn_id));
        turn_trace.link("session", &input.session_id);
        turn_trace.link("instance", &instance);
        *entry.events.lock().await = StreamState {
            turn_id: Some(turn_id.clone()),
            trace: Some(turn_trace),
            ..Default::default()
        };
        let saved = async {
            self.begin_turn(&input.session_id, &instance, &turn_id)
                .await?;
            let now = self.clock.now_rfc3339();
            let user = NewSessionActivity {
                id: self.ids.new_id(),
                session_id: input.session_id.clone(),
                turn_id: Some(turn_id.clone()),
                kind: SessionActivityKind::UserMessage,
                text: input.text.clone(),
                content: content
                    .as_ref()
                    .map(|blocks| super::prompt_content::prompt_display(blocks)),
                tool_call_id: None,
                created_at: now.clone(),
            };
            let started = NewSessionActivity {
                id: self.ids.new_id(),
                session_id: input.session_id.clone(),
                turn_id: Some(turn_id.clone()),
                kind: SessionActivityKind::Status,
                text: "Turn started".into(),
                content: None,
                tool_call_id: None,
                created_at: now,
            };
            if prepared {
                self.repository
                    .promote_prepared_session(user, started, &first_prompt_title(&input.text))
                    .await?;
            } else {
                self.activities.append_activity(user).await?;
                self.activities.append_activity(started).await?;
            }
            Ok::<_, SessionError>(())
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
            self.session_changed(&input.session_id);
            return Err(error);
        }
        let application = self.clone();
        let session_id = input.session_id.clone();
        tokio::spawn(async move {
            let result = match content {
                Some(blocks) => connection.prompt_content(&blocks).await,
                None => connection.prompt(&input.text).await,
            };
            application
                .finish_prompt(&session_id, &instance, &turn_id, delivery, result)
                .await;
        });
        drop(lifecycle);
        self.changed(vec![
            ApplicationResourceKey::Navigation,
            ApplicationResourceKey::ManagedSession {
                session_id: input.session_id.clone(),
            },
        ]);
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
        if let Some(trace) = &mut events.trace {
            let error_code = if persisted.is_err() {
                "activity_storage"
            } else if checkpoint.is_err() {
                "checkpoint_storage"
            } else if connection_closed {
                "closed"
            } else if result.is_err() {
                "driver"
            } else if delivered.is_err() {
                "feedback_delivery"
            } else {
                ""
            };
            trace.finish(
                if !error_code.is_empty() {
                    "failed"
                } else if live.cancelling {
                    "cancelled"
                } else {
                    "succeeded"
                },
                error_code,
            );
        }
        if !interrupted {
            events.turn_id = None;
        } else {
            // A lifecycle owner may prevent immediate retirement below. Keep
            // the instance unavailable until the background owner can retry it.
            live.runtime.connection = SessionConnectionState::Disconnected;
        }
        events.release_content();
        live.runtime.activity = SessionActivityState::Idle;
        let pending = std::mem::take(&mut live.interactions);
        let pending_connection = live.connection.clone();
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
        // Retire requests accepted after the driver's prompt response but before
        // this durable completion. Keep the event/turn gate until replies are
        // released so a later turn cannot reuse a request ID during this drain.
        if let Some(connection) = pending_connection {
            for interaction in pending {
                let _ = connection
                    .respond_interaction(&interaction.request_id, interaction.cancel_response())
                    .await;
            }
        }
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
        self.changed(vec![
            ApplicationResourceKey::Navigation,
            ApplicationResourceKey::ManagedSession {
                session_id: session_id.into(),
            },
        ]);
        self.delivery_wake.notify_one();
    }

    async fn record_agent_event(
        &self,
        session_id: &str,
        instance: &str,
        event: AgentSessionEvent,
    ) -> Result<(), SessionError> {
        let Some(entry) = self.entries.lock().await.get(session_id).cloned() else {
            return if matches!(event, AgentSessionEvent::InteractionRequested(_)) {
                Err(SessionError::InvalidInput)
            } else {
                Ok(())
            };
        };
        if let AgentSessionEvent::ContextUsage(usage) = event {
            let mut live = entry.live.lock().await;
            if live.runtime.instance_id.as_deref() == Some(instance)
                && matches!(
                    live.runtime.connection,
                    SessionConnectionState::Connected | SessionConnectionState::Connecting
                )
                && live.runtime.context_usage.as_ref() != Some(&usage)
            {
                live.runtime.context_usage = Some(usage);
                drop(live);
                self.session_changed(session_id);
            }
            return Ok(());
        }
        if matches!(event, AgentSessionEvent::ConfigurationChanged) {
            let live = entry.live.lock().await;
            if live.runtime.instance_id.as_deref() == Some(instance)
                && matches!(
                    live.runtime.connection,
                    SessionConnectionState::Connected | SessionConnectionState::Connecting
                )
            {
                drop(live);
                self.session_changed(session_id);
            }
            return Ok(());
        }
        let mut stream = entry.events.lock().await;
        let live = entry.live.lock().await;
        // Backend load can replay old updates. Our durable activity is authoritative;
        // startup replay and late traffic from a replaced instance are not new turns.
        if live.runtime.instance_id.as_deref() != Some(instance)
            || live.runtime.connection != SessionConnectionState::Connected
            || stream.turn_id.is_none()
        {
            // Blocked requests must be rejected by the owning adapter; success
            // would leave a responder parked without a visible card.
            return if matches!(event, AgentSessionEvent::InteractionRequested(_)) {
                Err(SessionError::InvalidInput)
            } else {
                Ok(())
            };
        }
        drop(live);
        if let Some(trace) = &mut stream.trace {
            trace.response();
        }
        let (kind, text, tool_call_id, append) = match event {
            AgentSessionEvent::ConfigurationChanged => {
                unreachable!("handled before turn attribution")
            }
            AgentSessionEvent::ContextUsage(_) => unreachable!("handled before turn attribution"),
            AgentSessionEvent::MessageChunk {
                kind,
                block,
                truncated,
            } => {
                self.record_message_chunk(session_id, &mut stream, kind, block, truncated)
                    .await?;
                self.session_changed(session_id);
                return Ok(());
            }
            AgentSessionEvent::ToolCall {
                tool_call_id,
                patch,
            } => {
                self.record_tool_patch(session_id, &mut stream, tool_call_id, patch)
                    .await?;
                self.session_changed(session_id);
                return Ok(());
            }
            AgentSessionEvent::Activity {
                kind,
                text,
                tool_call_id,
                append,
            } => (kind, text, tool_call_id, append),
            AgentSessionEvent::InteractionRequested(permission) => {
                if permission.session_id != session_id {
                    return Err(SessionError::InvalidInput);
                }
                let mut live = entry.live.lock().await;
                if live.cancelling || live.runtime.activity == SessionActivityState::Idle {
                    let connection = live.connection.clone();
                    drop(live);
                    if let Some(connection) = connection {
                        let _ = connection
                            .respond_interaction(
                                &permission.request_id,
                                permission.cancel_response(),
                            )
                            .await;
                    }
                    return Ok(());
                }
                if !live
                    .interactions
                    .iter()
                    .any(|pending| pending.request_id == permission.request_id)
                {
                    live.interactions.push(permission.clone());
                }
                live.runtime.activity = SessionActivityState::WaitingInput;
                drop(live);
                self.append_activity(
                    session_id,
                    stream.turn_id.as_deref(),
                    SessionActivityKind::Status,
                    format!(
                        "{}: {}",
                        match permission.kind {
                            SessionInteractionKind::Permission { .. } => "Permission required",
                            SessionInteractionKind::Question { .. } => "Answer required",
                            SessionInteractionKind::Plan { .. } => "Plan review required",
                        },
                        permission.title
                    ),
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
                content: None,
                tool_call_id,
                created_at: self.clock.now_rfc3339(),
            })
            .await?)
    }
}

fn first_prompt_title(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(80)
        .collect()
}
