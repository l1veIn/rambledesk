use super::*;
use rambledesk_core::{
    ActionInput, AddAttachmentInput, CancelFeedbackInput, ContextRef, ExecutionMode,
    FeedbackStatus, GetFeedbackInput, ListFeedbackRequestsInput, RemoveAttachmentInput,
    ReorderAttachmentsInput, RequestFeedbackInput, SaveDraftInput, SubmitFeedbackInput,
};
use tempfile::TempDir;
use uuid::Uuid;

struct TestWorkspace {
    _temp: TempDir,
    database: std::path::PathBuf,
}

impl TestWorkspace {
    async fn new() -> Self {
        let temp = tempfile::tempdir().expect("temporary directory");
        Self {
            database: temp.path().join("state").join("rambledesk.sqlite3"),
            _temp: temp,
        }
    }

    fn request(&self, request_id: String) -> RequestFeedbackInput {
        RequestFeedbackInput {
            request_id: Some(request_id),
            host_id: "test-host".to_owned(),
            host_session_id: "test-session".to_owned(),
            title: Some("Persistence review".to_owned()),
            what_happened: "Implemented the persistence kernel.".to_owned(),
            actions: vec![ActionInput {
                id: "review".to_owned(),
                instruction: "Review the implementation.".to_owned(),
            }],
            context_refs: vec![ContextRef {
                label: "diff".to_owned(),
                uri: "file:///tmp/change.diff".to_owned(),
            }],
            source_hint: Some("storage test fixture".to_owned()),
        }
    }
}

mod publication;
mod requests;
mod workspace;
