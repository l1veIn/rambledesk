use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use rambledesk_migrate_v2_to_v3::{execute, verify};
use serde_json::Value;
use sqlx::sqlite::SqliteConnectOptions;

mod support;

use support::{FileSnapshot, create_fixture, snapshot_tree};

#[cfg(unix)]
#[tokio::test]
async fn verify_rejects_an_artifact_shard_symlink_without_reading_outside_root() {
    use std::os::unix::fs::symlink;

    let fixture = published_fixture().await;
    let shards = fixture.target.join("library/artifacts/sha256");
    let shard = std::fs::read_dir(&shards)
        .expect("artifact shards")
        .map(|entry| entry.expect("artifact shard").path())
        .find(|path| path.is_dir())
        .expect("at least one artifact shard");
    let outside = fixture.output_root.path().join("outside-artifact-shard");
    std::fs::rename(&shard, &outside).expect("move real shard outside target");
    symlink(&outside, &shard).expect("replace shard with outside symlink");

    assert_invalid_or_error(&fixture.target, Some("artifact_objects")).await;
    fixture.assert_source_unchanged();
}

#[tokio::test]
async fn verify_rejects_a_submitted_request_without_its_delivery() {
    let fixture = published_fixture().await;
    let database = fixture.target.join("rambledesk-v3.sqlite3");
    let options = SqliteConnectOptions::new()
        .filename(&database)
        .create_if_missing(false)
        .foreign_keys(true);
    let pool = sqlx::SqlitePool::connect_with(options)
        .await
        .expect("open target for adversarial delivery deletion");
    let deleted = sqlx::query("DELETE FROM feedback_deliveries_v3 WHERE resolution = 'submitted'")
        .execute(&pool)
        .await
        .expect("delete submitted delivery")
        .rows_affected();
    assert_eq!(deleted, 1);
    pool.close().await;

    assert_failed_check(&fixture.target, "migration_lifecycle").await;
    fixture.assert_source_unchanged();
}

#[tokio::test]
async fn verify_rejects_a_replaced_source_backup() {
    let fixture = published_fixture().await;
    let backup = fixture.target.join("backup");
    make_writable(&backup);
    let source_backup = backup.join("source.sqlite3");
    make_writable(&source_backup);
    std::fs::write(&source_backup, b"not a SQLite backup").expect("replace source backup");

    assert_invalid_or_error(&fixture.target, Some("backup")).await;
    fixture.assert_source_unchanged();
}

#[tokio::test]
async fn verify_rejects_a_missing_migration_report() {
    let fixture = published_fixture().await;
    std::fs::remove_file(fixture.target.join("reports/migration-report.json"))
        .expect("remove migration report");

    assert_failed_check(&fixture.target, "migration_report").await;
    fixture.assert_source_unchanged();
}

#[tokio::test]
async fn verify_rejects_a_tampered_migration_report() {
    let fixture = published_fixture().await;
    let report_path = fixture.target.join("reports/migration-report.json");
    let mut report: Value =
        serde_json::from_slice(&std::fs::read(&report_path).expect("read migration report"))
            .expect("parse migration report");
    report["counts"]["sessions_created"] = Value::from(999);
    std::fs::write(
        &report_path,
        serde_json::to_vec_pretty(&report).expect("render tampered report"),
    )
    .expect("tamper migration report");

    assert_failed_check(&fixture.target, "migration_report").await;
    fixture.assert_source_unchanged();
}

struct PublishedFixture {
    source_root: tempfile::TempDir,
    output_root: tempfile::TempDir,
    target: PathBuf,
    source_before: BTreeMap<PathBuf, FileSnapshot>,
}

impl PublishedFixture {
    fn assert_source_unchanged(&self) {
        assert_eq!(self.source_before, snapshot_tree(self.source_root.path()));
    }
}

async fn published_fixture() -> PublishedFixture {
    let source_root = tempfile::tempdir().expect("source root");
    let output_root = tempfile::tempdir().expect("output root");
    let source = create_fixture(source_root.path()).await;
    let source_before = snapshot_tree(source_root.path());
    let target = output_root.path().join("migration");
    execute(&source, &target)
        .await
        .expect("publish adversarial verifier fixture");
    assert_eq!(source_before, snapshot_tree(source_root.path()));
    PublishedFixture {
        source_root,
        output_root,
        target,
        source_before,
    }
}

async fn assert_failed_check(target: &Path, check_name: &str) {
    let report = verify(target)
        .await
        .unwrap_or_else(|error| panic!("verify should return an invalid report: {error}"));
    assert!(!report.valid);
    assert!(
        report
            .checks
            .iter()
            .any(|check| check.name == check_name && !check.passed),
        "expected failed check {check_name}; checks: {:?}",
        report.checks
    );
}

async fn assert_invalid_or_error(target: &Path, expected_check: Option<&str>) {
    if let Ok(report) = verify(target).await {
        assert!(!report.valid);
        if let Some(check_name) = expected_check {
            assert!(
                report
                    .checks
                    .iter()
                    .any(|check| check.name == check_name && !check.passed),
                "expected failed check {check_name}; checks: {:?}",
                report.checks
            );
        }
    }
}

#[cfg(unix)]
fn make_writable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::symlink_metadata(path).expect("writable target metadata");
    let mode = if metadata.is_dir() { 0o755 } else { 0o644 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .expect("make adversarial target writable");
}

#[cfg(not(unix))]
fn make_writable(path: &Path) {
    let mut permissions = std::fs::metadata(path)
        .expect("writable target metadata")
        .permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions).expect("make adversarial target writable");
}
