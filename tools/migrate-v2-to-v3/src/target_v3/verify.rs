use std::{path::Path, time::Duration};

use rambledesk_core::kernel::{
    ArtifactId, ArtifactInput, ArtifactRole, ContextReference, CreateFeedbackRequest,
    FeedbackAction, PackageArtifact, PackageId, PackagePurpose, PackageRecord, RequestId,
    SessionId, SubmissionId, calculate_feedback_request_digest, package_digests_match,
    validate_feedback_request_input,
};
use serde::Deserialize;
use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};

use crate::{
    inspect::file_sha256,
    migration::{MigrationError, render_markdown},
    model::{
        MIGRATION_REPORT_SCHEMA, MigrationReport, VERIFY_REPORT_SCHEMA, VerifyCheck, VerifyCounts,
        VerifyReport,
    },
};

use super::verify_paths::{digest, read_real_file, reject_target_sidecars};
use super::verify_submissions::check_submissions;

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: u32,
    package_id: String,
    submission_id: String,
    package_purpose: String,
    request_id: Option<String>,
    content_digest: String,
    artifacts: Vec<ManifestArtifact>,
    published_at: String,
}

#[derive(Debug, Deserialize)]
struct ManifestArtifact {
    artifact_id: String,
    role: String,
    position: u32,
    display_name: String,
    media_type: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct BackupIndex {
    schema: String,
    source_database: String,
    source_database_sha256: String,
    objects: Vec<BackupObject>,
}

#[derive(Debug, Deserialize)]
struct BackupObject {
    backup_object: String,
    sha256: String,
    size_bytes: u64,
}

pub(crate) async fn verify_root(target_root: &Path) -> Result<VerifyReport, MigrationError> {
    verify_root_mode(target_root, false).await
}

pub(crate) async fn verify_published_root(
    target_root: &Path,
) -> Result<VerifyReport, MigrationError> {
    verify_root_mode(target_root, true).await
}

async fn verify_root_mode(
    target_root: &Path,
    require_report: bool,
) -> Result<VerifyReport, MigrationError> {
    let root_metadata = tokio::fs::symlink_metadata(target_root)
        .await
        .map_err(MigrationError::WriteTarget)?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(MigrationError::InvalidTargetRoot);
    }
    let database = target_root.join("rambledesk-v3.sqlite3");
    reject_target_sidecars(&database).await?;
    let database_bytes = read_real_file(target_root, &database)
        .await
        .map_err(MigrationError::WriteTarget)?;
    let target_database_sha256 = digest(&database_bytes);
    let options = SqliteConnectOptions::new()
        .filename(&database)
        .create_if_missing(false)
        .read_only(true)
        .immutable(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePool::connect_with(options)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    sqlx::query("PRAGMA query_only = ON")
        .execute(&pool)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    let mut checks = Vec::new();
    checks.push(check_schema(&pool).await?);
    checks.push(check_foreign_keys(&pool).await?);
    let (artifact_check, artifact_count) = check_artifacts(&pool, target_root).await?;
    checks.push(artifact_check);
    checks.push(check_artifact_entries(&pool).await?);
    checks.push(check_feedback_requests(&pool, target_root).await?);
    checks.push(check_submissions(&pool, target_root).await?);
    checks.push(check_packages(&pool).await?);
    checks.push(check_backup(target_root, require_report).await?);
    let counts = read_counts(&pool, artifact_count).await?;
    checks.push(check_lifecycle(&pool).await?);
    pool.close().await;
    if require_report {
        checks.push(check_report(target_root, &target_database_sha256, &counts).await?);
    }
    let valid = checks.iter().all(|check| check.passed);
    Ok(VerifyReport {
        report_schema: VERIFY_REPORT_SCHEMA.to_owned(),
        mode: "verify".to_owned(),
        target_schema: "v3".to_owned(),
        valid,
        target_database_sha256,
        counts,
        checks,
    })
}

async fn check_feedback_requests(
    pool: &SqlitePool,
    target_root: &Path,
) -> Result<VerifyCheck, MigrationError> {
    let rows = sqlx::query(
        "SELECT request_id, session_id, title, instructions, input_digest \
         FROM feedback_requests_v3 ORDER BY request_id",
    )
    .fetch_all(pool)
    .await
    .map_err(MigrationError::TargetDatabase)?;
    let mut errors = Vec::new();
    for row in &rows {
        let request_id: String = row
            .try_get("request_id")
            .map_err(MigrationError::TargetDatabase)?;
        let action_rows = sqlx::query(
            "SELECT action_id, instruction FROM feedback_request_actions_v3 \
             WHERE request_id = ?1 ORDER BY position",
        )
        .bind(&request_id)
        .fetch_all(pool)
        .await
        .map_err(MigrationError::TargetDatabase)?;
        let actions = action_rows
            .into_iter()
            .map(|action| {
                Ok(FeedbackAction {
                    id: action
                        .try_get("action_id")
                        .map_err(MigrationError::TargetDatabase)?,
                    instruction: action
                        .try_get("instruction")
                        .map_err(MigrationError::TargetDatabase)?,
                })
            })
            .collect::<Result<Vec<_>, MigrationError>>()?;
        let context_rows = sqlx::query(
            "SELECT label, uri FROM feedback_request_context_refs_v3 \
             WHERE request_id = ?1 ORDER BY position",
        )
        .bind(&request_id)
        .fetch_all(pool)
        .await
        .map_err(MigrationError::TargetDatabase)?;
        let context_refs = context_rows
            .into_iter()
            .map(|context| {
                Ok(ContextReference {
                    label: context
                        .try_get("label")
                        .map_err(MigrationError::TargetDatabase)?,
                    uri: context
                        .try_get("uri")
                        .map_err(MigrationError::TargetDatabase)?,
                })
            })
            .collect::<Result<Vec<_>, MigrationError>>()?;
        let artifact_rows = sqlx::query(
            "SELECT display_name, media_type, storage_key, sha256 \
             FROM feedback_request_artifacts_v3 WHERE request_id = ?1 ORDER BY position",
        )
        .bind(&request_id)
        .fetch_all(pool)
        .await
        .map_err(MigrationError::TargetDatabase)?;
        let mut artifacts = Vec::with_capacity(artifact_rows.len());
        for artifact in artifact_rows {
            let storage_key: String = artifact
                .try_get("storage_key")
                .map_err(MigrationError::TargetDatabase)?;
            let expected: String = artifact
                .try_get("sha256")
                .map_err(MigrationError::TargetDatabase)?;
            let path = target_root
                .join("library")
                .join("artifacts")
                .join(storage_key);
            let contents = read_real_file(target_root, &path)
                .await
                .map_err(MigrationError::WriteTarget)?;
            if digest(&contents) != expected {
                errors.push(format!("Artifact digest mismatch {request_id}"));
            }
            artifacts.push(ArtifactInput {
                display_name: artifact
                    .try_get("display_name")
                    .map_err(MigrationError::TargetDatabase)?,
                media_type: artifact
                    .try_get("media_type")
                    .map_err(MigrationError::TargetDatabase)?,
                contents,
            });
        }
        let input = CreateFeedbackRequest {
            request_id: Some(RequestId::new(request_id.clone())),
            session_id: SessionId::new(
                row.try_get::<String, _>("session_id")
                    .map_err(MigrationError::TargetDatabase)?,
            ),
            source_link_id: None,
            title: row
                .try_get("title")
                .map_err(MigrationError::TargetDatabase)?,
            instructions: row
                .try_get("instructions")
                .map_err(MigrationError::TargetDatabase)?,
            actions,
            context_refs,
            artifacts,
        };
        if validate_feedback_request_input(&input).is_err() {
            errors.push(format!("invalid Request facts {request_id}"));
            continue;
        }
        let stored: String = row
            .try_get("input_digest")
            .map_err(MigrationError::TargetDatabase)?;
        if calculate_feedback_request_digest(&input) != stored {
            errors.push(format!("Request digest mismatch {request_id}"));
        }
    }
    Ok(VerifyCheck {
        name: "feedback_requests".to_owned(),
        passed: errors.is_empty(),
        detail: if errors.is_empty() {
            format!("verified={}", rows.len())
        } else {
            errors.join("; ")
        },
    })
}

async fn check_schema(pool: &SqlitePool) -> Result<VerifyCheck, MigrationError> {
    let marker: Option<(i64, i64)> =
        sqlx::query_as("SELECT generation, revision FROM schema_generation_v3 WHERE singleton = 1")
            .fetch_optional(pool)
            .await
            .map_err(MigrationError::TargetDatabase)?;
    let migration: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(CASE \
             WHEN version IN (3001, 3002) AND success = TRUE THEN 1 ELSE 0 END), 0) \
         FROM _sqlx_migrations",
    )
    .fetch_one(pool)
    .await
    .map_err(MigrationError::TargetDatabase)?;
    Ok(VerifyCheck {
        name: "schema_generation".to_owned(),
        passed: marker == Some((3, 2)) && migration == (2, 2),
        detail: format!("marker={marker:?}, migrations_3001_3002={migration:?}"),
    })
}

