use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    ActionInput, ContextRef, FeedbackResolution, FeedbackResultView, FeedbackStatus,
    RepositoryError,
};

pub const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_ATTACHMENT_COUNT: usize = 20;
pub const MAX_REQUEST_ATTACHMENT_TOTAL_BYTES: usize = 60 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeedbackPackageAttachment {
    pub id: String,
    pub file_name: String,
    pub media_type: String,
    pub byte_size: u64,
    pub sha256: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeedbackPackageManifest {
    pub schema_version: u32,
    pub request_id: String,
    pub title: String,
    pub host_id: String,
    pub host_session_id: String,
    pub source_hint: Option<String>,
    pub submitted_at: String,
    #[serde(
        default = "default_feedback_submitted",
        skip_serializing_if = "is_feedback_submitted"
    )]
    pub resolution: FeedbackResolution,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_reason: Option<String>,
    pub source_revision: u64,
    pub draft_revision: u64,
    pub feedback_markdown: String,
    pub feedback_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncooked_markdown: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncooked_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooking_model: Option<String>,
    pub attachments: Vec<FeedbackPackageAttachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub request_attachments: Vec<FeedbackPackageAttachment>,
}

fn default_feedback_submitted() -> FeedbackResolution {
    FeedbackResolution::FeedbackSubmitted
}

fn is_feedback_submitted(resolution: &FeedbackResolution) -> bool {
    *resolution == FeedbackResolution::FeedbackSubmitted
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeedbackPackageContent {
    pub manifest: FeedbackPackageManifest,
    pub markdown: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncooked_markdown: Option<String>,
    pub attachment_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub request_attachment_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FeedbackRequestSummary {
    pub request_id: String,
    pub host_id: String,
    pub host_session_id: String,
    pub source_hint: Option<String>,
    pub title: String,
    pub what_happened: String,
    pub status: FeedbackStatus,
    pub resolution: Option<FeedbackResolution>,
    pub allow_finish: bool,
    pub final_summary: Option<String>,
    #[ts(type = "number")]
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct HostSessionSummary {
    pub host_id: String,
    pub host_session_id: String,
    pub title: String,
    pub source_hint: Option<String>,
    #[ts(type = "number")]
    pub request_count: u64,
    #[ts(type = "number")]
    pub pending_count: u64,
    pub updated_at: String,
    pub pinned_at: Option<String>,
    pub archived_at: Option<String>,
    pub host_pinned_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct HostSessionInput {
    pub host_id: String,
    pub host_session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct RenameHostSessionInput {
    pub host_id: String,
    pub host_session_id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SetHostSessionPinnedInput {
    pub host_id: String,
    pub host_session_id: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SetHostPinnedInput {
    pub host_id: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS, Default)]
pub struct ListHostSessionsInput {
    pub search: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct DeleteFeedbackRequestInput {
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS, Default)]
pub struct ListFeedbackRequestsInput {
    pub host_id: Option<String>,
    pub host_session_id: Option<String>,
    pub status: Option<Vec<FeedbackStatus>>,
    pub archived: Option<bool>,
    pub search: Option<String>,
    #[ts(type = "number | null")]
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ListFeedbackRequestsOutput {
    pub requests: Vec<FeedbackRequestSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackRequestQuery {
    pub host_id: Option<String>,
    pub host_session_id: Option<String>,
    pub statuses: Vec<FeedbackStatus>,
    pub archived: bool,
    pub search: Option<String>,
    pub limit: u32,
    pub before_updated_at: Option<String>,
    pub before_request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSessionQuery {
    pub archived: bool,
    pub search: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct DraftView {
    pub document_json: Option<String>,
    pub body_markdown: String,
    #[ts(type = "number")]
    pub saved_revision: u64,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FeedbackWorkspaceView {
    pub request: FeedbackRequestSummary,
    pub actions: Vec<ActionInput>,
    pub context_refs: Vec<ContextRef>,
    pub request_attachments: Vec<RequestAttachmentView>,
    pub draft: DraftView,
    pub attachments: Vec<AttachmentView>,
    pub feedback: Option<FeedbackResultView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct RequestAttachmentView {
    pub attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    #[ts(type = "number")]
    pub byte_size: u64,
    pub sha256: String,
    #[ts(type = "number")]
    pub position: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AttachmentView {
    pub attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    #[ts(type = "number")]
    pub byte_size: u64,
    pub sha256: String,
    #[ts(type = "number")]
    pub position: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AddAttachmentInput {
    pub request_id: String,
    pub file_name: String,
    #[ts(type = "number[]")]
    pub contents: Vec<u8>,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct RemoveAttachmentInput {
    pub request_id: String,
    pub attachment_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ReorderAttachmentsInput {
    pub request_id: String,
    pub attachment_ids: Vec<String>,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SaveDraftInput {
    pub request_id: String,
    pub document_json: String,
    pub body_markdown: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SubmitFeedbackInput {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
    #[serde(default)]
    #[ts(optional)]
    pub cooked_markdown: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub cooking_model: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub uncooked_markdown: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFeedbackWorkspace {
    pub request: FeedbackRequestSummary,
    pub actions: Vec<ActionInput>,
    pub context_refs: Vec<ContextRef>,
    pub request_attachments: Vec<RequestAttachmentView>,
    pub draft: DraftView,
    pub attachments: Vec<AttachmentView>,
    pub feedback: Option<FeedbackResultView>,
}

impl From<StoredFeedbackWorkspace> for FeedbackWorkspaceView {
    fn from(value: StoredFeedbackWorkspace) -> Self {
        Self {
            request: value.request,
            actions: value.actions,
            context_refs: value.context_refs,
            request_attachments: value.request_attachments,
            draft: value.draft,
            attachments: value.attachments,
            feedback: value.feedback,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAttachment {
    pub attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    pub contents: Vec<u8>,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionAttachment {
    pub attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    pub byte_size: u64,
    pub sha256: String,
    pub draft_path: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionRequestAttachment {
    pub attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    pub byte_size: u64,
    pub sha256: String,
    pub draft_path: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionPlan {
    pub request_id: String,
    pub host_id: String,
    pub host_session_id: String,
    pub source_hint: Option<String>,
    pub title: String,
    pub what_happened: String,
    pub actions: Vec<ActionInput>,
    pub attachments: Vec<SubmissionAttachment>,
    pub request_attachments: Vec<SubmissionRequestAttachment>,
    pub resolution: FeedbackResolution,
    pub cancel_reason: Option<String>,
    /// Canonical feedback returned to the host (`feedback.md`).
    pub body_markdown: String,
    /// Human-authored source preserved alongside canonical feedback (`uncooked.md`).
    pub uncooked_markdown: String,
    pub cooking_model: Option<String>,
    pub source_revision: u64,
    pub publication_id: String,
    pub body_sha256: String,
    pub submitted_at: String,
    pub package_uri: String,
    pub directory_path: String,
    pub temp_directory_path: String,
    pub markdown_path: String,
    pub manifest_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedFeedbackPackage {
    pub result: FeedbackResultView,
    pub manifest_sha256: String,
    pub published_at: String,
}

#[async_trait]
pub trait FeedbackPackagePublisher: Send + Sync {
    async fn publish(
        &self,
        plan: &SubmissionPlan,
    ) -> Result<PublishedFeedbackPackage, RepositoryError>;
}

#[async_trait]
pub trait FeedbackPackageReader: Send + Sync {
    async fn read(
        &self,
        request_id: &str,
        result: &FeedbackResultView,
    ) -> Result<FeedbackPackageContent, RepositoryError>;
}
