//! Framework-independent RambleDesk domain and application contracts.
//!
//! Host profiles and continuation strategy selection live in `rambledesk-hosts`
//! so host integration cadence stays independent of core protocol changes.

mod feedback;
mod process;
mod workspace;

/// Install-time / client-config host identity environment key.
pub const HOST_ENV_KEY: &str = "RAMBLEDESK_HOST";
/// HTTP header mirror so the loopback server can see the installed host id.
pub const HOST_HEADER: &str = "x-rambledesk-host";

pub use process::{find_executable, find_executable_on_path};

pub use feedback::{
    ActionInput, ApplicationError, ApproveFeedbackInput, AttachmentPathResolver,
    CancelFeedbackInput, Clock, ContextRef, ExecutionMode, FeedbackApplication, FeedbackRepository,
    FeedbackRequestView, FeedbackResolution, FeedbackResultView, FeedbackStatus, GetFeedbackInput,
    IdGenerator, NewFeedbackRequest, NewRequestAttachment, RecoverFeedbackInput, RepositoryError,
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
