use std::sync::Arc;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::workspace::{
    DraftView, FeedbackPackagePublisher, FeedbackRequestQuery, FeedbackRequestSummary,
    HostSessionQuery, HostSessionSummary, NewAttachment, PublishedFeedbackPackage,
    StoredFeedbackWorkspace, SubmissionPlan,
};

mod attachment_source;
mod error;
mod managed;
mod model;
mod path_resolver;
mod repository_error;
mod validation;
mod waiters;

use attachment_source::load_request_attachment;
pub use error::{ApplicationError, ApplicationErrorCode};
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
        self.request_feedback_with_scope(input, None).await
    }

    async fn request_feedback_with_scope(
        &self,
        input: RequestFeedbackInput,
        managed_session_id: Option<&str>,
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
                managed_session_id: managed_session_id.map(ToOwned::to_owned),
                host_session_record_id: managed_session_id
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| self.ids.new_id()),
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

#[cfg(test)]
#[path = "feedback/tests.rs"]
mod tests;