async fn check_foreign_keys(pool: &SqlitePool) -> Result<VerifyCheck, MigrationError> {
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(pool)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    Ok(VerifyCheck {
        name: "foreign_keys".to_owned(),
        passed: violations.is_empty(),
        detail: format!("violations={}", violations.len()),
    })
}

async fn check_artifacts(
    pool: &SqlitePool,
    target_root: &Path,
) -> Result<(VerifyCheck, u64), MigrationError> {
    let rows = sqlx::query(
        "SELECT storage_key, sha256, size_bytes FROM artifact_objects_v3 ORDER BY storage_key",
    )
    .fetch_all(pool)
    .await
    .map_err(MigrationError::TargetDatabase)?;
    let mut errors = Vec::new();
    for row in &rows {
        let storage_key: String = row
            .try_get("storage_key")
            .map_err(MigrationError::TargetDatabase)?;
        let expected: String = row
            .try_get("sha256")
            .map_err(MigrationError::TargetDatabase)?;
        let size: i64 = row
            .try_get("size_bytes")
            .map_err(MigrationError::TargetDatabase)?;
        if !valid_storage_key(&storage_key, &expected) {
            errors.push(format!("invalid key {storage_key}"));
            continue;
        }
        let path = target_root
            .join("library")
            .join("artifacts")
            .join(&storage_key);
        match read_real_file(target_root, &path).await {
            Ok(bytes)
                if bytes.len() as i64 == size && digest(&bytes).as_str() == expected.as_str() => {}
            Ok(_) => errors.push(format!("digest/size mismatch {storage_key}")),
            Err(_) => errors.push(format!("unreadable {storage_key}")),
        }
    }
    Ok((
        VerifyCheck {
            name: "artifact_objects".to_owned(),
            passed: errors.is_empty(),
            detail: if errors.is_empty() {
                format!("verified={}", rows.len())
            } else {
                errors.join("; ")
            },
        },
        rows.len() as u64,
    ))
}

