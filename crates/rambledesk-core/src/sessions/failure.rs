use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentFailureStage {
    Launch,
    Initialize,
    Session,
    Configuration,
    Prompt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentFailureReason {
    Authentication,
    Configuration,
    Model,
    RateLimit,
    Network,
    Connection,
    Unknown,
}

/// A safe diagnosis of the failed operation. Session/configuration/prompt stages
/// establish that ACP initialization succeeded, even if that instance was later closed.
/// Reasons require explicit protocol or local evidence; free-form Agent text is not evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentFailure {
    pub stage: AgentFailureStage,
    pub reason: AgentFailureReason,
    pub message: String,
}

impl AgentFailure {
    pub fn new(
        stage: AgentFailureStage,
        reason: AgentFailureReason,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            reason,
            message: message.into(),
        }
    }
}

impl super::SessionError {
    pub(crate) fn agent_failure(&self, stage: AgentFailureStage) -> Option<AgentFailure> {
        match self {
            Self::Driver(error) => Some(error.failure_at(stage)),
            _ => None,
        }
    }
}
