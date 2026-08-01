//! Framework-independent RambleDesk domain and application contracts.
//!
//! Host continuation / wakeup lives in `rambledesk-adapters` so adapter cadence
//! stays independent of core protocol changes.

mod feedback;
mod workspace;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use feedback::{
    ActionInput, ApplicationError, CancelFeedbackInput, Clock, ContextRef, ExecutionMode,
    FeedbackApplication, FeedbackRepository, FeedbackRequestView, FeedbackResultView,
    FeedbackStatus, GetFeedbackInput, IdGenerator, NewFeedbackRequest, ProjectInput,
    RepositoryError, RequestFeedbackInput, StoredFeedbackRequest, SystemClock, UuidV7Generator,
};
pub use workspace::{
    AddAttachmentInput, AttachmentView, DraftView, FeedbackPackagePublisher, FeedbackRequestQuery,
    FeedbackRequestSummary, FeedbackWorkspaceView, ListFeedbackRequestsInput,
    ListFeedbackRequestsOutput, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENT_COUNT, NewAttachment,
    PublishedFeedbackPackage, RemoveAttachmentInput, ReorderAttachmentsInput, SaveDraftInput,
    StoredFeedbackWorkspace, SubmissionAttachment, SubmissionPlan, SubmitFeedbackInput,
};

pub const SERVICE_NAME: &str = "rambledesk";
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ServiceStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum StorageStatus {
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct HealthSnapshot {
    pub service_name: String,
    pub service_version: String,
    pub status: ServiceStatus,
    pub storage: StorageStatus,
}

impl HealthSnapshot {
    pub fn ready() -> Self {
        Self {
            service_name: SERVICE_NAME.to_owned(),
            service_version: SERVICE_VERSION.to_owned(),
            status: ServiceStatus::Ready,
            storage: StorageStatus::Ready,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_health_is_stable_and_camel_case() {
        let value = serde_json::to_value(HealthSnapshot::ready()).expect("health serializes");
        assert_eq!(value["serviceName"], SERVICE_NAME);
        assert_eq!(value["status"], "ready");
        assert_eq!(value["storage"], "ready");
        assert!(value.get("service_name").is_none());
    }
}