async fn check_artifact_entries(pool: &SqlitePool) -> Result<VerifyCheck, MigrationError> {
    let mismatches: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM (
            SELECT storage_key, sha256, size_bytes FROM feedback_request_artifacts_v3
            UNION ALL SELECT storage_key, sha256, size_bytes FROM submission_artifacts_v3
            UNION ALL SELECT storage_key, sha256, size_bytes FROM draft_artifacts_v3
            UNION ALL SELECT storage_key, sha256, size_bytes FROM package_artifacts_v3
         ) entries LEFT JOIN artifact_objects_v3 objects
           ON objects.storage_key = entries.storage_key
          AND objects.sha256 = entries.sha256
          AND objects.size_bytes = entries.size_bytes
         WHERE objects.storage_key IS NULL"#,
    )
    .fetch_one(pool)
    .await
    .map_err(MigrationError::TargetDatabase)?;
    Ok(VerifyCheck {
        name: "artifact_entries".to_owned(),
        passed: mismatches == 0,
        detail: format!("mismatches={mismatches}"),
    })
}

async fn check_packages(pool: &SqlitePool) -> Result<VerifyCheck, MigrationError> {
    let rows = sqlx::query(
        "SELECT package_id, submission_id, package_purpose, request_id, schema_version, \
                manifest_json, content_digest, manifest_digest, published_at \
         FROM packages_v3 ORDER BY package_id",
    )
    .fetch_all(pool)
    .await
    .map_err(MigrationError::TargetDatabase)?;
    let mut errors = Vec::new();
    for row in &rows {
        let package_id: String = row
            .try_get("package_id")
            .map_err(MigrationError::TargetDatabase)?;
        let artifact_rows = sqlx::query(
            "SELECT artifact_id, role, position, display_name, media_type, size_bytes, sha256, storage_key \
             FROM package_artifacts_v3 WHERE package_id = ?1 ORDER BY position",
        )
        .bind(&package_id)
        .fetch_all(pool)
        .await
        .map_err(MigrationError::TargetDatabase)?;
        let artifacts = artifact_rows
            .into_iter()
            .map(|artifact| row_to_artifact(&artifact))
            .collect::<Result<Vec<_>, _>>()?;
        let submission_id: String = row
            .try_get("submission_id")
            .map_err(MigrationError::TargetDatabase)?;
        let purpose_label: String = row
            .try_get("package_purpose")
            .map_err(MigrationError::TargetDatabase)?;
        let request_id: Option<String> = row
            .try_get("request_id")
            .map_err(MigrationError::TargetDatabase)?;
        let published_at: String = row
            .try_get("published_at")
            .map_err(MigrationError::TargetDatabase)?;
        let package = PackageRecord {
            package_id: PackageId::new(package_id.clone()),
            submission_id: SubmissionId::new(submission_id.clone()),
            purpose: if purpose_label == "launch" {
                PackagePurpose::Launch
            } else {
                PackagePurpose::Response
            },
            request_id: request_id.clone().map(RequestId::new),
            content_digest: row
                .try_get("content_digest")
                .map_err(MigrationError::TargetDatabase)?,
            manifest_digest: row
                .try_get("manifest_digest")
                .map_err(MigrationError::TargetDatabase)?,
            schema_version: row
                .try_get::<i64, _>("schema_version")
                .map_err(MigrationError::TargetDatabase)? as u32,
            artifacts: artifacts.clone(),
            published_at: published_at.clone(),
        };
        if !package_digests_match(&package) {
            errors.push(format!("Package digest mismatch {package_id}"));
        }
        let manifest_json: String = row
            .try_get("manifest_json")
            .map_err(MigrationError::TargetDatabase)?;
        if manifest_json.contains("storage_key") || manifest_json.contains("legacy_path") {
            errors.push(format!("private locator leaked in manifest {package_id}"));
            continue;
        }
        match serde_json::from_str::<Manifest>(&manifest_json) {
            Ok(manifest)
                if manifest_matches(&manifest, &package, &purpose_label, request_id.as_deref()) => {
            }
            Ok(_) => errors.push(format!("manifest facts mismatch {package_id}")),
            Err(_) => errors.push(format!("invalid manifest {package_id}")),
        }
    }
    Ok(VerifyCheck {
        name: "packages".to_owned(),
        passed: errors.is_empty(),
        detail: if errors.is_empty() {
            format!("verified={}", rows.len())
        } else {
            errors.join("; ")
        },
    })
}

