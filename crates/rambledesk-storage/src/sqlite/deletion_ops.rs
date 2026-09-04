use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use rambledesk_core::{
    DeletedManagedSession, SessionDeletionRepository, SessionRepositoryError, SubmissionPlan,
};
use sqlx::Row;

use super::{RepositoryError, SqliteFeedbackStore};

#[async_trait]
impl SessionDeletionRepository for SqliteFeedbackStore {
    async fn begin_managed_session_deletion(
        &self,
        session_id: &str,
        now: &str,
    ) -> Result<(), SessionRepositoryError> {
        if now.trim().is_empty() || now.contains('\0') {
            return Err(SessionRepositoryError::InvalidInput);
        }
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        require_managed(&mut transaction, session_id).await?;
        sqlx::query("INSERT INTO session_deletions(session_id,started_at) VALUES (?1,?2) ON CONFLICT(session_id) DO NOTHING")
            .bind(session_id).bind(now).execute(&mut *transaction).await.map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(())
    }

    async fn is_managed_session_deleting(
        &self,
        session_id: &str,
    ) -> Result<bool, SessionRepositoryError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM session_deletions WHERE session_id=?1)")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage_error)
    }

    async fn list_managed_session_deletions(&self) -> Result<Vec<String>, SessionRepositoryError> {
        sqlx::query_scalar(
            "SELECT session_id FROM session_deletions ORDER BY started_at,session_id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)
    }

    async fn delete_managed_session_data(
        &self,
        session_id: &str,
    ) -> Result<DeletedManagedSession, SessionRepositoryError> {
        // Publication and deletion always acquire this lock before a write transaction.
        // A publisher holding an old plan must recheck it after obtaining the lock.
        let _publication = self.publish_lock.lock().await;
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let row = require_managed(&mut transaction, session_id).await?;
        let intent: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM session_deletions WHERE session_id=?1)",
        )
        .bind(session_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if !intent {
            return Err(SessionRepositoryError::Conflict);
        }
        let request_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM feedback_requests WHERE host_session_record_id=?1 ORDER BY id",
        )
        .bind(session_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(storage_error)?;
        let library_root = self.library_root();
        let canonical_root = tokio::fs::canonicalize(&library_root)
            .await
            .map_err(storage_error)?;
        let directories = owned_directories(
            &mut transaction,
            &library_root,
            &canonical_root,
            &request_ids,
        )
        .await?;
        // Check every target before beginning irreversible filesystem cleanup.
        for directory in &directories {
            validate_resolved_directory(&canonical_root, directory).await?;
        }
        for directory in &directories {
            validate_resolved_directory(&canonical_root, directory).await?;
            match tokio::fs::remove_dir_all(directory).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(SessionRepositoryError::Storage),
            }
        }
        for request_id in &request_ids {
            super::session_ops::delete_feedback_request_rows(&mut transaction, request_id)
                .await
                .map_err(storage_error)?;
        }
        sqlx::query("DELETE FROM host_sessions WHERE id=?1")
            .bind(session_id)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
        let deleted = DeletedManagedSession {
            session_id: session_id.into(),
            host_id: row.try_get("host_id").map_err(storage_error)?,
            host_session_id: row.try_get("host_session_id").map_err(storage_error)?,
            request_ids,
        };
        transaction.commit().await.map_err(storage_error)?;
        Ok(deleted)
    }
}

impl SqliteFeedbackStore {
    /// Must be called while holding publish_lock. Prevents delayed publication
    /// from recreating a package after its managed request has been deleted.
    pub(crate) async fn ensure_publication_plan_live(
        &self,
        plan: &SubmissionPlan,
    ) -> Result<(), RepositoryError> {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM submission_plans sp JOIN feedback_requests r ON r.id=sp.request_id \
             WHERE sp.request_id=?1 AND sp.publication_id=?2 \
               AND NOT EXISTS(SELECT 1 FROM session_deletions sd WHERE sd.session_id=r.host_session_record_id))",
        ).bind(&plan.request_id).bind(&plan.publication_id).fetch_one(&self.pool).await.map_err(super::storage_error)?;
        if valid {
            Ok(())
        } else {
            Err(RepositoryError::RequestNotFound)
        }
    }
}

