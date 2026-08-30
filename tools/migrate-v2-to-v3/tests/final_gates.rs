use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    path::{Path, PathBuf},
};

use rambledesk_core::kernel::{
    ArtifactInput, FeedbackSubmission, RequestId, SubmissionId,
    calculate_feedback_submission_digest,
};
use rambledesk_migrate_v2_to_v3::{MigrationReport, execute, verify};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{Row, sqlite::SqliteConnectOptions};

mod support;

use support::{FileSnapshot, create_fixture, sha256_hex, snapshot_tree};

const DRAFT_BYTES: &[u8] = b"Draft screenshot bytes.\n";
const FEEDBACK_BYTES: &[u8] = b"Structured human feedback.\n";
const PACKAGE_ATTACHMENT_BYTES: &[u8] = b"Legacy screenshot bytes.\n";

#[tokio::test]
async fn more_than_twenty_draft_entries_are_stably_truncated_before_reading() {
    let source_root = tempfile::tempdir().expect("source root");
    let source = create_fixture(source_root.path()).await;
    let pool = open_database(&source, false).await;
    let draft_path = source_root
        .path()
        .join("draft-library/draft-screenshot.png");
    for position in 2_i64..=20 {
        sqlx::query(
            "INSERT INTO attachments \
             (id, request_id, draft_path, published_path, file_name, byte_size, media_type, \
              sha256, position, created_at) \
             VALUES (?1, 'request-waiting', ?2, NULL, ?3, ?4, 'image/png', ?5, ?6, \
                     '2026-08-01T00:23:00Z')",
        )
        .bind(format!("draft-extra-{position:02}"))
        .bind(draft_path.to_string_lossy().as_ref())
        .bind(format!("draft-extra-{position:02}.png"))
        .bind(DRAFT_BYTES.len() as i64)
        .bind(sha256_hex(DRAFT_BYTES))
        .bind(position)
        .execute(&pool)
        .await
        .expect("insert excess Draft path entry");
    }
    pool.close().await;

    let run = execute_source(source_root, source).await;
    assert_loss(&run.report, "request-waiting", "draft_artifacts_truncated");
    let pool = open_target(&run.target).await;
    let positions: Vec<i64> = sqlx::query_scalar(
        "SELECT artifacts.position FROM draft_artifacts_v3 artifacts \
         JOIN ramble_drafts_v3 drafts ON drafts.draft_id = artifacts.draft_id \
         WHERE drafts.request_id = 'request-waiting' ORDER BY artifacts.position",
    )
    .fetch_all(&pool)
    .await
    .expect("truncated Draft positions");
    pool.close().await;
    assert_eq!(positions, (0_i64..20).collect::<Vec<_>>());
    assert_valid(&run.target).await;
    run.assert_source_unchanged();
}

#[tokio::test]
async fn draft_group_over_sixty_mib_is_dropped_before_path_reads() {
    const FILE_BYTES: u64 = 16 * 1024 * 1024;

    let source_root = tempfile::tempdir().expect("source root");
    let source = create_fixture(source_root.path()).await;
    let large_path = source_root.path().join("draft-library/large-sparse.bin");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&large_path)
        .expect("create sparse Draft artifact")
        .set_len(FILE_BYTES)
        .expect("size sparse Draft artifact");
    let digest = zero_digest(FILE_BYTES as usize);
    let pool = open_database(&source, false).await;
    for position in 2_i64..=5 {
        sqlx::query(
            "INSERT INTO attachments \
             (id, request_id, draft_path, published_path, file_name, byte_size, media_type, \
              sha256, position, created_at) \
             VALUES (?1, 'request-waiting', ?2, NULL, ?3, ?4, 'application/octet-stream', \
                     ?5, ?6, '2026-08-01T00:24:00Z')",
        )
        .bind(format!("draft-large-{position}"))
        .bind(large_path.to_string_lossy().as_ref())
        .bind(format!("draft-large-{position}.bin"))
        .bind(FILE_BYTES as i64)
        .bind(&digest)
        .bind(position)
        .execute(&pool)
        .await
        .expect("insert aggregate-overflow Draft path entry");
    }
    pool.close().await;

    let run = execute_source(source_root, source).await;
    assert_loss(
        &run.report,
        "request-waiting",
        "draft_artifacts_total_exceeded",
    );
    let pool = open_target(&run.target).await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM draft_artifacts_v3 artifacts \
         JOIN ramble_drafts_v3 drafts ON drafts.draft_id = artifacts.draft_id \
         WHERE drafts.request_id = 'request-waiting'",
    )
    .fetch_one(&pool)
    .await
    .expect("dropped Draft artifact count");
    pool.close().await;
    assert_eq!(count, 0);
    assert_valid(&run.target).await;
    run.assert_source_unchanged();
}

