use std::{collections::BTreeSet, path::Path};

use rambledesk_migrate_v2_to_v3::{
    MigrationError, MigrationReport, dry_run, execute, inspect, verify,
};
use rambledesk_storage::v3::SqliteV3Store;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteConnectOptions;

mod support;

use support::{add_unsafe_parent_package, create_fixture, snapshot_tree};

const FEEDBACK: &[u8] = b"Structured human feedback.\n";
const UNCOOKED: &[u8] = b"Original ramble.\n";
const PACKAGE_ATTACHMENT: &[u8] = b"Legacy screenshot bytes.\n";
const COMPLETED_REQUEST_ATTACHMENT: &[u8] = b"Completed request context.\n";
const DRAFT_ATTACHMENT: &[u8] = b"Draft screenshot bytes.\n";
const WAITING_REQUEST_ATTACHMENT: &[u8] = b"Request context attachment.\n";

#[tokio::test]
async fn dry_run_is_read_only_deterministic_and_requires_a_new_target() {
    let source_root = tempfile::tempdir().expect("source root");
    let output_root = tempfile::tempdir().expect("output root");
    let source = create_fixture(source_root.path()).await;
    let target = output_root.path().join("migration");
    let source_before = snapshot_tree(source_root.path());

    let first = dry_run(&source, &target).await.expect("first dry run");
    let second = dry_run(&source, &target).await.expect("second dry run");

    assert!(!target.exists(), "dry-run must not create the target root");
    assert_eq!(source_before, snapshot_tree(source_root.path()));
    assert_eq!(first.mode, "dry_run");
    assert_eq!(first.report_schema, "rambledesk-v2-to-v3-migration-v1");
    assert_eq!(first.source_schema, "v2");
    assert_eq!(first.target_schema, "v3");
    assert!(first.outputs.is_none());
    assert_eq!(logical_report(&first), logical_report(&second));
    assert_eq!(first.counts.sessions_created, 1);
    assert_eq!(first.counts.waiting_requests_migrated, 2);
    assert_eq!(first.counts.submitted_requests_migrated, 1);
    assert_eq!(first.counts.drafts_migrated, 2);
    assert_eq!(first.counts.artifacts_migrated, 6);
    assert_eq!(first.counts.records_dropped, 5);
    assert_loss(
        &first,
        "request-in-progress",
        "in_progress_state_collapsed_to_waiting",
    );
    assert_loss(&first, "request-in-progress", "missing_actions_synthesized");
    assert_loss(&first, "request-in-progress", "blank_action_dropped");
    assert_loss(&first, "request-in-progress", "blank_context_ref_dropped");
    assert_loss(&first, "request-waiting", "actions_truncated");
    assert_loss(
        &first,
        "request-waiting:draft-attachment-blank",
        "attachment_metadata_synthesized",
    );
    assert_loss(
        &first,
        "request-completed-readable",
        "missing_actions_synthesized",
    );
    assert_loss(
        &first,
        "request-completed-unreadable",
        "completed_package_unreadable",
    );
    assert_loss(&first, "request-cancelled", "cancelled_request");
    assert_loss(&first, "request-approved", "unsupported_approval_semantics");
    assert_loss(
        &first,
        "request-allow-finish",
        "unsupported_approval_semantics",
    );
    assert_loss(&first, "orphan-draft", "orphan_draft");
    assert!(first.losses.windows(2).all(|pair| {
        (&pair[0].legacy_id, &pair[0].reason) <= (&pair[1].legacy_id, &pair[1].reason)
    }));

    tokio::fs::create_dir(&target)
        .await
        .expect("existing target marker");
    let error = dry_run(&source, &target)
        .await
        .expect_err("dry-run must reject an existing target");
    assert!(matches!(error, MigrationError::TargetExists));
    assert_eq!(source_before, snapshot_tree(source_root.path()));
    assert!(
        snapshot_tree(&target).is_empty(),
        "target rejection must not add files"
    );
}