async fn require_managed(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    session_id: &str,
) -> Result<sqlx::sqlite::SqliteRow, SessionRepositoryError> {
    let row = sqlx::query("SELECT hs.host_id,hs.host_session_id,ms.session_id AS managed_id FROM host_sessions hs LEFT JOIN managed_sessions ms ON ms.session_id=hs.id WHERE hs.id=?1")
        .bind(session_id).fetch_optional(&mut **transaction).await.map_err(storage_error)?
        .ok_or(SessionRepositoryError::SessionNotFound)?;
    if row
        .try_get::<Option<String>, _>("managed_id")
        .map_err(storage_error)?
        .is_none()
    {
        return Err(SessionRepositoryError::Conflict);
    }
    Ok(row)
}

// Discover the same draft/result/submission-plan locations as the existing
// external-session deletion path, but reject unsafe locations instead of skipping
// them and reporting success with artifacts left behind.
async fn owned_directories(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    library_root: &Path,
    canonical_root: &Path,
    request_ids: &[String],
) -> Result<Vec<PathBuf>, SessionRepositoryError> {
    let mut directories = Vec::new();
    for request_id in request_ids {
        if !single_component(request_id) {
            return Err(SessionRepositoryError::CorruptData);
        }
        directories.push(canonical_root.join("drafts").join(request_id));
        let rows = sqlx::query("SELECT directory_path,NULL AS temp_directory_path FROM feedback_results WHERE request_id=?1 UNION ALL SELECT directory_path,temp_directory_path FROM submission_plans WHERE request_id=?1")
            .bind(request_id).fetch_all(&mut **transaction).await.map_err(storage_error)?;
        for row in rows {
            for column in ["directory_path", "temp_directory_path"] {
                let Some(value) = row
                    .try_get::<Option<String>, _>(column)
                    .map_err(storage_error)?
                else {
                    continue;
                };
                let path = PathBuf::from(value);
                if !path.is_absolute()
                    || path
                        .components()
                        .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
                {
                    return Err(SessionRepositoryError::CorruptData);
                }
                let relative = path
                    .strip_prefix(library_root)
                    .or_else(|_| path.strip_prefix(canonical_root))
                    .map_err(|_| SessionRepositoryError::CorruptData)?;
                let components = relative.components().collect::<Vec<_>>();
                if components.len() != 2 || components[0].as_os_str() != "feedback" {
                    return Err(SessionRepositoryError::CorruptData);
                }
                let name = components[1]
                    .as_os_str()
                    .to_str()
                    .ok_or(SessionRepositoryError::CorruptData)?;
                if !(name.ends_with(&format!("-{request_id}"))
                    || name.starts_with(&format!(".{request_id}.tmp-")))
                {
                    return Err(SessionRepositoryError::CorruptData);
                }
                directories.push(canonical_root.join(relative));
            }
        }
    }
    directories.sort();
    directories.dedup();
    Ok(directories)
}

async fn validate_resolved_directory(
    canonical_root: &Path,
    directory: &Path,
) -> Result<(), SessionRepositoryError> {
    let relative = directory
        .strip_prefix(canonical_root)
        .map_err(|_| SessionRepositoryError::CorruptData)?;
    if relative.as_os_str().is_empty() {
        return Err(SessionRepositoryError::CorruptData);
    }
    let mut current = canonical_root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(SessionRepositoryError::CorruptData);
        }
        current.push(component.as_os_str());
        match tokio::fs::symlink_metadata(&current).await {
            Ok(metadata) => {
                if reparse_or_symlink(&metadata) {
                    return Err(SessionRepositoryError::CorruptData);
                }
                if !metadata.is_dir() {
                    return Err(SessionRepositoryError::Storage);
                }
                let resolved = tokio::fs::canonicalize(&current)
                    .await
                    .map_err(storage_error)?;
                if resolved == canonical_root || !resolved.starts_with(canonical_root) {
                    return Err(SessionRepositoryError::CorruptData);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(SessionRepositoryError::Storage),
        }
    }
    Ok(())
}

fn reparse_or_symlink(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn single_component(value: &str) -> bool {
    !value.is_empty() && !value.contains(['/', '\\', ':', '\0']) && !matches!(value, "." | "..")
}

fn storage_error<T>(_error: T) -> SessionRepositoryError {
    SessionRepositoryError::Storage
}