async fn check_backup(
    target_root: &Path,
    require_read_only: bool,
) -> Result<VerifyCheck, MigrationError> {
    let index_path = target_root
        .join("backup")
        .join("legacy-library")
        .join("index.json");
    let mut errors = Vec::new();
    let index = match read_real_file(target_root, &index_path).await {
        Ok(bytes) => serde_json::from_slice::<BackupIndex>(&bytes).ok(),
        Err(_) => None,
    };
    if let Some(index) = index {
        if index.schema != "rambledesk-v2-backup-index-v1"
            || index.source_database != "source.sqlite3"
        {
            errors.push("invalid backup index header".to_owned());
        }
        for object in index.objects {
            let path = Path::new(&object.backup_object);
            if path.is_absolute()
                || path
                    .components()
                    .any(|value| matches!(value, std::path::Component::ParentDir))
            {
                errors.push("unsafe backup object key".to_owned());
                continue;
            }
            match read_real_file(target_root, &target_root.join("backup").join(path)).await {
                Ok(bytes)
                    if bytes.len() as u64 == object.size_bytes
                        && digest(&bytes) == object.sha256 => {}
                _ => errors.push(format!("invalid backup object {}", object.backup_object)),
            }
        }
    } else {
        errors.push("backup index unreadable".to_owned());
    }
    let source = target_root.join("backup").join("source.sqlite3");
    match read_real_file(target_root, &source).await {
        Ok(bytes) => {
            if let Some(index) = read_backup_index(target_root).await?
                && digest(&bytes) != index.source_database_sha256
            {
                errors.push("source database backup digest mismatch".to_owned());
            }
            match backup_quick_check(&source).await {
                Ok(true) => {}
                _ => errors.push("source database backup quick_check failed".to_owned()),
            }
        }
        Err(_) => errors.push("source database backup unreadable".to_owned()),
    }
    #[cfg(unix)]
    if require_read_only && backup_tree_has_write_bits(&target_root.join("backup")).await? {
        errors.push("backup tree is writable".to_owned());
    }
    Ok(VerifyCheck {
        name: "backup".to_owned(),
        passed: errors.is_empty(),
        detail: if errors.is_empty() {
            "source database and indexed objects verified".to_owned()
        } else {
            errors.join("; ")
        },
    })
}