#[tokio::test]
async fn execute_atomically_maps_legacy_facts_and_verify_is_read_only() {
    let source_root = tempfile::tempdir().expect("source root");
    let output_root = tempfile::tempdir().expect("output root");
    let source = create_fixture(source_root.path()).await;
    let source_before = snapshot_tree(source_root.path());
    let target = output_root.path().join("first");

    let report = execute(&source, &target).await.expect("execute migration");
    assert_eq!(source_before, snapshot_tree(source_root.path()));
    assert_eq!(report.mode, "execute");
    assert!(report.outputs.is_some());
    assert_fixed_layout(&target);

    let database = target.join("rambledesk-v3.sqlite3");
    let projection = read_projection(&database).await;
    assert_eq!(projection.sessions.len(), 1);
    assert_eq!(projection.sessions[0].1, "imported");
    assert!(projection.legacy_business_columns.is_empty());
    assert_eq!(report.session_mappings.len(), 1);
    let session_mapping = &report.session_mappings[0];
    assert_eq!(session_mapping.legacy_session_record_id, "session-1");
    assert_eq!(session_mapping.legacy_host_id, "generic");
    assert_eq!(session_mapping.legacy_host_session_id, "legacy-session");
    assert_eq!(session_mapping.session_id, projection.sessions[0].0);
    assert_eq!(
        projection.requests,
        vec![
            (
                "request-completed-readable".into(),
                Some("submitted".into())
            ),
            ("request-in-progress".into(), None),
            ("request-waiting".into(), None),
        ]
    );
    assert_eq!(projection.actions.len(), 22);
    for expected in [
        (
            "request-completed-readable".into(),
            "review".into(),
            "Review the migrated feedback request.".into(),
        ),
        (
            "request-in-progress".into(),
            "review".into(),
            "Review the migrated feedback request.".into(),
        ),
        (
            "request-waiting".into(),
            "action-review".into(),
            "Review the proposed implementation".into(),
        ),
        (
            "request-waiting".into(),
            "action-extra-19".into(),
            "Additional legacy action 19".into(),
        ),
    ] {
        assert!(projection.actions.contains(&expected));
    }
    assert!(
        !projection
            .actions
            .iter()
            .any(|(_, action_id, _)| action_id == "action-extra-20")
    );
    assert_eq!(
        projection.context_refs,
        vec![(
            "request-waiting".into(),
            "Relevant diff".into(),
            "https://example.invalid/review.diff".into(),
        )]
    );
    assert_eq!(projection.drafts.len(), 2);
    assert!(projection.drafts.contains(&(
        "request-in-progress".into(),
        "{\"schemaVersion\":2,\"doc\":{\"type\":\"doc\"}}".into(),
        "Unfinished structured feedback".into(),
        1,
    )));
    assert!(projection.drafts.contains(&(
        "request-waiting".into(),
        "{\"schemaVersion\":2,\"doc\":{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}}".into(),
        "Waiting draft projection".into(),
        2,
    )));
    assert_eq!(projection.request_artifact_count, 3);
    let duplicate_request_entries = projection
        .request_artifacts
        .iter()
        .filter(|(request_id, _, _, _, _)| request_id == "request-waiting")
        .collect::<Vec<_>>();
    assert_eq!(duplicate_request_entries.len(), 2);
    assert_eq!(
        duplicate_request_entries[0].3,
        duplicate_request_entries[1].3
    );
    assert_eq!(
        duplicate_request_entries[0].4,
        duplicate_request_entries[1].4
    );
    assert_eq!(
        duplicate_request_entries
            .iter()
            .map(|entry| entry.2.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["request-context-copy.md", "request-context.md"])
    );
    assert_eq!(projection.draft_artifact_count, 2);
    assert!(projection.draft_artifacts.contains(&(
        "request-waiting".into(),
        "attachment.bin".into(),
        "application/octet-stream".into(),
    )));
    assert_eq!(projection.package_artifact_count, 3);
    assert_eq!(projection.package_count, 1);
    assert_eq!(projection.delivered_count, 1);
    assert_eq!(projection.package_published_at, "2026-08-01T08:00:00Z");
    assert_eq!(projection.delivery_created_at, "2026-08-01T08:00:00Z");
    assert_eq!(projection.agent_work_count, 0);
    assert_eq!(projection.artifact_objects.len(), 6);
    assert_artifact_store(&target, &projection.artifact_objects);
    assert!(!projection.manifest_json.contains("storage_key"));
    assert!(
        !projection
            .manifest_json
            .contains(source_root.path().to_string_lossy().as_ref())
    );

    let source_bytes = std::fs::read(&source).expect("source database bytes");
    assert_eq!(
        std::fs::read(target.join("backup/source.sqlite3")).expect("backup database"),
        source_bytes
    );
    assert_backup_read_only(&target.join("backup"));
    let backup_index: Value = serde_json::from_slice(
        &std::fs::read(target.join("backup/legacy-library/index.json")).expect("backup index"),
    )
    .expect("parse backup index");
    assert_eq!(
        backup_index["session_mappings"][0],
        serde_json::json!({
            "legacy_session_record_id": "session-1",
            "legacy_host_id": "generic",
            "legacy_host_session_id": "legacy-session",
            "session_id": projection.sessions[0].0.clone(),
        })
    );
    let backup_bytes = snapshot_tree(&target.join("backup/legacy-library"))
        .into_values()
        .map(|snapshot| snapshot.bytes)
        .collect::<BTreeSet<_>>();
    for expected in expected_artifact_bytes() {
        assert!(
            backup_bytes.contains(&expected),
            "backup omitted a legacy artifact"
        );
    }

    let written: MigrationReport = serde_json::from_slice(
        &std::fs::read(target.join("reports/migration-report.json"))
            .expect("machine-readable report"),
    )
    .expect("parse machine-readable report");
    assert_eq!(written, report);
    assert!(
        std::fs::read_to_string(target.join("reports/migration-report.md"))
            .expect("human-readable report")
            .contains("request-completed-unreadable")
    );

    let before_verify = snapshot_tree(&target);
    let verified = verify(&target).await.expect("verify migrated root");
    assert!(verified.valid);
    assert!(verified.checks.iter().all(|check| check.passed));
    assert_eq!(verified.counts.sessions, 1);
    assert_eq!(verified.counts.waiting_requests, 2);
    assert_eq!(verified.counts.submitted_requests, 1);
    assert_eq!(verified.counts.drafts, 2);
    assert_eq!(verified.counts.packages, 1);
    assert_eq!(verified.counts.delivered_deliveries, 1);
    assert_eq!(verified.counts.artifact_objects, 6);
    assert_eq!(before_verify, snapshot_tree(&target));

    let runtime_store = SqliteV3Store::connect(&database)
        .await
        .expect("v3 runtime opens migrated database");
    let runtime_consistency = runtime_store
        .inspect_consistency()
        .await
        .expect("runtime consistency report");
    assert!(
        runtime_consistency.is_consistent(),
        "runtime rejected migrated facts: {:?}",
        runtime_consistency.violations
    );
    runtime_store.close().await;
    let after_runtime_interop = snapshot_tree(&target);

    let error = execute(&source, &target)
        .await
        .expect_err("a second execute must reject the existing target");
    assert!(matches!(error, MigrationError::TargetExists));
    assert_eq!(after_runtime_interop, snapshot_tree(&target));

    let second_target = output_root.path().join("second");
    let second_report = execute(&source, &second_target)
        .await
        .expect("execute to a second fresh root");
    assert_eq!(logical_report(&report), logical_report(&second_report));
    assert_eq!(
        projection,
        read_projection(&second_target.join("rambledesk-v3.sqlite3")).await
    );
    assert_eq!(source_before, snapshot_tree(source_root.path()));
}