#[tokio::test]
async fn migrated_feedback_submission_digest_exactly_matches_the_core_contract() {
    let source_root = tempfile::tempdir().expect("source root");
    let source = create_fixture(source_root.path()).await;
    let package = source_root.path().join("packages/readable");
    let manifest_path = package.join("manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read legacy manifest"))
            .expect("parse legacy manifest");
    let manifest_object = manifest.as_object_mut().expect("legacy manifest object");
    manifest_object.remove("uncooked_markdown");
    manifest_object.remove("uncooked_sha256");
    let manifest_bytes =
        serde_json::to_string_pretty(&manifest).expect("render legacy manifest") + "\n";
    std::fs::write(&manifest_path, manifest_bytes.as_bytes())
        .expect("remove legacy uncooked fields");
    let source_pool = open_database(&source, false).await;
    sqlx::query(
        "UPDATE feedback_results SET manifest_sha256 = ?1 \
         WHERE request_id = 'request-completed-readable'",
    )
    .bind(sha256_hex(manifest_bytes.as_bytes()))
    .execute(&source_pool)
    .await
    .expect("update legacy manifest digest");
    source_pool.close().await;

    let run = execute_source(source_root, source).await;
    assert_loss(
        &run.report,
        "request-completed-readable",
        "submitted_uncooked_synthesized",
    );
    let pool = open_target(&run.target).await;
    let row = sqlx::query(
        "SELECT submission_id, request_id, document_json, body_markdown, submission_digest \
         FROM ramble_submissions_v3 WHERE intent = 'feedback'",
    )
    .fetch_one(&pool)
    .await
    .expect("Feedback Submission row");
    let submission_id: String = row.get("submission_id");
    let request_id: String = row.get("request_id");
    let document_json: String = row.get("document_json");
    let feedback_markdown: String = row.get("body_markdown");
    let stored_digest: String = row.get("submission_digest");
    let artifact_rows = sqlx::query(
        "SELECT display_name, media_type, storage_key FROM submission_artifacts_v3 \
         WHERE submission_id = ?1 ORDER BY position",
    )
    .bind(&submission_id)
    .fetch_all(&pool)
    .await
    .expect("Feedback Submission artifacts");
    assert_eq!(artifact_rows.len(), 1);
    let artifact = &artifact_rows[0];
    let storage_key: String = artifact.get("storage_key");
    assert_eq!(artifact.get::<String, _>("display_name"), "evidence.txt");
    assert_eq!(
        artifact.get::<String, _>("media_type"),
        "application/octet-stream"
    );
    assert_eq!(
        std::fs::read(run.target.join("library/artifacts").join(storage_key))
            .expect("Feedback Submission Artifact bytes"),
        PACKAGE_ATTACHMENT_BYTES
    );
    let uncooked_key: String =
        sqlx::query_scalar("SELECT storage_key FROM package_artifacts_v3 WHERE role = 'uncooked'")
            .fetch_one(&pool)
            .await
            .expect("uncooked Package Artifact");
    pool.close().await;
    let uncooked_markdown = String::from_utf8(
        std::fs::read(run.target.join("library/artifacts").join(uncooked_key))
            .expect("uncooked Markdown bytes"),
    )
    .expect("valid synthesized uncooked Markdown");
    let expected_feedback = String::from_utf8(FEEDBACK_BYTES.to_vec()).expect("fixture feedback");
    let expected_document = serde_json::json!({
        "schemaVersion": 2,
        "doc": {
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": expected_feedback}]
            }]
        }
    })
    .to_string();
    assert_eq!(request_id, "request-completed-readable");
    assert_eq!(document_json, expected_document);
    assert_eq!(feedback_markdown, expected_feedback);
    assert_eq!(uncooked_markdown, expected_feedback);
    let canonical = FeedbackSubmission {
        submission_id: SubmissionId::new(submission_id),
        request_id: RequestId::new(request_id),
        expected_draft_revision: 0,
        submission_digest_assertion: None,
        document_json,
        uncooked_markdown,
        feedback_markdown,
        cooking_model: None,
        artifacts: vec![ArtifactInput {
            display_name: "evidence.txt".to_owned(),
            media_type: "application/octet-stream".to_owned(),
            contents: PACKAGE_ATTACHMENT_BYTES.to_vec(),
        }],
    };
    assert_eq!(
        stored_digest,
        calculate_feedback_submission_digest(&canonical)
    );
    assert_valid(&run.target).await;
    run.assert_source_unchanged();
}

