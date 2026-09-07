use super::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use ts_rs::TS;

#[cfg(test)]
#[path = "interaction_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionPermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionInteraction {
    pub request_id: String,
    pub session_id: String,
    pub title: String,
    // Bounded plain text describing the requested operation, when supplied.
    pub details: Option<String>,
    #[serde(flatten)]
    pub kind: SessionInteractionKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionInteractionKind {
    Permission {
        options: Vec<SessionPermissionOption>,
    },
    Question {
        input: SessionInputRequest,
    },
    Plan {
        input: SessionInputRequest,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionInputKind {
    Question,
    Plan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionInputRequest {
    /// Application form schema: a flat object of text, choice, boolean, numeric
    /// or choice-array fields. ACP validates its supported subset before accept;
    /// unknown constraints remain visible but cannot be silently weakened.
    #[ts(type = "Record<string, unknown>")]
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum SessionInputAction {
    Accept,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionInputResponse {
    pub action: SessionInputAction,
    #[ts(type = "Record<string, unknown> | null")]
    pub content: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionInteractionResponse {
    Permission { option_id: Option<String> },
    Question { response: SessionInputResponse },
    Plan { response: SessionInputResponse },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct RespondManagedInteractionInput {
    pub session_id: String,
    pub request_id: String,
    pub response: SessionInteractionResponse,
}

impl SessionInteraction {
    pub fn allows(&self, response: &SessionInteractionResponse) -> bool {
        match (&self.kind, response) {
            (
                SessionInteractionKind::Permission { options },
                SessionInteractionResponse::Permission { option_id },
            ) => option_id
                .as_ref()
                .is_none_or(|id| options.iter().any(|option| option.option_id == *id)),
            (
                SessionInteractionKind::Question { .. },
                SessionInteractionResponse::Question { .. },
            )
            | (SessionInteractionKind::Plan { .. }, SessionInteractionResponse::Plan { .. }) => {
                true
            }
            _ => false,
        }
    }

    pub fn cancel_response(&self) -> SessionInteractionResponse {
        let response = SessionInputResponse {
            action: SessionInputAction::Cancel,
            content: None,
        };
        match self.kind {
            SessionInteractionKind::Permission { .. } => {
                SessionInteractionResponse::Permission { option_id: None }
            }
            SessionInteractionKind::Question { .. } => {
                SessionInteractionResponse::Question { response }
            }
            SessionInteractionKind::Plan { .. } => SessionInteractionResponse::Plan { response },
        }
    }
}

impl SessionApplication {
    pub async fn respond_interaction(
        &self,
        input: RespondManagedInteractionInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        self.require_workable(&input.session_id).await?;
        let entry = self.entry(&input.session_id).await;
        let events = entry.events.lock().await;
        let live = entry.live.lock().await;
        let interaction = live
            .interactions
            .iter()
            .find(|permission| permission.request_id == input.request_id)
            .ok_or(SessionError::InvalidInput)?;
        if !interaction.allows(&input.response) {
            return Err(SessionError::InvalidInput);
        }
        let connection = live.connection.clone().ok_or(SessionError::NotConnected)?;
        let instance = live.runtime.instance_id.clone();
        let turn = events.turn_id.clone();
        drop(live);
        drop(events);
        connection
            .respond_interaction(&input.request_id, input.response)
            .await?;
        let events = entry.events.lock().await;
        let mut live = entry.live.lock().await;
        if live.runtime.instance_id != instance
            || (events.turn_id != turn && events.turn_id.is_some())
        {
            return Err(SessionError::Interrupted);
        }
        // A fast agent may already have completed the answered turn. Its reply
        // still succeeded; only mutate the queue if this is the original turn.
        if events.turn_id == turn {
            live.interactions
                .retain(|pending| pending.request_id != input.request_id);
            if live.interactions.is_empty()
                && live.runtime.activity == SessionActivityState::WaitingInput
            {
                live.runtime.activity = SessionActivityState::Running;
            }
        }
        drop(live);
        drop(events);
        self.session_changed(&input.session_id);
        self.get_session(ManagedSessionInput {
            session_id: input.session_id,
        })
        .await
    }

    pub async fn cancel_prompt(
        &self,
        input: ManagedSessionInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        let trace = crate::agent_operation_trace::AgentOperationTrace::new(
            "session.cancel",
            Some(&input.session_id),
        );
        let result = self.cancel_prompt_inner(input).await;
        trace.result(result, SessionError::diagnostic_code)
    }

    async fn cancel_prompt_inner(
        &self,
        input: ManagedSessionInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        self.managed_record(&input.session_id).await?;
        let entry = self.entry(&input.session_id).await;
        let events = entry.events.lock().await;
        let turn = events.turn_id.clone();
        let mut live = entry.live.lock().await;
        if live.runtime.activity == SessionActivityState::Idle {
            drop(live);
            drop(events);
            return self.get_session(input).await;
        }
        let connection = live.connection.clone().ok_or(SessionError::NotConnected)?;
        let instance = live.runtime.instance_id.clone();
        live.interactions.clear();
        live.cancelling = true;
        live.runtime.activity = SessionActivityState::Running;
        drop(live);
        drop(events);
        connection.cancel().await?;
        self.session_changed(&input.session_id);
        let app = self.clone();
        let session_id = input.session_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if let (Some(instance), Some(turn)) = (instance, turn) {
                let _ = app.stop_if_current(&session_id, &instance, &turn).await;
            }
        });
        self.get_session(input).await
    }
}
