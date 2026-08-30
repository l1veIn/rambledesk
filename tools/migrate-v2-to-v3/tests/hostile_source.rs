use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use rambledesk_migrate_v2_to_v3::{MigrationReport, execute, verify};
use sqlx::sqlite::SqliteConnectOptions;

mod support;

use support::{FileSnapshot, create_fixture, snapshot_tree};

#[tokio::test]
async fn verify_rejects_a_non_empty_target_database_wal() {
    let source_root = tempfile::tempdir().expect("source root");
    let source = create_fixture(source_root.path()).await;
    let run = execute_source(source_root, source).await;
    std::fs::write(
        run.target.join("rambledesk-v3.sqlite3-wal"),
        b"uncheckpointed target state",
    )
    .expect("non-empty target WAL");

    assert_verify_non_success(&run.target).await;
    run.assert_source_unchanged();
}

#[tokio::test]
async fn colliding_normalized_action_ids_remain_unique_and_report_the_loss() {
    let source_root = tempfile::tempdir().expect("source root");
    let source = create_fixture(source_root.path()).await;
    let pool = open_for_mutation(&source).await;
    sqlx::query(
        "UPDATE request_actions SET action_id = 'legacy-action-1' \
         WHERE request_id = 'request-waiting' AND position = 0",
    )
    .execute(&pool)
    .await
    .expect("install valid colliding action id");
    sqlx::query(
        "UPDATE request_actions SET action_id = 'INVALID ACTION' \
         WHERE request_id = 'request-waiting' AND position = 1",
    )
    .execute(&pool)
    .await
    .expect("install action id that normalizes into the collision");
    pool.close().await;

    let run = execute_source(source_root, source).await;
    assert_loss(&run.report, "request-waiting", "action_ids_normalized");
    let pool = open_target_read_only(&run.target).await;
    let actions: Vec<(String, String)> = sqlx::query_as(
        "SELECT action_id, instruction FROM feedback_request_actions_v3 \
         WHERE request_id = 'request-waiting' ORDER BY position",
    )
    .fetch_all(&pool)
    .await
    .expect("migrated actions");
    pool.close().await;
    assert_eq!(actions.len(), 20);
    assert_eq!(
        actions
            .iter()
            .map(|(action_id, _)| action_id)
            .collect::<BTreeSet<_>>()
            .len(),
        actions.len()
    );
    assert!(
        actions
            .iter()
            .any(|(_, instruction)| instruction == "Review the proposed implementation")
    );
    assert!(
        actions
            .iter()
            .any(|(_, instruction)| instruction == "Additional legacy action 01")
    );
    assert_published_target_valid(&run.target).await;
    run.assert_source_unchanged();
}

#[tokio::test]
async fn colliding_draft_attachment_positions_are_stably_reenumerated() {
    let source_root = tempfile::tempdir().expect("source root");
    let source = create_fixture(source_root.path()).await;
    let pool = open_for_mutation(&source).await;
    sqlx::query("UPDATE attachments SET position = -1 WHERE id = 'draft-attachment-blank'")
        .execute(&pool)
        .await
        .expect("install negative colliding Draft attachment position");
    pool.close().await;

    let run = execute_source(source_root, source).await;
    assert_loss(
        &run.report,
        "request-waiting",
        "draft_artifact_positions_normalized",
    );
    let pool = open_target_read_only(&run.target).await;
    let artifacts: Vec<(i64, String)> = sqlx::query_as(
        "SELECT artifacts.position, artifacts.display_name \
         FROM draft_artifacts_v3 artifacts \
         JOIN ramble_drafts_v3 drafts ON drafts.draft_id = artifacts.draft_id \
         WHERE drafts.request_id = 'request-waiting' ORDER BY artifacts.position",
    )
    .fetch_all(&pool)
    .await
    .expect("migrated Draft artifacts");
    pool.close().await;
    assert_eq!(
        artifacts,
        vec![
            (0, "attachment.bin".to_owned()),
            (1, "draft-screenshot.png".to_owned()),
        ]
    );
    assert_published_target_valid(&run.target).await;
    run.assert_source_unchanged();
}

