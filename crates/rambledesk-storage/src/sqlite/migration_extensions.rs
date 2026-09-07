//! Keep the v0.3.3 storage contract readable during a downgrade.
//!
//! v0.3.3 rejects any SQLx migration above 10, even if it only adds optional
//! columns or independent tables. The audited ACP additions have their own
//! ledger. This is not an instruction to hide arbitrary future schema changes:
//! incompatible changes must advance the legacy compatibility boundary.

use std::collections::BTreeMap;
use std::time::Instant;

use sha2::{Digest, Sha384};
use sqlx::{Executor, SqlitePool, migrate::MigrateError};

use super::{MIGRATOR, StorageOpenError};

pub(super) const LEGACY_VERSION: i64 = 10;
// Explicitly audited migrations; a new migration must choose its compatibility
// policy and update the round-trip tests rather than silently joining this list.
const LAST_EXTENSION: i64 = 20;

type Applied = (i64, bool, Vec<u8>);

fn validate(rows: &[Applied], extensions: bool) -> Result<(), StorageOpenError> {
    for (version, success, checksum) in rows {
        if *version > LAST_EXTENSION {
            return Err(StorageOpenError::NewerDatabase {
                applied: *version as u64,
                supported: LAST_EXTENSION as u64,
            });
        }
        let migration = MIGRATOR
            .iter()
            .find(|migration| migration.version == *version)
            .filter(|_| !extensions || *version > LEGACY_VERSION)
            .ok_or_else(|| StorageOpenError::Migrate(MigrateError::VersionMissing(*version)))?;
        if !success {
            return Err(StorageOpenError::Migrate(MigrateError::Dirty(*version)));
        }
        let normalized = migration.sql.replace("\r\n", "\n").replace('\r', "\n");
        let valid = checksum == migration.checksum.as_ref()
            || checksum == Sha384::digest(normalized.as_bytes()).as_slice()
            || checksum == Sha384::digest(normalized.replace('\n', "\r\n").as_bytes()).as_slice();
        if !valid {
            return Err(StorageOpenError::Migrate(MigrateError::VersionMismatch(
                *version,
            )));
        }
    }
    Ok(())
}

pub(super) async fn run(pool: &SqlitePool) -> Result<(), StorageOpenError> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(migrate_error)?;
    transaction
        .execute(
            "CREATE TABLE IF NOT EXISTS _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS _rambledesk_extensions (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            );",
        )
        .await
        .map_err(migrate_error)?;
    let legacy: Vec<Applied> =
        sqlx::query_as("SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&mut *transaction)
            .await
            .map_err(migrate_error)?;
    let extensions: Vec<Applied> = sqlx::query_as(
        "SELECT version, success, checksum FROM _rambledesk_extensions ORDER BY version",
    )
    .fetch_all(&mut *transaction)
    .await
    .map_err(migrate_error)?;
    // Validate both histories before moving a row or executing any schema SQL.
    validate(&legacy, false)?;
    validate(&extensions, true)?;
    let applied: BTreeMap<_, _> = legacy
        .iter()
        .chain(&extensions)
        .map(|(version, _, checksum)| (*version, checksum))
        .collect();

    // Existing ACP development builds wrote these records to SQLx's ledger.
    // Relocate their history, preserving timestamps, without replaying SQL or
    // deleting any application data. BEGIN IMMEDIATE makes concurrent openers
    // serialize and a failed migration rolls back the entire relocation.
    sqlx::query(
        "INSERT OR IGNORE INTO _rambledesk_extensions
            (version, description, installed_on, success, checksum, execution_time)
         SELECT version, description, installed_on, success, checksum, execution_time
         FROM _sqlx_migrations WHERE version > ?1 AND version <= ?2",
    )
    .bind(LEGACY_VERSION)
    .bind(LAST_EXTENSION)
    .execute(&mut *transaction)
    .await
    .map_err(migrate_error)?;
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version > ?1 AND version <= ?2")
        .bind(LEGACY_VERSION)
        .bind(LAST_EXTENSION)
        .execute(&mut *transaction)
        .await
        .map_err(migrate_error)?;

    for migration in MIGRATOR.iter() {
        if migration.version > LAST_EXTENSION {
            return Err(StorageOpenError::Migrate(MigrateError::VersionNotPresent(
                migration.version,
            )));
        }
        let table = if migration.version <= LEGACY_VERSION {
            "_sqlx_migrations"
        } else {
            "_rambledesk_extensions"
        };
        if let Some(checksum) = applied.get(&migration.version) {
            // Canonicalize only verified LF/CRLF variants, never arbitrary edits.
            // A normal reopen must not rewrite every migration row.
            if checksum.as_slice() != migration.checksum.as_ref() {
                sqlx::query(&format!(
                    "UPDATE {table} SET checksum = ?2 WHERE version = ?1"
                ))
                .bind(migration.version)
                .bind(migration.checksum.as_ref())
                .execute(&mut *transaction)
                .await
                .map_err(migrate_error)?;
            }
            continue;
        }
        let started = Instant::now();
        transaction
            .execute(migration.sql.as_ref())
            .await
            .map_err(|error| {
                StorageOpenError::Migrate(MigrateError::ExecuteMigration(error, migration.version))
            })?;
        sqlx::query(&format!(
            "INSERT INTO {table} (version, description, success, checksum, execution_time)
             VALUES (?1, ?2, TRUE, ?3, ?4)"
        ))
        .bind(migration.version)
        .bind(migration.description.as_ref())
        .bind(migration.checksum.as_ref())
        .bind(started.elapsed().as_nanos().min(i64::MAX as u128) as i64)
        .execute(&mut *transaction)
        .await
        .map_err(migrate_error)?;
    }
    transaction.commit().await.map_err(migrate_error)
}

fn migrate_error(error: sqlx::Error) -> StorageOpenError {
    StorageOpenError::Migrate(MigrateError::Execute(error))
}
