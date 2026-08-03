use sha2::{Digest, Sha384};
use sqlx::{SqlitePool, migrate::Migrator};

pub(super) async fn repair_line_ending_checksums(
    pool: &SqlitePool,
    migrator: &Migrator,
) -> Result<(), sqlx::Error> {
    let table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if !table_exists {
        return Ok(());
    }

    let applied: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations WHERE success = TRUE")
            .fetch_all(pool)
            .await?;
    let mut transaction = pool.begin().await?;
    for (version, applied_checksum) in applied {
        let Some(migration) = migrator
            .iter()
            .find(|migration| migration.version == version)
        else {
            continue;
        };
        if applied_checksum == migration.checksum.as_ref() {
            continue;
        }

        let normalized = migration.sql.replace("\r\n", "\n").replace('\r', "\n");
        let lf_checksum = Sha384::digest(normalized.as_bytes());
        let crlf_checksum = Sha384::digest(normalized.replace('\n', "\r\n").as_bytes());
        let known_line_ending_variant = applied_checksum == lf_checksum.as_slice()
            || applied_checksum == crlf_checksum.as_slice();
        let embedded_is_normalized_variant = migration.checksum.as_ref() == lf_checksum.as_slice()
            || migration.checksum.as_ref() == crlf_checksum.as_slice();
        if known_line_ending_variant && embedded_is_normalized_variant {
            sqlx::query("UPDATE _sqlx_migrations SET checksum = ?2 WHERE version = ?1")
                .bind(version)
                .bind(migration.checksum.as_ref())
                .execute(&mut *transaction)
                .await?;
        }
    }
    transaction.commit().await
}
