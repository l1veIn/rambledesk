use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::watch;
use ts_rs::TS;
use uuid::Uuid;

use crate::workspace::{
    DraftView, FeedbackPackagePublisher, FeedbackRequestQuery, FeedbackRequestSummary,
    HostSessionSummary, NewAttachment, PublishedFeedbackPackage, StoredFeedbackWorkspace,
    SubmissionPlan,
};

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
    fn from_stored(value: StoredFeedbackRequest, execution_mode: ExecutionMode) -> Self {
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

#[derive(Default)]
struct FeedbackWaiters {
    channels: Mutex<HashMap<String, watch::Sender<u64>>>,
}

impl FeedbackWaiters {
    fn subscribe(&self, request_id: &str) -> watch::Receiver<u64> {
        let mut channels = self
            .channels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        channels
            .entry(request_id.to_owned())
            .or_insert_with(|| watch::channel(0).0)
            .subscribe()
    }

    fn notify_terminal(&self, request_id: &str) {
        let sender = self
            .channels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(request_id);
        if let Some(sender) = sender {
            sender.send_modify(|generation| *generation += 1);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RepositoryError {
    #[error("feedback request was not found")]
    RequestNotFound,
    #[error("feedback request conflicts with an existing request")]
    RequestConflict,
    #[error("feedback request is already completed")]
    RequestAlreadyCompleted,
    #[error("feedback request is terminal")]
    RequestTerminal,
    #[error("draft revision conflicts with the stored revision")]
    DraftConflict,
    #[error("feedback draft is empty")]
    DraftEmpty,
    #[error("attachment was not found")]
    AttachmentNotFound,
    #[error("attachment limit was reached")]
    AttachmentLimit,
    #[error("feedback package publication failed")]
    PackagePublish,
    #[error("feedback package could not be read")]
    PackageRead,
    #[error("stored feedback data is invalid")]
    CorruptData,
    #[error("storage operation failed")]
    Storage,
}

#[async_trait]
pub trait FeedbackRepository: Send + Sync {
    async fn create_or_get_request(
        &self,
        request: NewFeedbackRequest,
    ) -> Result<StoredFeedbackRequest, RepositoryError>;

    async fn get_request(&self, request_id: &str)
    -> Result<StoredFeedbackRequest, RepositoryError>;

    async fn cancel_request(
        &self,
        request_id: &str,
        reason: &str,
        now: &str,
    ) -> Result<StoredFeedbackRequest, RepositoryError>;

    async fn list_open_requests(&self) -> Result<Vec<FeedbackRequestSummary>, RepositoryError>;

    async fn list_requests(
        &self,
        query: FeedbackRequestQuery,
    ) -> Result<Vec<FeedbackRequestSummary>, RepositoryError>;

    async fn list_host_sessions(&self) -> Result<Vec<HostSessionSummary>, RepositoryError>;

    async fn get_workspace(
        &self,
        request_id: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError>;

    async fn save_draft(
        &self,
        request_id: &str,
        body_markdown: &str,
        expected_revision: u64,
        now: &str,
    ) -> Result<DraftView, RepositoryError>;

    async fn add_attachment(
        &self,
        request_id: &str,
        attachment: NewAttachment,
        expected_revision: u64,
        now: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError>;

    async fn remove_attachment(
        &self,
        request_id: &str,
        attachment_id: &str,
        expected_revision: u64,
        now: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError>;

    async fn reorder_attachments(
        &self,
        request_id: &str,
        attachment_ids: &[String],
        expected_revision: u64,
        now: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError>;

    async fn read_attachment(
        &self,
        request_id: &str,
        attachment_id: &str,
    ) -> Result<Vec<u8>, RepositoryError>;

    async fn plan_submission(
        &self,
        request_id: &str,
        expected_revision: u64,
        publication_id: &str,
        now: &str,
    ) -> Result<SubmissionPlan, RepositoryError>;

    async fn complete_submission(
        &self,
        plan: &SubmissionPlan,
        published: &PublishedFeedbackPackage,
    ) -> Result<StoredFeedbackRequest, RepositoryError>;
}

pub trait Clock: Send + Sync {
    fn now_rfc3339(&self) -> String;
}

pub trait IdGenerator: Send + Sync {
    fn new_id(&self) -> String;
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_rfc3339(&self) -> String {
        OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .expect("UTC timestamp must format as RFC 3339")
    }
}

#[derive(Debug, Default)]
pub struct UuidV7Generator;

impl IdGenerator for UuidV7Generator {
    fn new_id(&self) -> String {
        Uuid::now_v7().to_string()
    }
}

#[derive(Clone)]
pub struct FeedbackApplication {
    pub(crate) repository: Arc<dyn FeedbackRepository>,
    pub(crate) publisher: Arc<dyn FeedbackPackagePublisher>,
    pub(crate) package_reader: Arc<dyn crate::FeedbackPackageReader>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) ids: Arc<dyn IdGenerator>,
    waiters: Arc<FeedbackWaiters>,
}

impl FeedbackApplication {
    pub(crate) fn notify_feedback_terminal(&self, request_id: &str) {
        self.waiters.notify_terminal(request_id);
    }

    pub fn new(
        repository: Arc<dyn FeedbackRepository>,
        publisher: Arc<dyn FeedbackPackagePublisher>,
        package_reader: Arc<dyn crate::FeedbackPackageReader>,
    ) -> Self {
        Self::with_runtime(
            repository,
            publisher,
            package_reader,
            Arc::new(SystemClock),
            Arc::new(UuidV7Generator),
        )
    }

    pub fn with_runtime(
        repository: Arc<dyn FeedbackRepository>,
        publisher: Arc<dyn FeedbackPackagePublisher>,
        package_reader: Arc<dyn crate::FeedbackPackageReader>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
    ) -> Self {
        Self {
            repository,
            publisher,
            package_reader,
            clock,
            ids,
            waiters: Arc::new(FeedbackWaiters::default()),
        }
    }

    pub async fn request_feedback(
        &self,
        input: RequestFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        validate_request_input(&input)?;
        let title = input
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "Untitled feedback request".to_owned());
        let request_id = match input.request_id.as_deref() {
            Some(request_id) => canonical_uuid(request_id, "request_id")?,
            None => self.ids.new_id(),
        };
        let now = self.clock.now_rfc3339();
        let stored = self
            .repository
            .create_or_get_request(NewFeedbackRequest {
                request_id,
                host_session_record_id: self.ids.new_id(),
                host_id: input.host_id,
                host_session_id: input.host_session_id,
                title,
                what_happened: input.what_happened,
                actions: input.actions,
                context_refs: input.context_refs,
                source_hint: input.source_hint,
                created_at: now,
            })
            .await
            .map_err(ApplicationError::from)?;
        Ok(stored.into())
    }

    pub async fn get_feedback(
        &self,
        input: GetFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        let request_id = canonical_uuid(&input.request_id, "request_id")?;
        self.repository
            .get_request(&request_id)
            .await
            .map(Into::into)
            .map_err(ApplicationError::from)
    }

    pub async fn wait_feedback(
        &self,
        input: GetFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        let request_id = canonical_uuid(&input.request_id, "request_id")?;
        let mut changes = self.waiters.subscribe(&request_id);
        loop {
            let stored = self
                .repository
                .get_request(&request_id)
                .await
                .map_err(ApplicationError::from)?;
            if matches!(
                stored.status,
                FeedbackStatus::Completed | FeedbackStatus::Cancelled
            ) {
                return Ok(FeedbackRequestView::from_stored(
                    stored,
                    ExecutionMode::Wait,
                ));
            }
            if changes.changed().await.is_err() {
                changes = self.waiters.subscribe(&request_id);
            }
        }
    }

    pub async fn cancel_feedback(
        &self,
        input: CancelFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        let request_id = canonical_uuid(&input.request_id, "request_id")?;
        validate_text("reason", &input.reason, 1, 4_000)?;
        let stored = self
            .repository
            .cancel_request(&request_id, &input.reason, &self.clock.now_rfc3339())
            .await
            .map_err(ApplicationError::from)?;
        self.notify_feedback_terminal(&request_id);
        Ok(stored.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[error("{message}")]
pub struct ApplicationError {
    code: &'static str,
    message: String,
    retryable: bool,
}

impl ApplicationError {
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self {
            code: "INVALID_ARGUMENT",
            message: message.into(),
            retryable: false,
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn retryable(&self) -> bool {
        self.retryable
    }
}

impl From<RepositoryError> for ApplicationError {
    fn from(value: RepositoryError) -> Self {
        let (code, message, retryable) = match value {
            RepositoryError::RequestNotFound => {
                ("REQUEST_NOT_FOUND", "feedback request was not found", false)
            }
            RepositoryError::RequestConflict => (
                "REQUEST_CONFLICT",
                "request_id already exists with different immutable input",
                false,
            ),
            RepositoryError::RequestAlreadyCompleted => (
                "REQUEST_ALREADY_COMPLETED",
                "completed feedback cannot be cancelled",
                false,
            ),
            RepositoryError::RequestTerminal => (
                "REQUEST_TERMINAL",
                "terminal feedback cannot be modified",
                false,
            ),
            RepositoryError::DraftConflict => (
                "DRAFT_CONFLICT",
                "draft revision changed; reload before saving or submitting",
                false,
            ),
            RepositoryError::DraftEmpty => (
                "INVALID_ARGUMENT",
                "feedback draft cannot be empty when submitting",
                false,
            ),
            RepositoryError::AttachmentNotFound => (
                "ATTACHMENT_NOT_FOUND",
                "feedback attachment was not found",
                false,
            ),
            RepositoryError::AttachmentLimit => (
                "ATTACHMENT_LIMIT",
                "a feedback request can contain at most 20 attachments",
                false,
            ),
            RepositoryError::PackagePublish => (
                "PACKAGE_PUBLISH_FAILURE",
                "feedback package could not be published",
                true,
            ),
            RepositoryError::PackageRead => (
                "FEEDBACK_PACKAGE_READ_FAILURE",
                "feedback package could not be read or verified",
                true,
            ),
            RepositoryError::CorruptData | RepositoryError::Storage => {
                ("STORAGE_FAILURE", "feedback storage operation failed", true)
            }
        };
        Self {
            code,
            message: message.to_owned(),
            retryable,
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

fn validate_request_input(input: &RequestFeedbackInput) -> Result<(), ApplicationError> {
    validate_text("host_id", &input.host_id, 1, 64)?;
    validate_text("host_session_id", &input.host_session_id, 1, 256)?;
    if let Some(title) = input.title.as_deref() {
        validate_text("title", title, 1, 160)?;
        if title.trim().is_empty() {
            return Err(ApplicationError::invalid_argument(
                "title must contain visible characters",
            ));
        }
    }
    validate_text("what_happened", &input.what_happened, 1, 12_000)?;

    if let Some(request_id) = input.request_id.as_deref() {
        canonical_uuid(request_id, "request_id")?;
    }
    if let Some(source_hint) = input.source_hint.as_deref() {
        validate_text("source_hint", source_hint, 1, 4_096)?;
    }

    if !(1..=20).contains(&input.actions.len()) {
        return Err(ApplicationError::invalid_argument(
            "actions must contain between 1 and 20 items",
        ));
    }
    let mut action_ids = HashSet::with_capacity(input.actions.len());
    for action in &input.actions {
        if !valid_action_id(&action.id) {
            return Err(ApplicationError::invalid_argument(
                "action id must match ^[a-z0-9][a-z0-9_-]{0,63}$",
            ));
        }
        if !action_ids.insert(action.id.as_str()) {
            return Err(ApplicationError::invalid_argument(
                "action ids must be unique within a request",
            ));
        }
        validate_text("action.instruction", &action.instruction, 1, 2_000)?;
    }

    if input.context_refs.len() > 20 {
        return Err(ApplicationError::invalid_argument(
            "context_refs cannot contain more than 20 items",
        ));
    }
    for context_ref in &input.context_refs {
        validate_text("context_ref.label", &context_ref.label, 1, 256)?;
        validate_text("context_ref.uri", &context_ref.uri, 1, 4_096)?;
    }

    Ok(())
}

pub(crate) fn canonical_uuid(value: &str, field: &str) -> Result<String, ApplicationError> {
    Uuid::parse_str(value)
        .map(|value| value.hyphenated().to_string())
        .map_err(|_| ApplicationError::invalid_argument(format!("{field} must be a UUID")))
}

pub(crate) fn validate_text(
    field: &str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<(), ApplicationError> {
    let length = value.chars().count();
    if !(min..=max).contains(&length) {
        return Err(ApplicationError::invalid_argument(format!(
            "{field} must contain between {min} and {max} characters"
        )));
    }
    if value.contains('\0') {
        return Err(ApplicationError::invalid_argument(format!(
            "{field} cannot contain NUL"
        )));
    }
    Ok(())
}

fn valid_action_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_lowercase_or_digit()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase_or_digit() || matches!(byte, b'_' | b'-'))
}

trait AsciiLowercaseOrDigit {
    fn is_ascii_lowercase_or_digit(&self) -> bool;
}

impl AsciiLowercaseOrDigit for u8 {
    fn is_ascii_lowercase_or_digit(&self) -> bool {
        self.is_ascii_lowercase() || self.is_ascii_digit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_action_id_format() {
        assert!(valid_action_id("open-onboarding_1"));
        assert!(!valid_action_id("Open"));
        assert!(!valid_action_id("-leading"));
    }

    #[test]
    fn application_errors_expose_stable_public_fields() {
        let error = ApplicationError::from(RepositoryError::RequestConflict);
        assert_eq!(error.code(), "REQUEST_CONFLICT");
        assert!(!error.retryable());
        assert!(!error.message().contains("sqlite"));
    }

    #[test]
    fn canonicalizes_uuid_inputs() {
        let canonical = "0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827";
        assert_eq!(
            canonical_uuid(&canonical.to_uppercase(), "request_id").expect("uppercase UUID"),
            canonical
        );
    }

    #[test]
    fn terminal_results_omit_poll_interval() {
        let value = serde_json::to_value(FeedbackRequestView::from(StoredFeedbackRequest {
            request_id: "request".to_owned(),
            host_id: "generic".to_owned(),
            host_session_id: "session".to_owned(),
            status: FeedbackStatus::Cancelled,
            created_at: "2026-07-29T00:00:00Z".to_owned(),
            updated_at: "2026-07-29T00:01:00Z".to_owned(),
            feedback: None,
        }))
        .expect("feedback result");
        assert!(value.get("poll_after_ms").is_none());
    }
}
