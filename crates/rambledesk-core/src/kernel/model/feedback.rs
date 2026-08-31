use serde::{Deserialize, Serialize};

use super::{
    AcpSessionLinkId, AgentWorkId, AgentWorkState, ArtifactInput, DeliveredArtifact, DeliveryId,
    PackageArtifact, PackageId, RequestArtifact, RequestId, SessionId, SubmissionId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackagePurpose {
    Launch,
    Response,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackResolution {
    Submitted,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    Pending,
    Delivered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackRequestStatus {
    Waiting,
    Submitted,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackAction {
    pub id: String,
    pub instruction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextReference {
    pub label: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateFeedbackRequest {
    pub request_id: Option<RequestId>,
    pub session_id: SessionId,
    /// Provenance for a Managed ACP Session request; absent for Imported Sessions.
    pub source_link_id: Option<AcpSessionLinkId>,
    pub title: String,
    pub instructions: String,
    pub actions: Vec<FeedbackAction>,
    pub context_refs: Vec<ContextReference>,
    /// Evidence supplied with the request. These are not Package Artifacts.
    pub artifacts: Vec<ArtifactInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackSubmission {
    pub submission_id: SubmissionId,
    pub request_id: RequestId,
    pub expected_draft_revision: u64,
    pub submission_digest_assertion: Option<String>,
    pub document_json: String,
    pub uncooked_markdown: String,
    pub feedback_markdown: String,
    pub cooking_model: Option<String>,
    pub artifacts: Vec<ArtifactInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelFeedbackRequest {
    pub request_id: RequestId,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "resolution")]
pub enum ResolveFeedbackRequest {
    Submit(FeedbackSubmission),
    Cancel(CancelFeedbackRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageRecord {
    pub package_id: PackageId,
    pub submission_id: SubmissionId,
    pub purpose: PackagePurpose,
    pub request_id: Option<RequestId>,
    /// Digest of immutable Package content. It is not the Submission digest.
    pub content_digest: String,
    /// Digest of the complete manifest projection, including stable identities.
    pub manifest_digest: String,
    pub schema_version: u32,
    pub artifacts: Vec<PackageArtifact>,
    pub published_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackRequestSnapshot {
    pub request_id: RequestId,
    pub session_id: SessionId,
    pub source_link_id: Option<AcpSessionLinkId>,
    pub title: String,
    pub instructions: String,
    pub actions: Vec<FeedbackAction>,
    pub context_refs: Vec<ContextReference>,
    pub input_digest: String,
    pub status: FeedbackRequestStatus,
    pub resolution: Option<FeedbackResolution>,
    pub response_package_id: Option<PackageId>,
    pub cancel_reason: Option<String>,
    pub request_artifacts: Vec<RequestArtifact>,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackDeliveryRecord {
    pub delivery_id: DeliveryId,
    pub request_id: RequestId,
    pub session_id: SessionId,
    pub resolution: FeedbackResolution,
    pub package: Option<PackageRecord>,
    pub cancel_reason: Option<String>,
    pub state: DeliveryState,
    pub attempt_count: u32,
    pub last_error_code: Option<String>,
    pub last_error_at: Option<String>,
    pub created_at: String,
    pub delivered_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetFeedback {
    pub request_id: RequestId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackDeliveryEnvelope {
    pub delivery_id: DeliveryId,
    pub request_id: RequestId,
    pub session_id: SessionId,
    pub resolution: FeedbackResolution,
    pub package_id: Option<PackageId>,
    pub package_content_digest: Option<String>,
    pub package_manifest_digest: Option<String>,
    pub artifacts: Vec<DeliveredArtifact>,
    pub cancel_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetFeedbackOutcome {
    Waiting {
        request_id: RequestId,
        session_id: SessionId,
    },
    Terminal(FeedbackDeliveryEnvelope),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchOutcome {
    pub session_id: SessionId,
    pub submission_id: SubmissionId,
    pub submission_digest: String,
    pub package_id: PackageId,
    pub package_content_digest: String,
    pub package_manifest_digest: String,
    pub agent_work_id: AgentWorkId,
    pub agent_work_state: AgentWorkState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteeringOutcome {
    pub session_id: SessionId,
    pub submission_id: SubmissionId,
    pub submission_digest: String,
    pub agent_work_id: AgentWorkId,
    pub agent_work_state: AgentWorkState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackResolutionOutcome {
    pub request: FeedbackRequestSnapshot,
    pub package_id: Option<PackageId>,
    pub package_content_digest: Option<String>,
    pub package_manifest_digest: Option<String>,
    pub delivery_id: DeliveryId,
    pub delivery_state: DeliveryState,
    pub agent_work_id: AgentWorkId,
}
