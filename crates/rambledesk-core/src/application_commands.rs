use std::sync::Arc;

mod agents;
mod managed;
pub use agents::AgentManagementError;
pub use managed::{ManagedCommandError, ManagedCommandErrorCode};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    ActionInput, AddAttachmentInput, ApplicationError, ApproveFeedbackInput, AttachmentView,
    CancelFeedbackInput, ContextRef, DeleteFeedbackRequestInput, DraftView, ExecutionMode,
    FeedbackApplication, FeedbackPackageView, FeedbackRequestSummary, FeedbackRequestView,
    FeedbackResolution, FeedbackStatus, FeedbackWorkspaceView, GetFeedbackInput, HostSessionInput,
    HostSessionSummary, ListFeedbackRequestsInput, ListFeedbackRequestsOutput,
    ListHostSessionsInput, ReadAttachmentInput, RemoveAttachmentInput, RenameHostSessionInput,
    ReorderAttachmentsInput, RequestAttachmentView, SaveDraftInput, SetHostPinnedInput,
    SetHostSessionPinnedInput, SubmitFeedbackInput, WorkbenchTerminalOperations,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ApplicationHostProfileView {
    pub id: String,
    pub label: String,
    pub icon_svg: String,
    pub default_adapter: String,
    pub continuation_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ApplicationFeedbackResultView {
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ApplicationFeedbackRequestView {
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
    pub feedback: Option<ApplicationFeedbackResultView>,
    pub resolution: Option<FeedbackResolution>,
    pub allow_finish: bool,
    pub final_summary: Option<String>,
}

impl From<FeedbackRequestView> for ApplicationFeedbackRequestView {
    fn from(value: FeedbackRequestView) -> Self {
        Self {
            request_id: value.request_id,
            host_id: value.host_id,
            host_session_id: value.host_session_id,
            status: value.status,
            execution_mode: value.execution_mode,
            created_at: value.created_at,
            updated_at: value.updated_at,
            poll_after_ms: value.poll_after_ms,
            feedback: value
                .feedback
                .map(|_| ApplicationFeedbackResultView { available: true }),
            resolution: value.resolution,
            allow_finish: value.allow_finish,
            final_summary: value.final_summary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ApplicationFeedbackWorkspaceView {
    pub request: FeedbackRequestSummary,
    pub actions: Vec<ActionInput>,
    pub context_refs: Vec<ContextRef>,
    pub request_attachments: Vec<RequestAttachmentView>,
    pub draft: DraftView,
    pub attachments: Vec<AttachmentView>,
    pub feedback: Option<ApplicationFeedbackResultView>,
}

impl From<FeedbackWorkspaceView> for ApplicationFeedbackWorkspaceView {
    fn from(value: FeedbackWorkspaceView) -> Self {
        Self {
            request: value.request,
            actions: value.actions,
            context_refs: value.context_refs,
            request_attachments: value.request_attachments,
            draft: value.draft,
            attachments: value.attachments,
            feedback: value
                .feedback
                .map(|_| ApplicationFeedbackResultView { available: true }),
        }
    }
}

#[derive(Clone)]
pub struct ApplicationCommandFacade {
    agents: Option<crate::AgentManagementApplication>,
    sessions: Option<crate::SessionApplication>,
    application: FeedbackApplication,
    terminal_operations: WorkbenchTerminalOperations,
    host_profiles: Arc<[ApplicationHostProfileView]>,
}

impl ApplicationCommandFacade {
    pub fn new(
        application: FeedbackApplication,
        terminal_operations: WorkbenchTerminalOperations,
        host_profiles: Vec<ApplicationHostProfileView>,
    ) -> Self {
        Self {
            agents: None,
            sessions: None,
            application,
            terminal_operations,
            host_profiles: Arc::from(host_profiles),
        }
    }

    pub async fn list_feedback_inbox(
        &self,
    ) -> Result<Vec<FeedbackRequestSummary>, ApplicationError> {
        self.application.list_open_feedback_requests().await
    }

    pub async fn list_host_sessions(&self) -> Result<Vec<HostSessionSummary>, ApplicationError> {
        self.application.list_host_sessions().await
    }

    pub async fn list_archived_host_sessions(
        &self,
        input: ListHostSessionsInput,
    ) -> Result<Vec<HostSessionSummary>, ApplicationError> {
        self.application.list_archived_host_sessions(input).await
    }

    pub fn list_host_profiles(&self) -> Vec<ApplicationHostProfileView> {
        self.host_profiles.to_vec()
    }

    pub async fn list_feedback_requests(
        &self,
        input: ListFeedbackRequestsInput,
    ) -> Result<ListFeedbackRequestsOutput, ApplicationError> {
        self.application.list_feedback_requests(input).await
    }

    pub async fn get_feedback_workspace(
        &self,
        input: GetFeedbackInput,
    ) -> Result<ApplicationFeedbackWorkspaceView, ApplicationError> {
        self.application
            .get_feedback_workspace(input.request_id)
            .await
            .map(Into::into)
    }

    pub async fn read_published_feedback(
        &self,
        input: GetFeedbackInput,
    ) -> Result<Option<FeedbackPackageView>, ApplicationError> {
        let request = self.application.get_feedback(input).await?;
        self.application
            .read_feedback_package(&request)
            .await
            .map(|content| content.map(FeedbackPackageView::from))
    }

    pub async fn save_feedback_draft(
        &self,
        input: SaveDraftInput,
    ) -> Result<DraftView, ApplicationError> {
        self.application.save_feedback_draft(input).await
    }

    pub async fn add_feedback_attachment(
        &self,
        input: AddAttachmentInput,
    ) -> Result<ApplicationFeedbackWorkspaceView, ApplicationError> {
        self.application
            .add_feedback_attachment(input)
            .await
            .map(Into::into)
    }

    pub async fn remove_feedback_attachment(
        &self,
        input: RemoveAttachmentInput,
    ) -> Result<ApplicationFeedbackWorkspaceView, ApplicationError> {
        self.application
            .remove_feedback_attachment(input)
            .await
            .map(Into::into)
    }

    pub async fn reorder_feedback_attachments(
        &self,
        input: ReorderAttachmentsInput,
    ) -> Result<ApplicationFeedbackWorkspaceView, ApplicationError> {
        self.application
            .reorder_feedback_attachments(input)
            .await
            .map(Into::into)
    }

    pub async fn submit_feedback(
        &self,
        input: SubmitFeedbackInput,
    ) -> Result<ApplicationFeedbackRequestView, ApplicationError> {
        self.terminal_operations
            .submit_feedback(input)
            .await
            .map(Into::into)
    }

    pub async fn approve_feedback_request(
        &self,
        input: ApproveFeedbackInput,
    ) -> Result<ApplicationFeedbackRequestView, ApplicationError> {
        self.terminal_operations
            .approve_feedback(input)
            .await
            .map(Into::into)
    }

    pub async fn cancel_feedback_request(
        &self,
        input: CancelFeedbackInput,
    ) -> Result<ApplicationFeedbackRequestView, ApplicationError> {
        self.terminal_operations
            .cancel_feedback(input)
            .await
            .map(Into::into)
    }

    pub async fn rename_host_session(
        &self,
        input: RenameHostSessionInput,
    ) -> Result<HostSessionSummary, ApplicationError> {
        self.application.rename_host_session(input).await
    }

    pub async fn set_host_session_pinned(
        &self,
        input: SetHostSessionPinnedInput,
    ) -> Result<HostSessionSummary, ApplicationError> {
        self.application.set_host_session_pinned(input).await
    }

    pub async fn archive_host_session(
        &self,
        input: HostSessionInput,
    ) -> Result<HostSessionSummary, ApplicationError> {
        self.application.archive_host_session(input).await
    }

    pub async fn unarchive_host_session(
        &self,
        input: HostSessionInput,
    ) -> Result<HostSessionSummary, ApplicationError> {
        self.application.unarchive_host_session(input).await
    }

    pub async fn delete_host_session(
        &self,
        input: HostSessionInput,
    ) -> Result<(), ApplicationError> {
        self.application.delete_host_session(input).await
    }

    pub async fn delete_feedback_request(
        &self,
        input: DeleteFeedbackRequestInput,
    ) -> Result<(), ApplicationError> {
        self.application.delete_feedback_request(input).await
    }

    pub async fn set_host_pinned(
        &self,
        input: SetHostPinnedInput,
    ) -> Result<Vec<HostSessionSummary>, ApplicationError> {
        self.application.set_host_pinned(input).await
    }

    pub async fn read_feedback_attachment(
        &self,
        input: ReadAttachmentInput,
    ) -> Result<Vec<u8>, ApplicationError> {
        self.application
            .read_feedback_attachment(input.request_id, input.attachment_id)
            .await
    }

    pub async fn read_request_attachment(
        &self,
        input: ReadAttachmentInput,
    ) -> Result<Vec<u8>, ApplicationError> {
        self.application
            .read_request_attachment(input.request_id, input.attachment_id)
            .await
    }
}
