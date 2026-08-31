use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::RepositoryError;

const DEFAULT_POLL_AFTER_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ActionInput {
    pub id: String,
    pub instruction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ContextRef {
    pub label: String,
    pub uri: String,
}

/// An immutable review artifact supplied by the requesting agent.
///
/// Provide exactly one of `markdown`, `contents_base64`, or `path`.
/// Prefer `path` for local files so the MCP/tool payload stays small.
/// The media type is detected server-side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RequestAttachmentInput {
    pub file_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "Markdown document contents. Requires a .md or .markdown file_name. Mutually exclusive with contents_base64 and path."
    )]
    pub markdown: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "Base64-encoded PNG/JPEG/GIF/WebP image. Prefer path when the file is already on disk. Mutually exclusive with markdown and path."
    )]
    pub contents_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "Absolute local filesystem path. The server reads the file. Use this for images and Markdown already on disk. Mutually exclusive with markdown and contents_base64."
    )]
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RequestFeedbackInput {
    pub request_id: Option<String>,
    /// Optional host family id (e.g. `pi`, `claude`, `codex`, `opencode`, `generic`).
    /// Auto-registered adapters inject it via `RAMBLEDESK_HOST` / `X-RambleDesk-Host`;
    /// when absent, the server defaults to `generic`.
    #[serde(default)]
    pub host_id: Option<String>,
    pub host_session_id: String,
    /// Short Ramble title shown in inboxes and workspace headings.
    pub title: Option<String>,
    pub what_happened: String,
    pub actions: Vec<ActionInput>,
    #[serde(default)]
    pub context_refs: Vec<ContextRef>,
    /// Markdown documents and images the human should review with this request.
    #[serde(default)]
    pub attachments: Vec<RequestAttachmentInput>,
    #[serde(default)]
    pub source_hint: Option<String>,
    #[serde(default)]
    pub allow_finish: bool,
    #[serde(default)]
    pub final_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct GetFeedbackInput {
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct RecoverFeedbackInput {
    #[serde(default)]
    #[ts(optional)]
    pub request_id: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub host_id: Option<String>,
    pub host_session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct CancelFeedbackInput {
    pub request_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ApproveFeedbackInput {
    pub request_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum FeedbackResolution {
    FeedbackSubmitted,
    Approved,
    Cancelled,
}

impl FeedbackResolution {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FeedbackSubmitted => "feedback_submitted",
            Self::Approved => "approved",
            Self::Cancelled => "cancelled",
        }
    }
}

impl TryFrom<&str> for FeedbackResolution {
    type Error = RepositoryError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "feedback_submitted" => Ok(Self::FeedbackSubmitted),
            "approved" => Ok(Self::Approved),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(RepositoryError::CorruptData),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum FeedbackStatus {
    Waiting,
    InProgress,
    Completed,
    Cancelled,
}

impl FeedbackStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl TryFrom<&str> for FeedbackStatus {
    type Error = RepositoryError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "waiting" => Ok(Self::Waiting),
            "in_progress" => Ok(Self::InProgress),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(RepositoryError::CorruptData),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ExecutionMode {
    Poll,
    Wait,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FeedbackResultView {
    pub package_uri: String,
    pub directory_path: String,
    pub markdown_path: String,
    pub manifest_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FeedbackRequestView {
    pub request_id: String,
    pub host_id: String,
    pub host_session_id: String,
    pub status: FeedbackStatus,
    pub execution_mode: ExecutionMode,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub poll_after_ms: Option<u64>,
    pub feedback: Option<FeedbackResultView>,
    pub resolution: Option<FeedbackResolution>,
    pub allow_finish: bool,
    pub final_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFeedbackRequest {
    pub request_id: String,
    pub host_session_record_id: String,
    pub host_id: String,
    pub host_session_id: String,
    pub title: String,
    pub what_happened: String,
    pub actions: Vec<ActionInput>,
    pub context_refs: Vec<ContextRef>,
    pub attachments: Vec<NewRequestAttachment>,
    pub source_hint: Option<String>,
    pub allow_finish: bool,
    pub final_summary: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRequestAttachment {
    pub attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    pub contents: Vec<u8>,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFeedbackRequest {
    pub request_id: String,
    pub host_id: String,
    pub host_session_id: String,
    pub status: FeedbackStatus,
    pub created_at: String,
    pub updated_at: String,
    pub feedback: Option<FeedbackResultView>,
    pub resolution: Option<FeedbackResolution>,
    pub allow_finish: bool,
    pub final_summary: Option<String>,
}

impl From<StoredFeedbackRequest> for FeedbackRequestView {
    fn from(value: StoredFeedbackRequest) -> Self {
        Self::from_stored(value, ExecutionMode::Poll)
    }
}

impl FeedbackRequestView {
    pub(super) fn from_stored(value: StoredFeedbackRequest, execution_mode: ExecutionMode) -> Self {
        Self {
            request_id: value.request_id,
            host_id: value.host_id,
            host_session_id: value.host_session_id,
            status: value.status,
            execution_mode,
            created_at: value.created_at,
            updated_at: value.updated_at,
            poll_after_ms: matches!(
                value.status,
                FeedbackStatus::Waiting | FeedbackStatus::InProgress
            )
            .then_some(DEFAULT_POLL_AFTER_MS),
            feedback: value.feedback,
            resolution: value.resolution,
            allow_finish: value.allow_finish,
            final_summary: value.final_summary,
        }
    }
}
