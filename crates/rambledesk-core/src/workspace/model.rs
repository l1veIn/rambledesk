use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ActionInput, ContextRef, FeedbackResultView, FeedbackStatus, RepositoryError};

pub const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_ATTACHMENT_COUNT: usize = 20;

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
    pub source_revision: u64,
    pub draft_revision: u64,
    pub feedback_markdown: String,
    pub feedback_sha256: String,
    pub attachments: Vec<FeedbackPackageAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeedbackPackageContent {
    pub manifest: FeedbackPackageManifest,
    pub markdown: String,
    pub attachment_paths: Vec<String>,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS, Default)]
pub struct ListFeedbackRequestsInput {
    pub host_id: Option<String>,
    pub host_session_id: Option<String>,
    pub status: Option<Vec<FeedbackStatus>>,
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
    pub limit: u32,
    pub before_updated_at: Option<String>,
    pub before_request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct DraftView {
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
    pub draft: DraftView,
    pub attachments: Vec<AttachmentView>,
    pub feedback: Option<FeedbackResultView>,
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
    pub body_markdown: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SubmitFeedbackInput {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFeedbackWorkspace {
    pub request: FeedbackRequestSummary,
    pub actions: Vec<ActionInput>,
    pub context_refs: Vec<ContextRef>,
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
pub struct SubmissionPlan {
    pub request_id: String,
    pub host_id: String,
    pub host_session_id: String,
    pub source_hint: Option<String>,
    pub title: String,
    pub what_happened: String,
    pub actions: Vec<ActionInput>,
    pub attachments: Vec<SubmissionAttachment>,
    pub body_markdown: String,
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