#[tokio::test]
async fn failed_execute_leaves_no_final_target_and_can_be_retried() {
    let source_root = tempfile::tempdir().expect("source root");
    let output_root = tempfile::tempdir().expect("output root");
    let source = create_fixture(source_root.path()).await;
    let source_before = snapshot_tree(source_root.path());
    let blocked_parent = output_root.path().join("blocked-parent");
    tokio::fs::write(&blocked_parent, b"not a directory")
        .await
        .expect("blocked parent");
    let target = blocked_parent.join("migration");

    execute(&source, &target)
        .await
        .expect_err("execute through a non-directory parent must fail");
    assert!(!target.exists());
    assert_eq!(source_before, snapshot_tree(source_root.path()));

    tokio::fs::remove_file(&blocked_parent)
        .await
        .expect("remove blocker");
    tokio::fs::create_dir(&blocked_parent)
        .await
        .expect("create output parent");
    execute(&source, &target)
        .await
        .expect("retry after fixing output parent");
    assert_fixed_layout(&target);
    assert_eq!(source_before, snapshot_tree(source_root.path()));
}

#[tokio::test]
async fn unsafe_package_parent_is_dropped_without_copying_unrelated_files() {
    let source_root = tempfile::tempdir().expect("source root");
    let output_root = tempfile::tempdir().expect("output root");
    let source = create_fixture(source_root.path()).await;
    let secret = add_unsafe_parent_package(source_root.path(), &source).await;
    let source_before = snapshot_tree(source_root.path());

    let inspected = inspect(&source)
        .await
        .expect("inspect unsafe package fixture");
    let inspected_json = serde_json::to_value(&inspected).expect("serialize inspect report");
    let unsafe_record = inspected_json["records"]
        .as_array()
        .expect("inspect records")
        .iter()
        .find(|record| record["legacy_id"] == "request-completed-unsafe-parent")
        .expect("unsafe package record");
    assert_eq!(unsafe_record["disposition"], "drop");
    assert_eq!(unsafe_record["detail"], "unsafe_package_directory");

    let target = output_root.path().join("migration");
    let report = execute(&source, &target)
        .await
        .expect("migrate while dropping unsafe package");
    assert_loss(
        &report,
        "request-completed-unsafe-parent",
        "completed_package_unsafe_directory",
    );
    assert_eq!(report.counts.records_dropped, 6);
    assert_eq!(source_before, snapshot_tree(source_root.path()));
    assert!(
        snapshot_tree(&target)
            .into_values()
            .all(|file| !contains_subslice(&file.bytes, &secret)),
        "unrelated source-parent bytes leaked into the migration target"
    );
    let projection = read_projection(&target.join("rambledesk-v3.sqlite3")).await;
    assert!(
        projection
            .requests
            .iter()
            .all(|(request_id, _)| request_id != "request-completed-unsafe-parent")
    );
}