#[tokio::test]
async fn submitted_package_with_invalid_utf8_feedback_is_dropped_with_loss() {
    let source_root = tempfile::tempdir().expect("source root");
    let source = create_fixture(source_root.path()).await;
    let package = source_root.path().join("packages/readable");
    let invalid_feedback = [0xff, 0xfe, 0xfd, b'\n'];
    std::fs::write(package.join("feedback.md"), invalid_feedback)
        .expect("write invalid UTF-8 legacy feedback");
    let manifest_path = package.join("manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read legacy manifest"))
            .expect("parse legacy manifest");
    manifest["feedback_sha256"] = Value::from(sha256_hex(&invalid_feedback));
    let manifest_bytes =
        serde_json::to_string_pretty(&manifest).expect("render legacy manifest") + "\n";
    std::fs::write(&manifest_path, manifest_bytes.as_bytes()).expect("update legacy manifest");
    let pool = open_database(&source, false).await;
    sqlx::query(
        "UPDATE feedback_results SET manifest_sha256 = ?1 \
         WHERE request_id = 'request-completed-readable'",
    )
    .bind(sha256_hex(manifest_bytes.as_bytes()))
    .execute(&pool)
    .await
    .expect("update legacy manifest digest");
    pool.close().await;

    let run = execute_source(source_root, source).await;
    assert_loss(
        &run.report,
        "request-completed-readable",
        "submitted_feedback_invalid_utf8",
    );
    assert_eq!(run.report.counts.submitted_requests_migrated, 0);
    let pool = open_target(&run.target).await;
    let request_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM feedback_requests_v3 \
         WHERE request_id = 'request-completed-readable'",
    )
    .fetch_one(&pool)
    .await
    .expect("invalid UTF-8 Request count");
    let package_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM packages_v3")
        .fetch_one(&pool)
        .await
        .expect("invalid UTF-8 Package count");
    pool.close().await;
    assert_eq!(request_count, 0);
    assert_eq!(package_count, 0);
    assert_valid(&run.target).await;
    run.assert_source_unchanged();
}

#[tokio::test]
async fn verify_rejects_a_failed_exact_v3_migration() {
    let run = normal_run().await;
    let pool = open_database(&run.target.join("rambledesk-v3.sqlite3"), false).await;
    sqlx::query("UPDATE _sqlx_migrations SET success = FALSE WHERE version = 3001")
        .execute(&pool)
        .await
        .expect("mark migration 3001 failed");
    pool.close().await;

    assert_failed_schema_check(&run.target).await;
    run.assert_source_unchanged();
}

#[tokio::test]
async fn verify_rejects_a_future_v3_migration() {
    let run = normal_run().await;
    let pool = open_database(&run.target.join("rambledesk-v3.sqlite3"), false).await;
    sqlx::query(
        "INSERT INTO _sqlx_migrations \
         (version, description, installed_on, success, checksum, execution_time) \
         SELECT 3002, 'future', installed_on, TRUE, checksum, execution_time \
         FROM _sqlx_migrations WHERE version = 3001",
    )
    .execute(&pool)
    .await
    .expect("inject future migration 3002");
    pool.close().await;

    assert_failed_schema_check(&run.target).await;
    run.assert_source_unchanged();
}

struct Run {
    source_root: tempfile::TempDir,
    _output_root: tempfile::TempDir,
    target: PathBuf,
    report: MigrationReport,
    source_before: BTreeMap<PathBuf, FileSnapshot>,
}

impl Run {
    fn assert_source_unchanged(&self) {
        assert_eq!(self.source_before, snapshot_tree(self.source_root.path()));
    }
}

async fn normal_run() -> Run {
    let source_root = tempfile::tempdir().expect("source root");
    let source = create_fixture(source_root.path()).await;
    execute_source(source_root, source).await
}

async fn execute_source(source_root: tempfile::TempDir, source: PathBuf) -> Run {
    let source_before = snapshot_tree(source_root.path());
    let output_root = tempfile::tempdir().expect("output root");
    let target = output_root.path().join("migration");
    let report = execute(&source, &target)
        .await
        .expect("final-gate fixture migration");
    assert_eq!(source_before, snapshot_tree(source_root.path()));
    Run {
        source_root,
        _output_root: output_root,
        target,
        report,
        source_before,
    }
}

async fn open_database(path: &Path, read_only: bool) -> sqlx::SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .read_only(read_only)
        .immutable(read_only);
    sqlx::SqlitePool::connect_with(options)
        .await
        .expect("open SQLite fixture")
}

async fn open_target(target: &Path) -> sqlx::SqlitePool {
    open_database(&target.join("rambledesk-v3.sqlite3"), true).await
}

fn zero_digest(size: usize) -> String {
    let block = [0_u8; 8192];
    let mut digest = Sha256::new();
    for _ in 0..(size / block.len()) {
        digest.update(block);
    }
    digest.update(&block[..size % block.len()]);
    hex::encode(digest.finalize())
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

async fn assert_valid(target: &Path) {
    let report = verify(target).await.expect("verify final-gate target");
    assert!(report.valid, "target invalid: {:?}", report.checks);
}

async fn assert_failed_schema_check(target: &Path) {
    let report = verify(target)
        .await
        .expect("verify should report the schema failure");
    assert!(!report.valid);
    assert!(
        report
            .checks
            .iter()
            .any(|check| check.name == "schema_generation" && !check.passed),
        "schema_generation did not reject the migration ledger: {:?}",
        report.checks
    );
}
