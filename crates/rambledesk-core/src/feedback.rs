use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::watch;
use uuid::Uuid;

use crate::workspace::{
    DraftView, FeedbackPackagePublisher, FeedbackRequestQuery, FeedbackRequestSummary,
    HostSessionSummary, NewAttachment, PublishedFeedbackPackage, StoredFeedbackWorkspace,
    SubmissionPlan,
};

mod model;
mod validation;

pub use model::*;
use validation::validate_request_input;
pub(crate) use validation::{canonical_uuid, validate_text};

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

    async fn approve_request(
        &self,
        request_id: &str,
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
                allow_finish: input.allow_finish,
                final_summary: input.final_summary,
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

    pub async fn approve_feedback(
        &self,
        input: ApproveFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        let request_id = canonical_uuid(&input.request_id, "request_id")?;
        let stored = self
            .repository
            .approve_request(&request_id, &self.clock.now_rfc3339())
            .await
            .map_err(ApplicationError::from)?;
        self.notify_feedback_terminal(&request_id);
        Ok(stored.into())
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

#[cfg(test)]
#[path = "feedback/tests.rs"]
mod tests;
