use std::{io::ErrorKind, path::Path};

use sqlx::SqlitePool;

use super::{StorageOpenError, security::secure_path};

const BACKUPS_TO_KEEP: usize = 3;

pub(super) async fn before_migration(
    database_path: &Path,
    pool: &SqlitePool,
    database_existed: bool,
) -> Result<(), StorageOpenError> {
    if !database_existed {
        return Ok(());
    }

    let parent = database_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = database_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("rambledesk");
    let prefix = format!("{stem}.pre-migration-");
    let backup_path = parent.join(format!("{prefix}v{}.sqlite3", env!("CARGO_PKG_VERSION")));
    if tokio::fs::try_exists(&backup_path)
        .await
        .map_err(StorageOpenError::ManageBackup)?
    {
        return Ok(());
    }

    let temporary_path = backup_path.with_extension("sqlite3.tmp");
    match tokio::fs::remove_file(&temporary_path).await {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(StorageOpenError::ManageBackup(error)),
    }

    sqlx::query("VACUUM INTO ?1")
        .bind(temporary_path.to_string_lossy().as_ref())
        .execute(pool)
        .await
        .map_err(StorageOpenError::BackupDatabase)?;
    secure_path(&temporary_path, 0o600).await?;
    tokio::fs::rename(&temporary_path, &backup_path)
        .await
        .map_err(StorageOpenError::ManageBackup)?;
    prune_old_backups(parent, &prefix).await;
    Ok(())
}

async fn prune_old_backups(parent: &Path, prefix: &str) {
    let Ok(mut entries) = tokio::fs::read_dir(parent).await else {
        return;
    };
    let mut backups = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(prefix) || !name.ends_with(".sqlite3") {
            continue;
        }
        let Ok(metadata) = entry.metadata().await else {
            continue;
        };
        if metadata.is_file() {
            backups.push((metadata.modified().ok(), entry.path()));
        }
    }
    backups.sort_by_key(|(modified, _)| *modified);
    let remove_count = backups.len().saturating_sub(BACKUPS_TO_KEEP);
    for (_, path) in backups.into_iter().take(remove_count) {
        let _ = tokio::fs::remove_file(path).await;
    }
}
