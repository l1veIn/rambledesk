use std::{path::Path, time::Duration};

use async_trait::async_trait;
use rambledesk_core::kernel::{
    AgentWorkBatch, AgentWorkRecordOutcome, FactMutation, FactMutationOutcome, FactQuery,
    FactQueryOutcome, StoredWorkResult, WorkClaim,
    ports::{FactStore, FactStoreError},
};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use thiserror::Error;

mod consistency;
mod manifest;
mod query;
mod read;
mod work;
mod write;
mod write_support;

#[derive(Debug, Error)]
pub enum SqliteV3OpenError {
    #[error("failed to create the RambleDesk v3 data directory")]
    CreateDirectory(#[source] std::io::Error),
    #[error("failed to inspect the RambleDesk v3 database path")]
    InspectPath(#[source] std::io::Error),
    #[error("failed to secure the RambleDesk v3 database path")]
    SecurePath(#[source] std::io::Error),
    #[error("failed to connect to the RambleDesk v3 database")]
    Connect(#[source] sqlx::Error),
    #[error("failed to migrate the RambleDesk v3 database")]
    Migrate(#[source] sqlx::migrate::MigrateError),
    #[error(
        "the database contains schema version {applied}, newer than supported v3 version {supported}"
    )]
    NewerDatabase { applied: u64, supported: u64 },
    #[error(
        "the selected database uses the legacy RambleDesk schema; migrate it explicitly into a new v3 database"
    )]
    LegacyDatabaseRejected,
    #[error("the selected database contains an unknown non-RambleDesk schema")]
    UnknownDatabaseRejected,
    #[error("the RambleDesk v3 schema generation marker is invalid")]
    InvalidGeneration,
    #[error("the RambleDesk v3 database contains foreign-key violations")]
    ForeignKeyViolation,
    #[error("failed to inspect the RambleDesk v3 schema")]
    InspectSchema(#[source] sqlx::Error),
}

#[derive(Clone)]
pub struct SqliteV3Store {
    pub(super) pool: SqlitePool,
}

pub use consistency::V3ConsistencyReport;

impl SqliteV3Store {
    pub async fn connect(path: &Path) -> Result<Self, SqliteV3OpenError> {
        if tokio::fs::try_exists(path)
            .await
            .map_err(SqliteV3OpenError::InspectPath)?
        {
            reject_legacy_database_read_only(path).await?;
        }
        if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(SqliteV3OpenError::CreateDirectory)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(SqliteV3OpenError::Connect)?;
        secure_database_path(path).await?;

        let supported = 3001_u64;
        let applied = applied_migration_version(&pool)
            .await
            .map_err(SqliteV3OpenError::InspectSchema)?;
        if applied > supported {
            return Err(SqliteV3OpenError::NewerDatabase { applied, supported });
        }

        sqlx::migrate!("./migrations_v3")
            .run(&pool)
            .await
            .map_err(SqliteV3OpenError::Migrate)?;

        let marker: Option<(i64, i64)> = sqlx::query_as(
            "SELECT generation, revision FROM schema_generation_v3 WHERE singleton = 1",
        )
        .fetch_optional(&pool)
        .await
        .map_err(SqliteV3OpenError::InspectSchema)?;
        if marker != Some((3, 1)) {
            return Err(SqliteV3OpenError::InvalidGeneration);
        }
        let violations = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .map_err(SqliteV3OpenError::InspectSchema)?;
        if !violations.is_empty() {
            return Err(SqliteV3OpenError::ForeignKeyViolation);
        }
        Ok(Self { pool })
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

async fn reject_legacy_database_read_only(path: &Path) -> Result<(), SqliteV3OpenError> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(SqliteV3OpenError::Connect)?;
    let classification = classify_existing_database(&pool)
        .await
        .map_err(SqliteV3OpenError::InspectSchema)?;
    pool.close().await;
    match classification {
        ExistingDatabase::Empty | ExistingDatabase::V3 => Ok(()),
        ExistingDatabase::Legacy => Err(SqliteV3OpenError::LegacyDatabaseRejected),
        ExistingDatabase::Unknown => Err(SqliteV3OpenError::UnknownDatabaseRejected),
    }
}

enum ExistingDatabase {
    Empty,
    V3,
    Legacy,
    Unknown,
}

async fn classify_existing_database(pool: &SqlitePool) -> Result<ExistingDatabase, sqlx::Error> {
    let has_legacy_table: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name IN (
                'host_sessions', 'feedback_requests', 'request_actions',
                'request_context_refs', 'drafts', 'attachments',
                'feedback_results', 'submission_plans', 'request_attachments',
                'host_preferences'
            )
        )",
    )
    .fetch_one(pool)
    .await?;
    if has_legacy_table {
        return Ok(ExistingDatabase::Legacy);
    }
    let has_migrations: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = '_sqlx_migrations'
        )",
    )
    .fetch_one(pool)
    .await?;
    if has_migrations {
        let has_legacy_migration: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM _sqlx_migrations
                WHERE success = TRUE AND version BETWEEN 1 AND 2999
            )",
        )
        .fetch_one(pool)
        .await?;
        if has_legacy_migration {
            return Ok(ExistingDatabase::Legacy);
        }
    }
    let has_v3_marker: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = 'schema_generation_v3'
        )",
    )
    .fetch_one(pool)
    .await?;
    let has_v3_migration = if has_migrations {
        sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM _sqlx_migrations
                WHERE success = TRUE AND version >= 3001
            )",
        )
        .fetch_one(pool)
        .await?
    } else {
        false
    };
    // A single familiar table is not enough to claim an arbitrary database.
    // A database created by this Adapter always commits the generation marker
    // and the SQLx migration record together.
    if has_v3_marker && has_v3_migration {
        return Ok(ExistingDatabase::V3);
    }
    let user_table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_one(pool)
    .await?;
    Ok(if user_table_count == 0 {
        ExistingDatabase::Empty
    } else {
        ExistingDatabase::Unknown
    })
}