#[derive(Debug, PartialEq, Eq)]
struct LogicalProjection {
    sessions: Vec<(String, String)>,
    legacy_business_columns: Vec<String>,
    requests: Vec<(String, Option<String>)>,
    actions: Vec<(String, String, String)>,
    context_refs: Vec<(String, String, String)>,
    drafts: Vec<(String, String, String, i64)>,
    artifact_objects: Vec<(String, String, i64)>,
    request_artifact_count: i64,
    request_artifacts: Vec<(String, String, String, String, String)>,
    draft_artifact_count: i64,
    draft_artifacts: Vec<(String, String, String)>,
    package_artifact_count: i64,
    package_count: i64,
    delivered_count: i64,
    package_published_at: String,
    delivery_created_at: String,
    agent_work_count: i64,
    manifest_json: String,
}

async fn read_projection(database: &Path) -> LogicalProjection {
    let options = SqliteConnectOptions::new()
        .filename(database)
        .create_if_missing(false)
        .read_only(true)
        .immutable(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open migrated database read-only");
    let quick_check: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(&pool)
        .await
        .expect("quick check");
    assert_eq!(quick_check, "ok");
    let foreign_key_violations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pragma_foreign_key_check")
            .fetch_one(&pool)
            .await
            .expect("foreign key check");
    assert_eq!(foreign_key_violations, 0);
    let projection = LogicalProjection {
        sessions: sqlx::query_as(
            "SELECT session_id, session_kind FROM sessions_v3 ORDER BY session_id",
        )
        .fetch_all(&pool)
        .await
        .expect("sessions"),
        legacy_business_columns: legacy_business_columns(&pool).await,
        requests: sqlx::query_as(
            "SELECT request_id, resolution FROM feedback_requests_v3 ORDER BY request_id",
        )
        .fetch_all(&pool)
        .await
        .expect("requests"),
        actions: sqlx::query_as(
            "SELECT request_id, action_id, instruction FROM feedback_request_actions_v3 \
             ORDER BY request_id, position",
        )
        .fetch_all(&pool)
        .await
        .expect("actions"),
        context_refs: sqlx::query_as(
            "SELECT request_id, label, uri FROM feedback_request_context_refs_v3 \
             ORDER BY request_id, position",
        )
        .fetch_all(&pool)
        .await
        .expect("context refs"),
        drafts: sqlx::query_as(
            "SELECT request_id, document_json, body_markdown, revision FROM ramble_drafts_v3 \
             ORDER BY request_id",
        )
        .fetch_all(&pool)
        .await
        .expect("drafts"),
        artifact_objects: sqlx::query_as(
            "SELECT sha256, storage_key, size_bytes FROM artifact_objects_v3 ORDER BY sha256",
        )
        .fetch_all(&pool)
        .await
        .expect("artifact objects"),
        request_artifact_count: table_count(&pool, "feedback_request_artifacts_v3").await,
        request_artifacts: sqlx::query_as(
            "SELECT request_id, artifact_id, display_name, sha256, storage_key \
             FROM feedback_request_artifacts_v3 ORDER BY request_id, position",
        )
        .fetch_all(&pool)
        .await
        .expect("request artifacts"),
        draft_artifact_count: table_count(&pool, "draft_artifacts_v3").await,
        draft_artifacts: sqlx::query_as(
            "SELECT drafts.request_id, artifacts.display_name, artifacts.media_type \
             FROM draft_artifacts_v3 artifacts \
             JOIN ramble_drafts_v3 drafts ON drafts.draft_id = artifacts.draft_id \
             ORDER BY drafts.request_id, artifacts.position",
        )
        .fetch_all(&pool)
        .await
        .expect("draft artifacts"),
        package_artifact_count: table_count(&pool, "package_artifacts_v3").await,
        package_count: table_count(&pool, "packages_v3").await,
        delivered_count: sqlx::query_scalar(
            "SELECT COUNT(*) FROM feedback_deliveries_v3 WHERE state = 'delivered'",
        )
        .fetch_one(&pool)
        .await
        .expect("delivered count"),
        package_published_at: sqlx::query_scalar("SELECT published_at FROM packages_v3")
            .fetch_one(&pool)
            .await
            .expect("package published_at"),
        delivery_created_at: sqlx::query_scalar(
            "SELECT created_at FROM feedback_deliveries_v3 WHERE state = 'delivered'",
        )
        .fetch_one(&pool)
        .await
        .expect("delivery created_at"),
        agent_work_count: table_count(&pool, "agent_work_v3").await,
        manifest_json: sqlx::query_scalar("SELECT manifest_json FROM packages_v3")
            .fetch_one(&pool)
            .await
            .expect("package manifest"),
    };
    pool.close().await;
    projection
}

async fn table_count(pool: &sqlx::SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .expect("table count")
}

async fn legacy_business_columns(pool: &sqlx::SqlitePool) -> Vec<String> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master \
         WHERE type = 'table' AND name LIKE '%_v3' AND name != 'migration_sources_v3' \
         ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .expect("v3 business tables");
    let mut matches = Vec::new();
    for table in tables {
        let columns: Vec<String> =
            sqlx::query_scalar(&format!("SELECT name FROM pragma_table_info('{table}')"))
                .fetch_all(pool)
                .await
                .expect("business table columns");
        for column in columns {
            if matches!(column.as_str(), "host_id" | "host_session_id") {
                matches.push(format!("{table}.{column}"));
            }
        }
    }
    matches
}

