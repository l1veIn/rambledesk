use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    ActionInput, ApplicationError, ContextRef, FeedbackApplication, FeedbackResultView,
    FeedbackStatus, RepositoryError, StoredFeedbackRequest,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FeedbackRequestSummary {
    pub request_id: String,
    pub project_id: String,
    pub project_name: String,
    pub agent: String,
    pub session_id: String,
    pub what_happened: String,
    pub status: FeedbackStatus,
    #[ts(type = "number")]
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
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
}

impl From<StoredFeedbackWorkspace> for FeedbackWorkspaceView {
    fn from(value: StoredFeedbackWorkspace) -> Self {
        Self {
            request: value.request,
            actions: value.actions,
            context_refs: value.context_refs,
            draft: value.draft,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionPlan {
    pub request_id: String,
    pub project_id: String,
    pub agent: String,
    pub session_id: String,
    pub what_happened: String,
    pub actions: Vec<ActionInput>,
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

impl FeedbackApplication {
    pub async fn list_open_feedback_requests(
        &self,
    ) -> Result<Vec<FeedbackRequestSummary>, ApplicationError> {
        self.repository
            .list_open_requests()
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn get_feedback_workspace(
        &self,
        request_id: String,
    ) -> Result<FeedbackWorkspaceView, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&request_id, "request_id")?;
        self.repository
            .get_workspace(&request_id)
            .await
            .map(Into::into)
            .map_err(ApplicationError::from)
    }

    pub async fn save_feedback_draft(
        &self,
        input: SaveDraftInput,
    ) -> Result<DraftView, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&input.request_id, "request_id")?;
        crate::feedback::validate_text("body_markdown", &input.body_markdown, 0, 100_000)?;
        self.repository
            .save_draft(
                &request_id,
                &input.body_markdown,
                input.expected_revision,
                &self.clock.now_rfc3339(),
            )
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn submit_feedback(
        &self,
        input: SubmitFeedbackInput,
    ) -> Result<crate::FeedbackRequestView, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&input.request_id, "request_id")?;
        let existing = self
            .repository
            .get_request(&request_id)
            .await
            .map_err(ApplicationError::from)?;
        if existing.status == FeedbackStatus::Completed {
            return Ok(existing.into());
        }
        let now = self.clock.now_rfc3339();
        let plan_result = self
            .repository
            .plan_submission(
                &request_id,
                input.expected_revision,
                &self.ids.new_id(),
                &now,
            )
            .await;
        let plan = match plan_result {
            Ok(plan) => plan,
            Err(RepositoryError::RequestTerminal) => {
                let raced = self
                    .repository
                    .get_request(&request_id)
                    .await
                    .map_err(ApplicationError::from)?;
                if raced.status == FeedbackStatus::Completed {
                    return Ok(raced.into());
                }
                return Err(ApplicationError::from(RepositoryError::RequestTerminal));
            }
            Err(error) => return Err(ApplicationError::from(error)),
        };
        let published = self
            .publisher
            .publish(&plan)
            .await
            .map_err(ApplicationError::from)?;
        let stored: StoredFeedbackRequest = self
            .repository
            .complete_submission(&plan, &published)
            .await
            .map_err(ApplicationError::from)?;
        Ok(stored.into())
    }
}
