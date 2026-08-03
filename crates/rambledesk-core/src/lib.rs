//! Framework-independent RambleDesk domain and application contracts.
//!
//! Host profiles and continuation strategy selection live in `rambledesk-hosts`
//! so host integration cadence stays independent of core protocol changes.

mod feedback;
mod workspace;

pub use feedback::{
    ActionInput, ApplicationError, ApproveFeedbackInput, CancelFeedbackInput, Clock, ContextRef,
    ExecutionMode, FeedbackApplication, FeedbackRepository, FeedbackRequestView,
    FeedbackResolution, FeedbackResultView, FeedbackStatus, GetFeedbackInput, IdGenerator,
    NewFeedbackRequest, NewRequestAttachment, RecoverFeedbackInput, RepositoryError,
    RequestAttachmentInput, RequestFeedbackInput, StoredFeedbackRequest, SystemClock,
    UuidV7Generator,
};
pub use workspace::{
    AddAttachmentInput, AttachmentView, DraftView, FeedbackPackageAttachment,
    FeedbackPackageContent, FeedbackPackageManifest, FeedbackPackagePublisher,
    FeedbackPackageReader, FeedbackRequestQuery, FeedbackRequestSummary, FeedbackWorkspaceView,
    HostSessionSummary, ListFeedbackRequestsInput, ListFeedbackRequestsOutput,
    MAX_ATTACHMENT_BYTES, MAX_ATTACHMENT_COUNT, MAX_REQUEST_ATTACHMENT_TOTAL_BYTES, NewAttachment,
    PublishedFeedbackPackage, RemoveAttachmentInput, ReorderAttachmentsInput,
    RequestAttachmentView, SaveDraftInput, StoredFeedbackWorkspace, SubmissionAttachment,
    SubmissionPlan, SubmissionRequestAttachment, SubmitFeedbackInput,
};
