use super::*;
use rambledesk_core::{
    ActionInput, AddAttachmentInput, ApproveFeedbackInput, CancelFeedbackInput, ContextRef,
    DeleteFeedbackRequestInput, ExecutionMode, FeedbackResolution, FeedbackStatus,
    GetFeedbackInput, HostSessionInput, ListFeedbackRequestsInput, ListHostSessionsInput,
    RecoverFeedbackInput, RemoveAttachmentInput, RenameHostSessionInput, ReorderAttachmentsInput,
    RequestAttachmentInput, RequestFeedbackInput, SaveDraftInput, SetHostPinnedInput,
    SetHostSessionPinnedInput, SubmitFeedbackInput,
};
use sha2::{Digest, Sha384};
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
            host_id: Some("test-host".to_owned()),
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

#[tokio::test]
async fn connect_rejects_a_database_created_by_a_newer_version() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("initial database");
    store.close().await;

    // Simulate a database migrated by a newer app build by recording an
    // applied migration version above the embedded migration set.
    let url = format!("sqlite://{}", workspace.database.display());
    let pool = sqlx::SqlitePool::connect(&url)
        .await
        .expect("open database");
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) \
         VALUES (9999, 'future migration', datetime('now'), 1, X'00', 0)",
    )
    .execute(&pool)
    .await
    .expect("record future migration");
    pool.close().await;

    let outcome = SqliteFeedbackStore::connect(&workspace.database).await;
    let error = match outcome {
        Err(error) => error,
        Ok(_) => panic!("a database from a newer version must be rejected"),
    };
    match error {
        StorageOpenError::NewerDatabase { applied, supported } => {
            assert_eq!(applied, 9999);
            assert!(supported >= 1, "supported migrations are recorded");
        }
        other => panic!("expected NewerDatabase, got {other}"),
    }
}

#[tokio::test]
async fn reconnect_repairs_migration_checksums_changed_only_by_line_endings() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("initial database");
    store.close().await;

    let migration = MIGRATOR.iter().next().expect("embedded migration");
    let normalized = migration.sql.replace("\r\n", "\n").replace('\r', "\n");
    let alternate = if migration.sql.contains("\r\n") {
        normalized
    } else {
        normalized.replace('\n', "\r\n")
    };
    let alternate_checksum = Sha384::digest(alternate.as_bytes());
    let options = SqliteConnectOptions::new().filename(&workspace.database);
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("test database");
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ?2 WHERE version = ?1")
        .bind(migration.version)
        .bind(alternate_checksum.as_slice())
        .execute(&pool)
        .await
        .expect("alternate checksum");
    pool.close().await;

    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("database with line-ending-only checksum difference");
    store.close().await;

    let options = SqliteConnectOptions::new().filename(&workspace.database);
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("reopened test database");
    let repaired: Vec<u8> =
        sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = ?1")
            .bind(migration.version)
            .fetch_one(&pool)
            .await
            .expect("repaired checksum");
    assert_eq!(repaired, migration.checksum.as_ref());
    pool.close().await;
}

#[tokio::test]
async fn reconnect_rejects_arbitrary_migration_checksum_changes() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("initial database");
    store.close().await;

    let migration = MIGRATOR.iter().next().expect("embedded migration");
    let options = SqliteConnectOptions::new().filename(&workspace.database);
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("test database");
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ?2 WHERE version = ?1")
        .bind(migration.version)
        .bind(vec![0_u8; 48])
        .execute(&pool)
        .await
        .expect("invalid checksum");
    pool.close().await;

    let error = match SqliteFeedbackStore::connect(&workspace.database).await {
        Ok(_) => panic!("arbitrary migration edits must still fail"),
        Err(error) => error,
    };
    assert!(matches!(error, StorageOpenError::Migrate(_)));
}

mod document_json_migration;
mod publication;
mod requests;
mod workspace;