#[tokio::test]
async fn oversized_inline_attachment_is_dropped_before_materialization() {
    const OVERSIZED_BYTES: i64 = 20 * 1024 * 1024 + 1;

    let source_root = tempfile::tempdir().expect("source root");
    let source = create_fixture(source_root.path()).await;
    let pool = open_for_mutation(&source).await;
    sqlx::query(
        "INSERT INTO request_attachments \
         (id, request_id, file_name, byte_size, media_type, sha256, position, contents, \
          created_at, draft_path, published_path) \
         VALUES ('oversized-blob', 'request-waiting', 'oversized.bin', ?1, \
                 'application/octet-stream', ?2, 2, zeroblob(?1), \
                 '2026-08-01T00:22:00Z', NULL, NULL)",
    )
    .bind(OVERSIZED_BYTES)
    .bind("0".repeat(64))
    .execute(&pool)
    .await
    .expect("install oversized inline BLOB without allocating it in the test process");
    pool.close().await;

    let run = execute_source(source_root, source).await;
    assert_loss(
        &run.report,
        "request-waiting:oversized-blob",
        "oversized_attachment_dropped",
    );
    let pool = open_target_read_only(&run.target).await;
    let oversized_entries: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM feedback_request_artifacts_v3 \
         WHERE display_name = 'oversized.bin'",
    )
    .fetch_one(&pool)
    .await
    .expect("oversized target entry count");
    let request_entries: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM feedback_request_artifacts_v3")
            .fetch_one(&pool)
            .await
            .expect("request Artifact Entry count");
    pool.close().await;
    assert_eq!(oversized_entries, 0);
    assert_eq!(request_entries, 3);
    assert_eq!(run.report.counts.artifacts_migrated, 6);
    assert_published_target_valid(&run.target).await;
    run.assert_source_unchanged();
}

struct MigrationRun {
    source_root: tempfile::TempDir,
    _output_root: tempfile::TempDir,
    target: PathBuf,
    report: MigrationReport,
    source_before: BTreeMap<PathBuf, FileSnapshot>,
}

impl MigrationRun {
    fn assert_source_unchanged(&self) {
        assert_eq!(self.source_before, snapshot_tree(self.source_root.path()));
    }
}

async fn execute_source(source_root: tempfile::TempDir, source: PathBuf) -> MigrationRun {
    let source_before = snapshot_tree(source_root.path());
    let output_root = tempfile::tempdir().expect("output root");
    let target = output_root.path().join("migration");
    let report = execute(&source, &target)
        .await
        .expect("hostile source should migrate with explicit normalization");
    assert_eq!(source_before, snapshot_tree(source_root.path()));
    MigrationRun {
        source_root,
        _output_root: output_root,
        target,
        report,
        source_before,
    }
}

async fn open_for_mutation(path: &std::path::Path) -> sqlx::SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false);
    sqlx::SqlitePool::connect_with(options)
        .await
        .expect("open hostile source fixture")
}

async fn open_target_read_only(target: &std::path::Path) -> sqlx::SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(target.join("rambledesk-v3.sqlite3"))
        .create_if_missing(false)
        .read_only(true)
        .immutable(true);
    sqlx::SqlitePool::connect_with(options)
        .await
        .expect("open migrated target read-only")
}

fn assert_loss(report: &MigrationReport, legacy_id: &str, reason: &str) {
    assert!(
        report
            .losses
            .iter()
            .any(|loss| loss.legacy_id == legacy_id && loss.reason == reason),
        "missing loss {legacy_id}: {reason}; available: {:?}",
        report.losses
    );
}

async fn assert_published_target_valid(target: &std::path::Path) {
    let report = verify(target).await.expect("verify normalized target");
    assert!(
        report.valid,
        "normalized target invalid: {:?}",
        report.checks
    );
}

async fn assert_verify_non_success(target: &std::path::Path) {
    if let Ok(report) = verify(target).await {
        assert!(!report.valid, "verify ignored a non-empty target WAL");
    }
}
