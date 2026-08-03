use super::*;
use rambledesk_core::{
    ActionInput, AddAttachmentInput, ApproveFeedbackInput, CancelFeedbackInput, ContextRef,
    ExecutionMode, FeedbackResolution, FeedbackStatus, GetFeedbackInput, ListFeedbackRequestsInput,
    RecoverFeedbackInput, RemoveAttachmentInput, ReorderAttachmentsInput, RequestAttachmentInput,
    RequestFeedbackInput, SaveDraftInput, SubmitFeedbackInput,
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
            attachments: Vec::new(),
            source_hint: Some("storage test fixture".to_owned()),
            allow_finish: false,
            final_summary: None,
        }
    }
}

#[tokio::test]
async fn reconnect_creates_a_versioned_pre_migration_backup() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("initial database");
    store.close().await;

    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("reopened database");
    store.close().await;

    let parent = workspace.database.parent().expect("database parent");
    let backup = parent.join(format!(
        "rambledesk.pre-migration-v{}.sqlite3",
        env!("CARGO_PKG_VERSION")
    ));
    assert!(backup.is_file(), "expected backup at {}", backup.display());
    assert!(backup.metadata().expect("backup metadata").len() > 0);
}

mod publication;
mod requests;
mod workspace;
