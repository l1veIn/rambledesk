use std::{io::ErrorKind, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};
use thiserror::Error;
use tokio::io::AsyncReadExt;

use crate::legacy_v2::{LegacyPackageIssue, LegacyPackagePaths, inspect_package};

const REPORT_SCHEMA: &str = "rambledesk-v2-inspect-v1";

#[derive(Debug, Error)]
pub enum InspectError {
    #[error("the source database path is not a regular file")]
    SourceNotFile,
    #[error(
        "the source database has a non-empty WAL; fully exit RambleDesk and checkpoint it first"
    )]
    ActiveWal,
    #[error("failed to read the source database")]
    SourceRead(#[source] std::io::Error),
    #[error("failed to open the source database in immutable read-only mode")]
    SourceOpen(#[source] sqlx::Error),
    #[error("the source database does not match the supported v2 schema")]
    SourceSchema(#[source] sqlx::Error),
    #[error("the source database contains an unsupported request state: {0}")]
    UnsupportedState(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectReport {
    pub report_schema: String,
    pub mode: String,
    pub source_schema: String,
    pub target_schema: String,
    pub source_database_sha256: String,
    pub source_migration_version: Option<i64>,
    pub counts: InspectCounts,
    pub records: Vec<InspectRecord>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectCounts {
    pub sessions_seen: u64,
    pub requests_seen: u64,
    pub drafts_seen: u64,
    pub waiting_requests: u64,
    pub in_progress_requests: u64,
    pub completed_readable: u64,
    pub completed_unreadable: u64,
    pub cancelled_requests: u64,
    pub unsupported_approval_semantics: u64,
    pub orphan_drafts: u64,
    pub records_migratable: u64,
    pub records_dropped: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectRecord {
    pub legacy_id: String,
    pub classification: RecordClassification,
    pub disposition: RecordDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loss_reason: Option<LossReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<LegacyPackageIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordClassification {
    Waiting,
    InProgress,
    CompletedReadable,
    CompletedUnreadable,
    Cancelled,
    UnsupportedApprovalSemantics,
    OrphanDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordDisposition {
    MigrateWaiting,
    MigrateSubmitted,
    Drop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LossReason {
    InProgressStateCollapsedToWaiting,
    CompletedPackageUnreadable,
    CancelledRequest,
    UnsupportedApprovalSemantics,
    OrphanDraft,
}

struct LegacyRequestRow {
    id: String,
    status: String,
    resolution: Option<String>,
    allow_finish: bool,
    package: Option<LegacyPackagePaths>,
}

pub async fn inspect(source_db: &Path) -> Result<InspectReport, InspectError> {
    let source_db =
        tokio::fs::canonicalize(source_db)
            .await
            .map_err(|error| match error.kind() {
                ErrorKind::NotFound => InspectError::SourceNotFile,
                _ => InspectError::SourceRead(error),
            })?;
    let metadata = tokio::fs::symlink_metadata(&source_db)
        .await
        .map_err(InspectError::SourceRead)?;
    if !metadata.is_file() {
        return Err(InspectError::SourceNotFile);
    }
    reject_active_wal(&source_db).await?;
    let source_database_sha256 = file_sha256(&source_db).await?;

    let options = SqliteConnectOptions::new()
        .filename(&source_db)
        .create_if_missing(false)
        .read_only(true)
        .immutable(true)
        .foreign_keys(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(InspectError::SourceOpen)?;
    sqlx::query("PRAGMA query_only = ON")
        .execute(&pool)
        .await
        .map_err(InspectError::SourceOpen)?;

    let report = inspect_pool(&pool, source_database_sha256).await;
    pool.close().await;
    report
}

async fn inspect_pool(
    pool: &SqlitePool,
    source_database_sha256: String,
) -> Result<InspectReport, InspectError> {
    require_v2_schema(pool).await?;
    let sessions_seen = count(pool, "host_sessions").await?;
    let requests_seen = count(pool, "feedback_requests").await?;
    let drafts_seen = count(pool, "drafts").await?;
    let source_migration_version = source_migration_version(pool).await?;
    let rows = load_requests(pool).await?;
    let mut records = Vec::with_capacity(rows.len());

    for row in rows {
        let record = classify_request(row).await?;
        records.push(record);
    }
    let orphan_rows = sqlx::query(
        "SELECT d.request_id FROM drafts d \
         LEFT JOIN feedback_requests r ON r.id = d.request_id \
         WHERE r.id IS NULL ORDER BY d.request_id",
    )
    .fetch_all(pool)
    .await
    .map_err(InspectError::SourceSchema)?;
    for row in orphan_rows {
        records.push(InspectRecord {
            legacy_id: row
                .try_get("request_id")
                .map_err(InspectError::SourceSchema)?,
            classification: RecordClassification::OrphanDraft,
            disposition: RecordDisposition::Drop,
            loss_reason: Some(LossReason::OrphanDraft),
            detail: None,
        });
    }
    records.sort_by(|left, right| {
        left.legacy_id.cmp(&right.legacy_id).then_with(|| {
            classification_rank(left.classification).cmp(&classification_rank(right.classification))
        })
    });

    let mut counts = InspectCounts {
        sessions_seen,
        requests_seen,
        drafts_seen,
        ..InspectCounts::default()
    };
    for record in &records {
        match record.classification {
            RecordClassification::Waiting => counts.waiting_requests += 1,
            RecordClassification::InProgress => counts.in_progress_requests += 1,
            RecordClassification::CompletedReadable => counts.completed_readable += 1,
            RecordClassification::CompletedUnreadable => counts.completed_unreadable += 1,
            RecordClassification::Cancelled => counts.cancelled_requests += 1,
            RecordClassification::UnsupportedApprovalSemantics => {
                counts.unsupported_approval_semantics += 1;
            }
            RecordClassification::OrphanDraft => counts.orphan_drafts += 1,
        }
        match record.disposition {
            RecordDisposition::MigrateWaiting | RecordDisposition::MigrateSubmitted => {
                counts.records_migratable += 1;
            }
            RecordDisposition::Drop => counts.records_dropped += 1,
        }
    }

    Ok(InspectReport {
        report_schema: REPORT_SCHEMA.to_owned(),
        mode: "inspect".to_owned(),
        source_schema: "v2".to_owned(),
        target_schema: "v3".to_owned(),
        source_database_sha256,
        source_migration_version,
        counts,
        records,
    })
}

async fn classify_request(row: LegacyRequestRow) -> Result<InspectRecord, InspectError> {
    if row.allow_finish || row.resolution.as_deref() == Some("approved") {
        return Ok(InspectRecord {
            legacy_id: row.id,
            classification: RecordClassification::UnsupportedApprovalSemantics,
            disposition: RecordDisposition::Drop,
            loss_reason: Some(LossReason::UnsupportedApprovalSemantics),
            detail: None,
        });
    }
    match row.status.as_str() {
        "waiting" => Ok(InspectRecord {
            legacy_id: row.id,
            classification: RecordClassification::Waiting,
            disposition: RecordDisposition::MigrateWaiting,
            loss_reason: None,
            detail: None,
        }),
        "in_progress" => Ok(InspectRecord {
            legacy_id: row.id,
            classification: RecordClassification::InProgress,
            disposition: RecordDisposition::MigrateWaiting,
            loss_reason: Some(LossReason::InProgressStateCollapsedToWaiting),
            detail: None,
        }),
        "completed" => match row.package {
            Some(package) => match inspect_package(&row.id, &package).await {
                Ok(()) => Ok(InspectRecord {
                    legacy_id: row.id,
                    classification: RecordClassification::CompletedReadable,
                    disposition: RecordDisposition::MigrateSubmitted,
                    loss_reason: None,
                    detail: None,
                }),
                Err(issue) => Ok(unreadable_completed(row.id, issue)),
            },
            None => Ok(unreadable_completed(
                row.id,
                LegacyPackageIssue::MissingDatabaseResult,
            )),
        },
        "cancelled" => Ok(InspectRecord {
            legacy_id: row.id,
            classification: RecordClassification::Cancelled,
            disposition: RecordDisposition::Drop,
            loss_reason: Some(LossReason::CancelledRequest),
            detail: None,
        }),
        other => Err(InspectError::UnsupportedState(other.to_owned())),
    }
}

fn unreadable_completed(id: String, issue: LegacyPackageIssue) -> InspectRecord {
    InspectRecord {
        legacy_id: id,
        classification: RecordClassification::CompletedUnreadable,
        disposition: RecordDisposition::Drop,
        loss_reason: Some(LossReason::CompletedPackageUnreadable),
        detail: Some(issue),
    }
}

async fn load_requests(pool: &SqlitePool) -> Result<Vec<LegacyRequestRow>, InspectError> {
    let rows = sqlx::query(
        "SELECT r.id, r.status, r.resolution, r.allow_finish, \
                fr.directory_path, fr.markdown_path, fr.manifest_path, fr.manifest_sha256 \
         FROM feedback_requests r \
         LEFT JOIN feedback_results fr ON fr.request_id = r.id \
         ORDER BY r.id",
    )
    .fetch_all(pool)
    .await
    .map_err(InspectError::SourceSchema)?;
    rows.into_iter()
        .map(|row| {
            let manifest_path: Option<String> = row
                .try_get("manifest_path")
                .map_err(InspectError::SourceSchema)?;
            let package = manifest_path
                .map(|manifest_path| {
                    Ok(LegacyPackagePaths {
                        directory_path: row
                            .try_get("directory_path")
                            .map_err(InspectError::SourceSchema)?,
                        markdown_path: row
                            .try_get("markdown_path")
                            .map_err(InspectError::SourceSchema)?,
                        manifest_path,
                        manifest_sha256: row
                            .try_get("manifest_sha256")
                            .map_err(InspectError::SourceSchema)?,
                    })
                })
                .transpose()?;
            Ok(LegacyRequestRow {
                id: row.try_get("id").map_err(InspectError::SourceSchema)?,
                status: row.try_get("status").map_err(InspectError::SourceSchema)?,
                resolution: row
                    .try_get("resolution")
                    .map_err(InspectError::SourceSchema)?,
                allow_finish: row
                    .try_get::<i64, _>("allow_finish")
                    .map_err(InspectError::SourceSchema)?
                    != 0,
                package,
            })
        })
        .collect()
}

async fn require_v2_schema(pool: &SqlitePool) -> Result<(), InspectError> {
    for table in [
        "host_sessions",
        "feedback_requests",
        "drafts",
        "feedback_results",
    ] {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        )
        .bind(table)
        .fetch_one(pool)
        .await
        .map_err(InspectError::SourceSchema)?;
        if !exists {
            return Err(InspectError::SourceSchema(sqlx::Error::ColumnNotFound(
                table.to_owned(),
            )));
        }
    }
    Ok(())
}

async fn count(pool: &SqlitePool, table: &str) -> Result<u64, InspectError> {
    let query = format!("SELECT COUNT(*) FROM {table}");
    let count: i64 = sqlx::query_scalar(&query)
        .fetch_one(pool)
        .await
        .map_err(InspectError::SourceSchema)?;
    Ok(count.max(0) as u64)
}

async fn source_migration_version(pool: &SqlitePool) -> Result<Option<i64>, InspectError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await
    .map_err(InspectError::SourceSchema)?;
    if !exists {
        return Ok(None);
    }
    sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = TRUE")
        .fetch_one(pool)
        .await
        .map_err(InspectError::SourceSchema)
}

async fn reject_active_wal(source_db: &Path) -> Result<(), InspectError> {
    let file_name = source_db
        .file_name()
        .ok_or(InspectError::SourceNotFile)?
        .to_string_lossy();
    let wal = source_db.with_file_name(format!("{file_name}-wal"));
    match tokio::fs::metadata(wal).await {
        Ok(metadata) if metadata.len() > 0 => Err(InspectError::ActiveWal),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(InspectError::SourceRead(error)),
    }
}

async fn file_sha256(path: &Path) -> Result<String, InspectError> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(InspectError::SourceRead)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(InspectError::SourceRead)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

const fn classification_rank(classification: RecordClassification) -> u8 {
    match classification {
        RecordClassification::Waiting => 0,
        RecordClassification::InProgress => 1,
        RecordClassification::CompletedReadable => 2,
        RecordClassification::CompletedUnreadable => 3,
        RecordClassification::Cancelled => 4,
        RecordClassification::UnsupportedApprovalSemantics => 5,
        RecordClassification::OrphanDraft => 6,
    }
}