async fn read_backup_index(target_root: &Path) -> Result<Option<BackupIndex>, MigrationError> {
    let path = target_root
        .join("backup")
        .join("legacy-library")
        .join("index.json");
    Ok(read_real_file(target_root, &path)
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok()))
}

async fn backup_quick_check(path: &Path) -> Result<bool, MigrationError> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .read_only(true)
        .immutable(true);
    let pool = SqlitePool::connect_with(options)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    let result: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(&pool)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    pool.close().await;
    Ok(result == "ok")
}

#[cfg(unix)]
async fn backup_tree_has_write_bits(root: &Path) -> Result<bool, MigrationError> {
    use std::os::unix::fs::PermissionsExt;
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = tokio::fs::symlink_metadata(&path)
            .await
            .map_err(MigrationError::WriteTarget)?;
        if metadata.file_type().is_symlink() || metadata.permissions().mode() & 0o222 != 0 {
            return Ok(true);
        }
        if metadata.is_dir() {
            let mut entries = tokio::fs::read_dir(&path)
                .await
                .map_err(MigrationError::WriteTarget)?;
            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(MigrationError::WriteTarget)?
            {
                pending.push(entry.path());
            }
        }
    }
    Ok(false)
}

async fn check_lifecycle(pool: &SqlitePool) -> Result<VerifyCheck, MigrationError> {
    let violations: i64 = sqlx::query_scalar(
        r#"SELECT (
            SELECT COUNT(*) FROM sessions_v3 WHERE session_kind != 'imported'
         ) + (
            SELECT COUNT(*) FROM agent_work_v3
         ) + (
            SELECT COUNT(*) FROM feedback_requests_v3 r
            WHERE r.resolution IS NULL AND (
                r.response_package_id IS NOT NULL OR EXISTS (
                    SELECT 1 FROM feedback_deliveries_v3 d WHERE d.request_id = r.request_id
                )
            )
         ) + (
            SELECT COUNT(*) FROM feedback_requests_v3 r
            WHERE r.resolution = 'submitted' AND NOT EXISTS (
                SELECT 1 FROM feedback_deliveries_v3 d
                WHERE d.request_id = r.request_id AND d.state = 'delivered'
            )
         )"#,
    )
    .fetch_one(pool)
    .await
    .map_err(MigrationError::TargetDatabase)?;
    Ok(VerifyCheck {
        name: "migration_lifecycle".to_owned(),
        passed: violations == 0,
        detail: format!("violations={violations}"),
    })
}

async fn check_report(
    target_root: &Path,
    database_sha256: &str,
    counts: &VerifyCounts,
) -> Result<VerifyCheck, MigrationError> {
    let path = target_root.join("reports").join("migration-report.json");
    let report = read_real_file(target_root, &path)
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice::<MigrationReport>(&bytes).ok());
    let backup_digest = file_sha256(&target_root.join("backup").join("source.sqlite3"))
        .await
        .ok();
    let backup_index = read_backup_index(target_root).await?;
    let markdown = read_real_file(
        target_root,
        &target_root.join("reports").join("migration-report.md"),
    )
    .await
    .ok()
    .and_then(|bytes| String::from_utf8(bytes).ok());
    let passed = report.as_ref().is_some_and(|report| {
        let Some(outputs) = &report.outputs else {
            return false;
        };
        let Some(index) = &backup_index else {
            return false;
        };
        let unique_backup_objects = index
            .objects
            .iter()
            .map(|object| object.sha256.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len() as u64;
        report.report_schema == MIGRATION_REPORT_SCHEMA
            && report.mode == "execute"
            && outputs.database == "rambledesk-v3.sqlite3"
            && outputs.artifact_library == "library/artifacts"
            && outputs.backup_database == "backup/source.sqlite3"
            && outputs.backup_objects == "backup/legacy-library/objects"
            && outputs.backup_index == "backup/legacy-library/index.json"
            && outputs.json_report == "reports/migration-report.json"
            && outputs.markdown_report == "reports/migration-report.md"
            && outputs.database_sha256 == database_sha256
            && Some(outputs.backup_database_sha256.as_str()) == backup_digest.as_deref()
            && report.source_database_sha256 == outputs.backup_database_sha256
            && report.source_database_sha256 == index.source_database_sha256
            && outputs.backup_objects_count == unique_backup_objects
            && markdown.as_deref() == Some(render_markdown(report).as_str())
            && report.counts.sessions_created == counts.sessions
            && report.counts.waiting_requests_migrated == counts.waiting_requests
            && report.counts.submitted_requests_migrated == counts.submitted_requests
            && report.counts.drafts_migrated == counts.drafts
            && report.counts.artifacts_migrated == counts.artifact_objects
    });
    Ok(VerifyCheck {
        name: "migration_report".to_owned(),
        passed,
        detail: if passed {
            "report facts match target".to_owned()
        } else {
            "report missing or inconsistent".to_owned()
        },
    })
}

