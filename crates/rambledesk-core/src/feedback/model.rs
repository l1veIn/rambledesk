use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RequestFeedbackInput {
    pub request_id: Option<String>,
    pub host_id: String,
    pub host_session_id: String,
    /// Short Ramble title shown in inboxes and workspace headings.
    pub title: Option<String>,
    pub what_happened: String,
    pub actions: Vec<ActionInput>,
    #[serde(default)]
    pub context_refs: Vec<ContextRef>,
    #[serde(default)]
    pub source_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GetFeedbackInput {
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct CancelFeedbackInput {
    pub request_id: String,
    pub reason: String,
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
    pub source_hint: Option<String>,
    pub created_at: String,
}

impl NewFeedbackRequest {
    pub fn immutable_input_hash(&self) -> String {
        let bytes = serde_json::to_vec(&ImmutableRequest {
            host_id: &self.host_id,
            host_session_id: &self.host_session_id,
            title: &self.title,
            what_happened: &self.what_happened,
            actions: &self.actions,
            context_refs: &self.context_refs,
            source_hint: self.source_hint.as_deref(),
        })
        .expect("validated feedback input must serialize");
        hex::encode(Sha256::digest(bytes))
    }
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
        }
    }
}

#[derive(Serialize)]
struct ImmutableRequest<'a> {
    host_id: &'a str,
    host_session_id: &'a str,
    title: &'a str,
    what_happened: &'a str,
    actions: &'a [ActionInput],
    context_refs: &'a [ContextRef],
    source_hint: Option<&'a str>,
}
