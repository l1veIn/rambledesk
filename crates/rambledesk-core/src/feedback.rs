use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use ts_rs::TS;
use uuid::Uuid;

use crate::workspace::{
    DraftView, FeedbackPackagePublisher, FeedbackRequestQuery, FeedbackRequestSummary,
    HostSessionQuery, HostSessionSummary, NewAttachment, PublishedFeedbackPackage,
    StoredFeedbackWorkspace, SubmissionPlan,
};

mod attachment_source;
mod model;
mod path_resolver;
mod repository_error;
mod validation;
mod waiters;

use attachment_source::load_request_attachment;
pub use model::*;
pub use path_resolver::AttachmentPathResolver;
pub use repository_error::RepositoryError;
use validation::validate_request_input;
pub(crate) use validation::{canonical_uuid, validate_text};
use waiters::FeedbackWaiters;

#[derive(Clone, Copy, Debug)]
pub struct SubmissionPlanInput<'a> {
    pub request_id: &'a str,
    pub expected_revision: u64,
    pub cooked_markdown: Option<&'a str>,
    pub cooking_model: Option<&'a str>,
    pub uncooked_markdown: Option<&'a str>,
    pub publication_id: &'a str,
    pub now: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationOutcome<Value> {
    pub value: Value,
    pub changed: bool,
}

impl<Value> MutationOutcome<Value> {
    pub fn changed(value: Value) -> Self {
        Self {
            value,
            changed: true,
        }
    }

    pub fn unchanged(value: Value) -> Self {
        Self {
            value,
            changed: false,
        }
    }
}

#[async_trait]
pub trait FeedbackRepository: AttachmentPathResolver + Send + Sync {
    async fn create_or_get_request(
        &self,
        request: NewFeedbackRequest,
    ) -> Result<MutationOutcome<StoredFeedbackRequest>, RepositoryError>;

    async fn get_request(&self, request_id: &str)
    -> Result<StoredFeedbackRequest, RepositoryError>;

    async fn plan_cancellation(
        &self,
        request_id: &str,
        reason: &str,
        publication_id: &str,
        now: &str,
    ) -> Result<SubmissionPlan, RepositoryError>;

    async fn complete_cancellation(
        &self,
        plan: &SubmissionPlan,
        published: &PublishedFeedbackPackage,
    ) -> Result<MutationOutcome<StoredFeedbackRequest>, RepositoryError>;

    async fn approve_request(
        &self,
        request_id: &str,
        now: &str,
    ) -> Result<MutationOutcome<StoredFeedbackRequest>, RepositoryError>;

    async fn list_open_requests(&self) -> Result<Vec<FeedbackRequestSummary>, RepositoryError>;

    async fn list_requests(
        &self,
        query: FeedbackRequestQuery,
    ) -> Result<Vec<FeedbackRequestSummary>, RepositoryError>;

    async fn list_host_sessions(
        &self,
        query: HostSessionQuery,
    ) -> Result<Vec<HostSessionSummary>, RepositoryError>;

    async fn rename_host_session(
        &self,
        host_id: &str,
        host_session_id: &str,
        title: &str,
        now: &str,
    ) -> Result<HostSessionSummary, RepositoryError>;

    async fn set_host_session_pinned(
        &self,
        host_id: &str,
        host_session_id: &str,
        pinned_at: Option<&str>,
    ) -> Result<HostSessionSummary, RepositoryError>;

    async fn archive_host_session(
        &self,
        host_id: &str,
        host_session_id: &str,
        now: &str,
    ) -> Result<HostSessionSummary, RepositoryError>;

    async fn unarchive_host_session(
        &self,
        host_id: &str,
        host_session_id: &str,
        now: &str,
    ) -> Result<HostSessionSummary, RepositoryError>;

    async fn set_host_pinned(
        &self,
        host_id: &str,
        pinned_at: Option<&str>,
        now: &str,
    ) -> Result<(), RepositoryError>;