fn assert_fixed_layout(target: &Path) {
    for relative in [
        "rambledesk-v3.sqlite3",
        "library/artifacts",
        "reports/migration-report.json",
        "reports/migration-report.md",
        "backup/source.sqlite3",
        "backup/legacy-library",
    ] {
        assert!(target.join(relative).exists(), "missing {relative}");
    }
    assert!(!target.join("rambledesk-v3.sqlite3-wal").exists());
    assert!(!target.join("rambledesk-v3.sqlite3-shm").exists());
}

#[cfg(unix)]
fn assert_backup_read_only(root: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fn visit(path: &Path) {
        let metadata = std::fs::symlink_metadata(path).expect("backup metadata");
        assert_eq!(
            metadata.permissions().mode() & 0o222,
            0,
            "backup remains writable: {}",
            path.display()
        );
        if metadata.is_dir() {
            for entry in std::fs::read_dir(path).expect("read backup directory") {
                visit(&entry.expect("backup entry").path());
            }
        }
    }

    visit(root);
}

#[cfg(not(unix))]
fn assert_backup_read_only(_root: &Path) {}

fn assert_artifact_store(target: &Path, objects: &[(String, String, i64)]) {
    let expected = expected_artifact_bytes();
    let mut actual = BTreeSet::new();
    for (digest, storage_key, size_bytes) in objects {
        let relative = Path::new(storage_key);
        assert!(!relative.is_absolute());
        assert!(!storage_key.contains(".."));
        assert!(storage_key.starts_with("sha256/"));
        let bytes = std::fs::read(target.join("library/artifacts").join(relative))
            .expect("open opaque artifact key");
        assert_eq!(bytes.len() as i64, *size_bytes);
        assert_eq!(
            format!("sha256:{}", hex::encode(Sha256::digest(&bytes))),
            *digest
        );
        actual.insert(bytes);
    }
    assert_eq!(actual, expected);
}

fn expected_artifact_bytes() -> BTreeSet<Vec<u8>> {
    [
        FEEDBACK,
        UNCOOKED,
        PACKAGE_ATTACHMENT,
        COMPLETED_REQUEST_ATTACHMENT,
        DRAFT_ATTACHMENT,
        WAITING_REQUEST_ATTACHMENT,
    ]
    .into_iter()
    .map(<[u8]>::to_vec)
    .collect()
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

fn logical_report(report: &MigrationReport) -> Value {
    let mut value = serde_json::to_value(report).expect("serialize report");
    let object = value.as_object_mut().expect("report object");
    object.remove("started_at");
    object.remove("finished_at");
    object.remove("outputs");
    value
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}