async fn read_counts(
    pool: &SqlitePool,
    artifact_objects: u64,
) -> Result<VerifyCounts, MigrationError> {
    Ok(VerifyCounts {
        sessions: count(pool, "SELECT COUNT(*) FROM sessions_v3").await?,
        waiting_requests: count(
            pool,
            "SELECT COUNT(*) FROM feedback_requests_v3 WHERE resolution IS NULL",
        )
        .await?,
        submitted_requests: count(
            pool,
            "SELECT COUNT(*) FROM feedback_requests_v3 WHERE resolution = 'submitted'",
        )
        .await?,
        drafts: count(pool, "SELECT COUNT(*) FROM ramble_drafts_v3").await?,
        packages: count(pool, "SELECT COUNT(*) FROM packages_v3").await?,
        delivered_deliveries: count(
            pool,
            "SELECT COUNT(*) FROM feedback_deliveries_v3 WHERE state = 'delivered'",
        )
        .await?,
        artifact_objects,
    })
}

async fn count(pool: &SqlitePool, query: &str) -> Result<u64, MigrationError> {
    let value: i64 = sqlx::query_scalar(query)
        .fetch_one(pool)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    Ok(value.max(0) as u64)
}

fn row_to_artifact(row: &sqlx::sqlite::SqliteRow) -> Result<PackageArtifact, MigrationError> {
    let role: String = row
        .try_get("role")
        .map_err(MigrationError::TargetDatabase)?;
    Ok(PackageArtifact {
        artifact_id: ArtifactId::new(
            row.try_get::<String, _>("artifact_id")
                .map_err(MigrationError::TargetDatabase)?,
        ),
        role: match role.as_str() {
            "feedback" => ArtifactRole::Feedback,
            "uncooked" => ArtifactRole::Uncooked,
            "attachment" => ArtifactRole::Attachment,
            _ => ArtifactRole::Other(role),
        },
        position: row
            .try_get::<i64, _>("position")
            .map_err(MigrationError::TargetDatabase)? as u32,
        display_name: row
            .try_get("display_name")
            .map_err(MigrationError::TargetDatabase)?,
        media_type: row
            .try_get("media_type")
            .map_err(MigrationError::TargetDatabase)?,
        size_bytes: row
            .try_get::<i64, _>("size_bytes")
            .map_err(MigrationError::TargetDatabase)? as u64,
        sha256: row
            .try_get("sha256")
            .map_err(MigrationError::TargetDatabase)?,
        storage_key: row
            .try_get("storage_key")
            .map_err(MigrationError::TargetDatabase)?,
    })
}

fn manifest_matches(
    manifest: &Manifest,
    package: &PackageRecord,
    purpose: &str,
    request_id: Option<&str>,
) -> bool {
    manifest.schema_version == package.schema_version
        && manifest.package_id == package.package_id.as_str()
        && manifest.submission_id == package.submission_id.as_str()
        && manifest.package_purpose == purpose
        && manifest.request_id.as_deref() == request_id
        && manifest.content_digest == package.content_digest
        && manifest.published_at == package.published_at
        && manifest.artifacts.len() == package.artifacts.len()
        && manifest
            .artifacts
            .iter()
            .zip(&package.artifacts)
            .all(|(left, right)| {
                left.artifact_id == right.artifact_id.as_str()
                    && left.role == right.role.digest_label()
                    && left.position == right.position
                    && left.display_name == right.display_name
                    && left.media_type == right.media_type
                    && left.size_bytes == right.size_bytes
                    && left.sha256 == right.sha256
            })
}

fn valid_storage_key(storage_key: &str, digest: &str) -> bool {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && storage_key == format!("sha256/{}/{}", &hex[..2], &hex[2..])
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
