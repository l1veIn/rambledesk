use std::path::{Path, PathBuf};

use rambledesk_migrate_v2_to_v3::{InspectError, inspect};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteConnectOptions;

async fn create_fixture(root: &Path) -> PathBuf {
    let database = root.join("feedback.sqlite3");
    let options = SqliteConnectOptions::new()
        .filename(&database)
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(options)
        .await
        .expect("create fixture database");
    sqlx::raw_sql(include_str!("fixtures/mixed-v2.sql"))
        .execute(&pool)
        .await
        .expect("install mixed v2 fixture");

    let readable = root.join("packages").join("readable");
    tokio::fs::create_dir_all(&readable)
        .await
        .expect("readable package directory");
    let feedback = b"Structured human feedback.\n";
    let uncooked = b"Original ramble.\n";
    tokio::fs::write(readable.join("feedback.md"), feedback)
        .await
        .expect("feedback");
    tokio::fs::write(readable.join("uncooked.md"), uncooked)
        .await
        .expect("uncooked");
    let manifest = serde_json::to_string_pretty(&json!({
        "schema_version": 1,
        "request_id": "request-completed-readable",
        "feedback_markdown": "feedback.md",
        "feedback_sha256": sha256_hex(feedback),
        "uncooked_markdown": "uncooked.md",
        "uncooked_sha256": sha256_hex(uncooked),
        "attachments": []
    }))
    .expect("manifest json")
        + "\n";
    tokio::fs::write(readable.join("manifest.json"), manifest.as_bytes())
        .await
        .expect("manifest");
    insert_result(
        &pool,
        "request-completed-readable",
        &readable,
        &sha256_hex(manifest.as_bytes()),
    )
    .await;

    let missing = root.join("packages").join("missing");
    insert_result(
        &pool,
        "request-completed-unreadable",
        &missing,
        &"0".repeat(64),
    )
    .await;
    pool.close().await;
    database
}

async fn insert_result(
    pool: &sqlx::SqlitePool,
    request_id: &str,
    directory: &Path,
    manifest_sha256: &str,
) {
    sqlx::query(
        "INSERT INTO feedback_results \
         (request_id, package_uri, directory_path, markdown_path, manifest_path, manifest_sha256, published_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, '2026-08-01T08:00:00Z')",
    )
    .bind(request_id)
    .bind(format!("rambledesk://feedback/{request_id}"))
    .bind(directory.to_string_lossy().as_ref())
    .bind(directory.join("feedback.md").to_string_lossy().as_ref())
    .bind(directory.join("manifest.json").to_string_lossy().as_ref())
    .bind(manifest_sha256)
    .execute(pool)
    .await
    .expect("insert feedback result");
}

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
    assert_eq!(report.counts.drafts_seen, 2);
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

fn sha256_hex(contents: &[u8]) -> String {
    hex::encode(Sha256::digest(contents))
}
