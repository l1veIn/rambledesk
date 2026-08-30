use serde::{Deserialize, Serialize};

use super::{
    AgentWorkId, DeliveryId, PackageId, RequestId, SessionId, SubmissionId, WorkClaimToken,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWorkKind {
    LaunchPrompt,
    SteeringPrompt,
    FeedbackResume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWorkState {
    Pending,
    Claimed,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentWorkPayload {
    Launch {
        submission_id: SubmissionId,
        package_id: PackageId,
        prompt_markdown: String,
    },
    Steering {
        submission_id: SubmissionId,
        prompt_markdown: String,
    },
    FeedbackResume {
        delivery_id: DeliveryId,
        request_id: RequestId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWorkRecord {
    pub work_id: AgentWorkId,
    pub session_id: SessionId,
    pub kind: AgentWorkKind,
    pub source_id: String,
    pub payload_digest: String,
    pub payload: AgentWorkPayload,
    pub state: AgentWorkState,
    pub attempt_count: u32,
    pub last_error_code: Option<String>,
    pub last_error_at: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkScope {
    pub session_id: Option<SessionId>,
    pub limit: u32,
    pub lease_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimedAgentWork {
    pub work: AgentWorkRecord,
    pub claim_token: WorkClaimToken,
    pub lease_until: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWorkBatch {
    pub items: Vec<ClaimedAgentWork>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentWorkEvidence {
    PromptTurnCompleted,
    FeedbackConsumedAndTurnCompleted { delivery_id: DeliveryId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "disposition")]
pub enum AgentWorkDisposition {
    Completed { evidence: AgentWorkEvidence },
    Retry { error_code: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWorkResult {
    pub work_id: AgentWorkId,
    pub claim_token: WorkClaimToken,
    pub disposition: AgentWorkDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWorkRecordOutcome {
    pub work_id: AgentWorkId,
    pub state: AgentWorkState,
    pub delivered: Option<DeliveryId>,
}
