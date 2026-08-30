use rambledesk_migrate_v2_to_v3::{InspectError, inspect};
use sqlx::sqlite::SqliteConnectOptions;
mod support;

use support::{create_fixture, snapshot_tree};

#[tokio::test]
async fn inspect_classifies_the_mixed_legacy_fixture_without_writing_source() {
    let root = tempfile::tempdir().expect("fixture root");
    let database = create_fixture(root.path()).await;
    let before = tokio::fs::read(&database).await.expect("source before");
    let modified_before = std::fs::metadata(&database)
        .expect("source metadata before")
        .modified()
        .expect("source modification time before");

    let report = inspect(&database).await.expect("inspect legacy database");
    let after = tokio::fs::read(&database).await.expect("source after");
    let modified_after = std::fs::metadata(&database)
        .expect("source metadata after")
        .modified()
        .expect("source modification time after");

    assert_eq!(before, after, "inspect must not write the source database");
    assert_eq!(
        modified_before, modified_after,
        "inspect must not update the source database"
    );
    assert_eq!(report.report_schema, "rambledesk-v2-inspect-v1");
    assert_eq!(report.mode, "inspect");
    assert_eq!(report.counts.sessions_seen, 1);
    assert_eq!(report.counts.requests_seen, 7);
    assert_eq!(report.counts.drafts_seen, 3);
    assert_eq!(report.counts.waiting_requests, 1);
    assert_eq!(report.counts.in_progress_requests, 1);
    assert_eq!(report.counts.completed_readable, 1);
    assert_eq!(report.counts.completed_unreadable, 1);
    assert_eq!(report.counts.cancelled_requests, 1);
    assert_eq!(report.counts.unsupported_approval_semantics, 2);
    assert_eq!(report.counts.orphan_drafts, 1);
    assert_eq!(report.counts.records_migratable, 3);
    assert_eq!(report.counts.records_dropped, 5);

    let ids = report
        .records
        .iter()
        .map(|record| record.legacy_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "orphan-draft",
            "request-allow-finish",
            "request-approved",
            "request-cancelled",
            "request-completed-readable",
            "request-completed-unreadable",
            "request-in-progress",
            "request-waiting",
        ]
    );
}

#[tokio::test]
async fn repeated_inspection_produces_identical_json() {
    let root = tempfile::tempdir().expect("fixture root");
    let database = create_fixture(root.path()).await;

    let first = inspect(&database).await.expect("first inspect");
    let second = inspect(&database).await.expect("second inspect");
    assert_eq!(
        serde_json::to_string_pretty(&first).expect("first json"),
        serde_json::to_string_pretty(&second).expect("second json")
    );
}

#[tokio::test]
async fn inspect_rejects_a_non_empty_wal_sidecar() {
    let root = tempfile::tempdir().expect("fixture root");
    let database = create_fixture(root.path()).await;
    let wal = database.with_file_name("feedback.sqlite3-wal");
    tokio::fs::write(wal, b"active")
        .await
        .expect("non-empty WAL marker");

    let error = inspect(&database).await.expect_err("reject active WAL");
    assert!(matches!(error, InspectError::ActiveWal));
}

#[tokio::test]
async fn inspect_accepts_the_latest_successful_v2_migration() {
    let root = tempfile::tempdir().expect("fixture root");
    let database = create_fixture(root.path()).await;
    install_migration_record(&database, 10, true).await;
    let before = snapshot_tree(root.path());

    let report = inspect(&database).await.expect("accept v2 migration 10");
    assert_eq!(report.source_migration_version, Some(10));
    assert_eq!(before, snapshot_tree(root.path()));
}

#[tokio::test]
async fn inspect_rejects_a_future_v2_migration() {
    let root = tempfile::tempdir().expect("fixture root");
    let database = create_fixture(root.path()).await;
    install_migration_record(&database, 11, true).await;

    let error = inspect(&database)
        .await
        .expect_err("reject future v2 migration");
    assert!(matches!(error, InspectError::UnsupportedMigrationState));
}

#[tokio::test]
async fn inspect_rejects_a_failed_v2_migration() {
    let root = tempfile::tempdir().expect("fixture root");
    let database = create_fixture(root.path()).await;
    install_migration_record(&database, 10, false).await;

    let error = inspect(&database)
        .await
        .expect_err("reject failed v2 migration");
    assert!(matches!(error, InspectError::UnsupportedMigrationState));
}

async fn install_migration_record(database: &std::path::Path, version: i64, success: bool) {
    let options = SqliteConnectOptions::new()
        .filename(database)
        .create_if_missing(false);
    let pool = sqlx::SqlitePool::connect_with(options)
        .await
        .expect("open migration gate fixture");
    sqlx::query("CREATE TABLE _sqlx_migrations (version INTEGER, success BOOLEAN)")
        .execute(&pool)
        .await
        .expect("create migration ledger");
    sqlx::query("INSERT INTO _sqlx_migrations (version, success) VALUES (?1, ?2)")
        .bind(version)
        .bind(success)
        .execute(&pool)
        .await
        .expect("insert migration ledger row");
    pool.close().await;
}