    async fn delete_host_session(
        &self,
        host_id: &str,
        host_session_id: &str,
    ) -> Result<Vec<String>, RepositoryError>;

    async fn delete_feedback_request(&self, request_id: &str) -> Result<(), RepositoryError>;

    async fn get_workspace(
        &self,
        request_id: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError>;

    async fn save_draft(
        &self,
        request_id: &str,
        document_json: &str,
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

    async fn read_request_attachment(
        &self,
        request_id: &str,
        attachment_id: &str,
    ) -> Result<Vec<u8>, RepositoryError>;

    async fn plan_submission(
        &self,
        input: SubmissionPlanInput<'_>,
    ) -> Result<SubmissionPlan, RepositoryError>;

    async fn complete_submission(
        &self,
        plan: &SubmissionPlan,
        published: &PublishedFeedbackPackage,
    ) -> Result<MutationOutcome<StoredFeedbackRequest>, RepositoryError>;
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
    change_observer: Arc<dyn crate::ApplicationChangeObserver>,
}

impl FeedbackApplication {
    pub(crate) fn notify_application_changed(&self, resources: Vec<crate::ApplicationResourceKey>) {
        self.change_observer
            .observe(crate::ApplicationChange { resources });
    }

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
            change_observer: Arc::new(crate::NoopApplicationChangeObserver),
        }
    }

    pub fn with_change_observer(
        mut self,
        observer: Arc<dyn crate::ApplicationChangeObserver>,
    ) -> Self {
        self.change_observer = observer;
        self
    }

