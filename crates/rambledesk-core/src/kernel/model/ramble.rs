use serde::{Deserialize, Serialize};

use super::LaunchConfiguration;
use super::{
    ArtifactId, ArtifactInput, DraftArtifact, DraftId, RequestId, SessionId, SubmissionArtifact,
    SubmissionId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RambleIntent {
    Launch,
    Steering,
    Feedback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RambleContent {
    /// Opaque, versioned structured editor document.
    pub document_json: String,
    pub body_markdown: String,
    pub artifacts: Vec<ArtifactInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchSubmission {
    pub submission_id: SubmissionId,
    /// Caller assertion which Core verifies against its canonical projection.
    pub submission_digest_assertion: Option<String>,
    pub title: String,
    pub launch_configuration: LaunchConfiguration,
    pub ramble: RambleContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteeringSubmission {
    pub submission_id: SubmissionId,
    pub session_id: SessionId,
    pub submission_digest_assertion: Option<String>,
    pub ramble: RambleContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RambleSubmissionRecord {
    pub submission_id: SubmissionId,
    pub session_id: SessionId,
    pub intent: RambleIntent,
    pub request_id: Option<RequestId>,
    pub document_json: String,
    pub body_markdown: String,
    /// Canonical digest of the complete Ramble Submission input.
    pub submission_digest: String,
    pub artifacts: Vec<SubmissionArtifact>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveDraft {
    pub draft_id: DraftId,
    pub intent: RambleIntent,
    pub session_id: Option<SessionId>,
    pub request_id: Option<RequestId>,
    /// Required only for Launch Drafts.
    pub launch_configuration: Option<LaunchConfiguration>,
    pub document_json: String,
    pub body_markdown: String,
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddDraftArtifact {
    pub draft_id: DraftId,
    pub expected_revision: u64,
    pub artifact: ArtifactInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoveDraftArtifact {
    pub draft_id: DraftId,
    pub expected_revision: u64,
    pub artifact_id: ArtifactId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReorderDraftArtifacts {
    pub draft_id: DraftId,
    pub expected_revision: u64,
    pub artifact_ids: Vec<ArtifactId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mutation")]
pub enum DraftMutation {
    Save(SaveDraft),
    AddArtifact(AddDraftArtifact),
    RemoveArtifact(RemoveDraftArtifact),
    ReorderArtifacts(ReorderDraftArtifacts),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftSnapshot {
    pub draft_id: DraftId,
    pub intent: RambleIntent,
    pub session_id: Option<SessionId>,
    pub request_id: Option<RequestId>,
    pub launch_configuration: Option<LaunchConfiguration>,
    pub document_json: String,
    pub body_markdown: String,
    pub revision: u64,
    pub artifacts: Vec<DraftArtifact>,
    pub created_at: String,
    pub updated_at: String,
}