#[async_trait]
impl FactStore for SqliteV3Store {
    async fn apply(&self, mutation: FactMutation) -> Result<FactMutationOutcome, FactStoreError> {
        self.apply_mutation(mutation).await
    }

    async fn query(&self, query: FactQuery) -> Result<FactQueryOutcome, FactStoreError> {
        self.query_facts(query).await
    }

    async fn claim_work(&self, claim: WorkClaim) -> Result<AgentWorkBatch, FactStoreError> {
        self.claim_work_impl(claim).await
    }

    async fn record_work(
        &self,
        result: StoredWorkResult,
    ) -> Result<AgentWorkRecordOutcome, FactStoreError> {
        self.record_work_impl(result).await
    }
}

async fn applied_migration_version(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if !exists {
        return Ok(0);
    }
    let version: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = TRUE",
    )
    .fetch_one(pool)
    .await?;
    Ok(version.max(0) as u64)
}

#[cfg(unix)]
async fn secure_database_path(path: &Path) -> Result<(), SqliteV3OpenError> {
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(SqliteV3OpenError::SecurePath)
}

#[cfg(not(unix))]
async fn secure_database_path(_path: &Path) -> Result<(), SqliteV3OpenError> {
    Ok(())
}

pub(super) fn storage_error<T>(_error: T) -> FactStoreError {
    FactStoreError::Storage
}

pub(super) fn checked_u32(value: i64) -> Result<u32, FactStoreError> {
    u32::try_from(value).map_err(|_| FactStoreError::CorruptData)
}

pub(super) fn checked_u64(value: i64) -> Result<u64, FactStoreError> {
    u64::try_from(value).map_err(|_| FactStoreError::CorruptData)
}

pub(super) fn required<T>(value: Option<T>) -> Result<T, FactStoreError> {
    value.ok_or(FactStoreError::CorruptData)
}

pub(super) fn parse_json<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, FactStoreError> {
    serde_json::from_str(value).map_err(|_| FactStoreError::CorruptData)
}

pub(super) fn to_json<T: serde::Serialize>(value: &T) -> Result<String, FactStoreError> {
    serde_json::to_string(value).map_err(|_| FactStoreError::Storage)
}

#[cfg(test)]
mod tests;