    pub async fn request_feedback(
        &self,
        input: RequestFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        validate_request_input(&input)?;
        let host_id = input.host_id.as_deref().unwrap_or("generic").to_owned();
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
        let mut attachments = Vec::with_capacity(input.attachments.len());
        let mut total_attachment_bytes = 0usize;
        for attachment in &input.attachments {
            let (file_name, contents, media_type) = load_request_attachment(attachment)?;
            if contents.is_empty() {
                return Err(ApplicationError::invalid_argument(
                    "attachment contents cannot be empty",
                ));
            }
            if contents.len() > crate::MAX_ATTACHMENT_BYTES {
                return Err(ApplicationError::invalid_argument(format!(
                    "attachment exceeds the {} MiB limit",
                    crate::MAX_ATTACHMENT_BYTES / 1024 / 1024
                )));
            }
            total_attachment_bytes = total_attachment_bytes
                .checked_add(contents.len())
                .ok_or_else(|| ApplicationError::invalid_argument("attachments are too large"))?;
            if total_attachment_bytes > crate::MAX_REQUEST_ATTACHMENT_TOTAL_BYTES {
                return Err(ApplicationError::invalid_argument(format!(
                    "attachments exceed the {} MiB total limit",
                    crate::MAX_REQUEST_ATTACHMENT_TOTAL_BYTES / 1024 / 1024
                )));
            }
            attachments.push(NewRequestAttachment {
                attachment_id: self.ids.new_id(),
                file_name,
                media_type,
                sha256: hex::encode(Sha256::digest(&contents)),
                contents,
            });
        }
        let now = self.clock.now_rfc3339();
        let outcome = self
            .repository
            .create_or_get_request(NewFeedbackRequest {
                request_id,
                host_session_record_id: self.ids.new_id(),
                host_id,
                host_session_id: input.host_session_id,
                title,
                what_happened: input.what_happened,
                actions: input.actions,
                context_refs: input.context_refs,
                attachments,
                source_hint: input.source_hint,
                allow_finish: input.allow_finish,
                final_summary: input.final_summary,
                created_at: now,
            })
            .await
            .map_err(ApplicationError::from)?;
        let request: FeedbackRequestView = outcome.value.into();
        if outcome.changed {
            self.notify_application_changed(vec![
                crate::ApplicationResourceKey::Navigation,
                crate::ApplicationResourceKey::FeedbackWorkspace {
                    request_id: request.request_id.clone(),
                },
            ]);
        }
        Ok(request)
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

    pub async fn recover_feedback(
        &self,
        input: RecoverFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        validate_text("host_session_id", &input.host_session_id, 1, 200)?;
        let host_id = input.host_id.as_deref().ok_or_else(|| {
            ApplicationError::invalid_argument(
                "host_id is required unless supplied by the authenticated adapter",
            )
        })?;
        validate_text("host_id", host_id, 1, 200)?;

        if let Some(request_id) = input.request_id.as_deref() {
            let request = self
                .get_feedback(GetFeedbackInput {
                    request_id: request_id.to_owned(),
                })
                .await?;
            if request.host_session_id != input.host_session_id || request.host_id != host_id {
                return Err(ApplicationError::request_not_found());
            }
            return Ok(request);
        }

        let candidates = self
            .list_feedback_requests(crate::ListFeedbackRequestsInput {
                host_id: input.host_id,
                host_session_id: Some(input.host_session_id),
                status: Some(vec![
                    FeedbackStatus::Waiting,
                    FeedbackStatus::InProgress,
                    FeedbackStatus::Completed,
                    FeedbackStatus::Cancelled,
                ]),
                archived: None,
                search: None,
                limit: Some(2),
                cursor: None,
            })
            .await?;
        match candidates.requests.as_slice() {
            [] => Err(ApplicationError::request_not_found()),
            [request] => {
                self.get_feedback(GetFeedbackInput {
                    request_id: request.request_id.clone(),
                })
                .await
            }
            _ => Err(ApplicationError::recovery_ambiguous()),
        }
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
        let outcome = self
            .repository
            .approve_request(&request_id, &self.clock.now_rfc3339())
            .await
            .map_err(ApplicationError::from)?;
        self.notify_feedback_terminal(&request_id);
        let request: FeedbackRequestView = outcome.value.into();
        if outcome.changed {
            self.notify_application_changed(vec![
                crate::ApplicationResourceKey::Navigation,
                crate::ApplicationResourceKey::FeedbackWorkspace {
                    request_id: request_id.clone(),
                },
            ]);
        }
        Ok(request)
    }

    pub async fn cancel_feedback(
        &self,
        input: CancelFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        let request_id = canonical_uuid(&input.request_id, "request_id")?;
        validate_text("reason", &input.reason, 1, 4_000)?;
        let existing = self
            .repository
            .get_request(&request_id)
            .await
            .map_err(ApplicationError::from)?;
        if existing.status == FeedbackStatus::Completed {
            return Err(ApplicationError::from(RepositoryError::RequestTerminal));
        }
        if existing.status == FeedbackStatus::Cancelled && existing.feedback.is_some() {
            self.notify_feedback_terminal(&request_id);
            let request: FeedbackRequestView = existing.into();
            return Ok(request);
        }
        let now = self.clock.now_rfc3339();
        let plan = self
            .repository
            .plan_cancellation(&request_id, &input.reason, &self.ids.new_id(), &now)
            .await
            .map_err(ApplicationError::from)?;
        let published = self
            .publisher
            .publish(&plan)
            .await
            .map_err(ApplicationError::from)?;
        let outcome = self
            .repository
            .complete_cancellation(&plan, &published)
            .await
            .map_err(ApplicationError::from)?;
        self.notify_feedback_terminal(&request_id);
        let request: FeedbackRequestView = outcome.value.into();
        if outcome.changed {
            self.notify_application_changed(vec![
                crate::ApplicationResourceKey::Navigation,
                crate::ApplicationResourceKey::FeedbackWorkspace {
                    request_id: request_id.clone(),
                },
                crate::ApplicationResourceKey::PublishedFeedback { request_id },
            ]);
        }
        Ok(request)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub enum ApplicationErrorCode {
    #[serde(rename = "INVALID_ARGUMENT")]
    #[ts(rename = "INVALID_ARGUMENT")]
    InvalidArgument,
    #[serde(rename = "REQUEST_NOT_FOUND")]
    #[ts(rename = "REQUEST_NOT_FOUND")]
    RequestNotFound,
    #[serde(rename = "RECOVERY_AMBIGUOUS")]
    #[ts(rename = "RECOVERY_AMBIGUOUS")]
    RecoveryAmbiguous,
    #[serde(rename = "REQUEST_CONFLICT")]
    #[ts(rename = "REQUEST_CONFLICT")]
    RequestConflict,
    #[serde(rename = "REQUEST_ALREADY_COMPLETED")]
    #[ts(rename = "REQUEST_ALREADY_COMPLETED")]
    RequestAlreadyCompleted,
    #[serde(rename = "REQUEST_TERMINAL")]
    #[ts(rename = "REQUEST_TERMINAL")]
    RequestTerminal,
    #[serde(rename = "DRAFT_CONFLICT")]
    #[ts(rename = "DRAFT_CONFLICT")]
    DraftConflict,
    #[serde(rename = "ATTACHMENT_NOT_FOUND")]
    #[ts(rename = "ATTACHMENT_NOT_FOUND")]
    AttachmentNotFound,
    #[serde(rename = "ATTACHMENT_LIMIT")]
    #[ts(rename = "ATTACHMENT_LIMIT")]
    AttachmentLimit,
    #[serde(rename = "HOST_SESSION_NOT_FOUND")]
    #[ts(rename = "HOST_SESSION_NOT_FOUND")]
    HostSessionNotFound,
    #[serde(rename = "HOST_SESSION_HAS_OPEN_REQUESTS")]
    #[ts(rename = "HOST_SESSION_HAS_OPEN_REQUESTS")]
    HostSessionHasOpenRequests,
    #[serde(rename = "DELETE_REQUIRES_ARCHIVED_HOST_SESSION")]
    #[ts(rename = "DELETE_REQUIRES_ARCHIVED_HOST_SESSION")]
    DeleteRequiresArchivedHostSession,
    #[serde(rename = "REQUEST_NOT_TERMINAL")]
    #[ts(rename = "REQUEST_NOT_TERMINAL")]
    RequestNotTerminal,
    #[serde(rename = "PACKAGE_PUBLISH_FAILURE")]
    #[ts(rename = "PACKAGE_PUBLISH_FAILURE")]
    PackagePublishFailure,
    #[serde(rename = "FEEDBACK_PACKAGE_READ_FAILURE")]
    #[ts(rename = "FEEDBACK_PACKAGE_READ_FAILURE")]
    FeedbackPackageReadFailure,
    #[serde(rename = "STORAGE_FAILURE")]
    #[ts(rename = "STORAGE_FAILURE")]
    StorageFailure,
}

impl ApplicationErrorCode {
    pub const ALL: [Self; 16] = [
        Self::InvalidArgument,
        Self::RequestNotFound,
        Self::RecoveryAmbiguous,
        Self::RequestConflict,
        Self::RequestAlreadyCompleted,
        Self::RequestTerminal,
        Self::DraftConflict,
        Self::AttachmentNotFound,
        Self::AttachmentLimit,
        Self::HostSessionNotFound,
        Self::HostSessionHasOpenRequests,
        Self::DeleteRequiresArchivedHostSession,
        Self::RequestNotTerminal,
        Self::PackagePublishFailure,
        Self::FeedbackPackageReadFailure,
        Self::StorageFailure,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::RequestNotFound => "REQUEST_NOT_FOUND",
            Self::RecoveryAmbiguous => "RECOVERY_AMBIGUOUS",
            Self::RequestConflict => "REQUEST_CONFLICT",
            Self::RequestAlreadyCompleted => "REQUEST_ALREADY_COMPLETED",
            Self::RequestTerminal => "REQUEST_TERMINAL",
            Self::DraftConflict => "DRAFT_CONFLICT",
            Self::AttachmentNotFound => "ATTACHMENT_NOT_FOUND",
            Self::AttachmentLimit => "ATTACHMENT_LIMIT",
            Self::HostSessionNotFound => "HOST_SESSION_NOT_FOUND",
            Self::HostSessionHasOpenRequests => "HOST_SESSION_HAS_OPEN_REQUESTS",
            Self::DeleteRequiresArchivedHostSession => "DELETE_REQUIRES_ARCHIVED_HOST_SESSION",
            Self::RequestNotTerminal => "REQUEST_NOT_TERMINAL",
            Self::PackagePublishFailure => "PACKAGE_PUBLISH_FAILURE",
            Self::FeedbackPackageReadFailure => "FEEDBACK_PACKAGE_READ_FAILURE",
            Self::StorageFailure => "STORAGE_FAILURE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize, JsonSchema, TS)]
#[error("{message}")]
pub struct ApplicationError {
    code: ApplicationErrorCode,
    message: String,
    retryable: bool,
}

impl ApplicationError {
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self {
            code: ApplicationErrorCode::InvalidArgument,
            message: message.into(),
            retryable: false,
        }
    }

    fn request_not_found() -> Self {
        Self {
            code: ApplicationErrorCode::RequestNotFound,
            message: "feedback request was not found for this host session".to_owned(),
            retryable: false,
        }
    }

    fn recovery_ambiguous() -> Self {
        Self {
            code: ApplicationErrorCode::RecoveryAmbiguous,
            message: "multiple feedback requests match this host session; provide request_id"
                .to_owned(),
            retryable: false,
        }
    }

    pub fn code(&self) -> &'static str {
        self.code.as_str()
    }

    pub const fn code_enum(&self) -> ApplicationErrorCode {
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
            RepositoryError::RequestNotFound => (
                ApplicationErrorCode::RequestNotFound,
                "feedback request was not found",
                false,
            ),
            RepositoryError::RequestConflict => (
                ApplicationErrorCode::RequestConflict,
                "request_id already exists with different immutable input",
                false,
            ),
            RepositoryError::RequestAlreadyCompleted => (
                ApplicationErrorCode::RequestAlreadyCompleted,
                "completed feedback cannot be cancelled",
                false,
            ),
            RepositoryError::RequestTerminal => (
                ApplicationErrorCode::RequestTerminal,
                "terminal feedback cannot be modified",
                false,
            ),
            RepositoryError::DraftConflict => (
                ApplicationErrorCode::DraftConflict,
                "draft revision changed; reload before saving or submitting",
                false,
            ),
            RepositoryError::DraftEmpty => (
                ApplicationErrorCode::InvalidArgument,
                "feedback draft cannot be empty when submitting",
                false,
            ),
            RepositoryError::AttachmentNotFound => (
                ApplicationErrorCode::AttachmentNotFound,
                "feedback attachment was not found",
                false,
            ),
            RepositoryError::AttachmentLimit => (
                ApplicationErrorCode::AttachmentLimit,
                "a feedback request can contain at most 20 attachments",
                false,
            ),
            RepositoryError::HostSessionNotFound => (
                ApplicationErrorCode::HostSessionNotFound,
                "host session was not found",
                false,
            ),
            RepositoryError::HostSessionHasOpenRequests => (
                ApplicationErrorCode::HostSessionHasOpenRequests,
                "finish or cancel open feedback requests before archiving this session",
                false,
            ),
            RepositoryError::DeleteRequiresArchivedHostSession => (
                ApplicationErrorCode::DeleteRequiresArchivedHostSession,
                "archive the host session before deleting requests",
                false,
            ),
            RepositoryError::RequestNotTerminal => (
                ApplicationErrorCode::RequestNotTerminal,
                "finish or cancel the feedback request before deleting it",
                false,
            ),
            RepositoryError::PackagePublish => (
                ApplicationErrorCode::PackagePublishFailure,
                "feedback package could not be published",
                true,
            ),
            RepositoryError::PackageRead => (
                ApplicationErrorCode::FeedbackPackageReadFailure,
                "feedback package could not be read or verified",
                true,
            ),
            RepositoryError::CorruptData | RepositoryError::Storage => (
                ApplicationErrorCode::StorageFailure,
                "feedback storage operation failed",
                true,
            ),
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
