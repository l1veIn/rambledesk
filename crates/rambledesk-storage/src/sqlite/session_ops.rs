use std::path::{Path, PathBuf};

use super::*;

const HOST_SESSION_SUMMARY_BY_ID: &str = "\
    SELECT hs.id AS session_id, hs.host_id, hs.host_session_id, \
           ms.protocol, ms.agent_config_id, ms.cwd, ms.remote_session_id, \
           COALESCE(NULLIF(hs.display_title, ''), (SELECT first_request.title \
            FROM feedback_requests first_request \
            WHERE first_request.host_session_record_id = hs.id \
            ORDER BY first_request.created_at, first_request.id LIMIT 1), hs.host_session_id) AS title, \
           COALESCE((SELECT first_request.source_hint \
            FROM feedback_requests first_request \
            WHERE first_request.host_session_record_id = hs.id \
            ORDER BY first_request.created_at, first_request.id LIMIT 1), ms.cwd) AS source_hint, \
           COUNT(r.id) AS request_count, \
           SUM(CASE WHEN r.status IN ('waiting', 'in_progress') THEN 1 ELSE 0 END) AS pending_count, \
           CASE WHEN ms.session_id IS NOT NULL \
                THEN MAX(hs.updated_at, COALESCE(MAX(r.updated_at), hs.updated_at)) \
                ELSE MAX(r.updated_at) END AS updated_at, \
           hs.pinned_at, hs.archived_at, hp.pinned_at AS host_pinned_at \
    FROM host_sessions hs \
    LEFT JOIN feedback_requests r ON r.host_session_record_id = hs.id \
    LEFT JOIN managed_sessions ms ON ms.session_id = hs.id \
    LEFT JOIN host_preferences hp ON hp.host_id = hs.host_id \
    WHERE hs.id = ?1 AND (ms.session_id IS NOT NULL OR r.id IS NOT NULL) \
    GROUP BY hs.id, hs.host_id, hs.host_session_id";

impl SqliteFeedbackStore {
    pub(super) async fn rename_host_session_impl(
        &self,
        host_id: &str,
        host_session_id: &str,
        title: &str,
        now: &str,
    ) -> Result<HostSessionSummary, RepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let record_id = host_session_record_id(&mut transaction, host_id, host_session_id).await?;
        let updated = sqlx::query(
            "UPDATE host_sessions SET display_title = ?2, updated_at = ?3 WHERE id = ?1",
        )
        .bind(&record_id)
        .bind(title)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(RepositoryError::HostSessionNotFound);
        }
        let summary = load_host_session_summary(&mut transaction, &record_id).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(summary)
    }

    pub(super) async fn set_host_session_pinned_impl(
        &self,
        host_id: &str,
        host_session_id: &str,
        pinned_at: Option<&str>,
    ) -> Result<HostSessionSummary, RepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let record_id = host_session_record_id(&mut transaction, host_id, host_session_id).await?;
        let updated = sqlx::query(
            "UPDATE host_sessions SET pinned_at = CASE \
                 WHEN archived_at IS NULL THEN ?2 ELSE NULL END \
             WHERE id = ?1",
        )
        .bind(&record_id)
        .bind(pinned_at)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(RepositoryError::HostSessionNotFound);
        }
        let summary = load_host_session_summary(&mut transaction, &record_id).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(summary)
    }

    pub(super) async fn archive_host_session_impl(
        &self,
        host_id: &str,
        host_session_id: &str,
        now: &str,
    ) -> Result<HostSessionSummary, RepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let record_id = host_session_record_id(&mut transaction, host_id, host_session_id).await?;
        let has_open_request: bool = sqlx::query_scalar(
            "SELECT EXISTS( \
                 SELECT 1 FROM feedback_requests \
                 WHERE host_session_record_id = ?1 \
                   AND status IN ('waiting', 'in_progress') \
             )",
        )
        .bind(&record_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if has_open_request {
            return Err(RepositoryError::HostSessionHasOpenRequests);
        }
        let updated = sqlx::query(
            "UPDATE host_sessions SET archived_at = COALESCE(archived_at, ?2), \
                 pinned_at = NULL, updated_at = ?2 \
             WHERE id = ?1",
        )
        .bind(&record_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(RepositoryError::HostSessionNotFound);
        }
        let summary = load_host_session_summary(&mut transaction, &record_id).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(summary)
    }

    pub(super) async fn unarchive_host_session_impl(
        &self,
        host_id: &str,
        host_session_id: &str,
        now: &str,
    ) -> Result<HostSessionSummary, RepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let record_id = host_session_record_id(&mut transaction, host_id, host_session_id).await?;
        let updated = sqlx::query(
            "UPDATE host_sessions SET archived_at = NULL, updated_at = ?2 WHERE id = ?1",
        )
        .bind(&record_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(RepositoryError::HostSessionNotFound);
        }
        let summary = load_host_session_summary(&mut transaction, &record_id).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(summary)
    }

    pub(super) async fn set_host_pinned_impl(
        &self,
        host_id: &str,
        pinned_at: Option<&str>,
        now: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO host_preferences (host_id, pinned_at, updated_at) \
             VALUES (?1, ?2, ?3) \
             ON CONFLICT(host_id) DO UPDATE SET \
                 pinned_at = excluded.pinned_at, updated_at = excluded.updated_at",
        )
        .bind(host_id)
        .bind(pinned_at)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(storage_error)?;
        Ok(())
    }

    pub(super) async fn delete_host_session_impl(
        &self,
        host_id: &str,
        host_session_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let record_id = host_session_record_id(&mut transaction, host_id, host_session_id).await?;
        let archived_at: Option<String> =
            sqlx::query_scalar("SELECT archived_at FROM host_sessions WHERE id = ?1")
                .bind(&record_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(storage_error)?;
        if archived_at.is_none() {
            return Err(RepositoryError::DeleteRequiresArchivedHostSession);
        }
        let has_open_request: bool = sqlx::query_scalar(
            "SELECT EXISTS( \
                 SELECT 1 FROM feedback_requests \
                 WHERE host_session_record_id = ?1 \
                   AND status IN ('waiting', 'in_progress') \
             )",
        )
        .bind(&record_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if has_open_request {
            return Err(RepositoryError::HostSessionHasOpenRequests);
        }
        let request_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM feedback_requests WHERE host_session_record_id = ?1",
        )
        .bind(&record_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(storage_error)?;
        let artifacts =
            collect_deletion_artifacts(&mut transaction, &self.library_root(), &request_ids)
                .await?;
        for request_id in &request_ids {
            delete_feedback_request_rows(&mut transaction, request_id).await?;
        }
        sqlx::query("DELETE FROM host_sessions WHERE id = ?1")
            .bind(&record_id)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        cleanup_deletion_artifacts(artifacts).await;
        Ok(request_ids)
    }

    pub(super) async fn delete_feedback_request_impl(
        &self,
        request_id: &str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let row = sqlx::query(
            "SELECT r.host_session_record_id, r.status, hs.archived_at \
             FROM feedback_requests r \
             JOIN host_sessions hs ON hs.id = r.host_session_record_id \
             WHERE r.id = ?1",
        )
        .bind(request_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage_error)?
        .ok_or(RepositoryError::RequestNotFound)?;
        let archived_at: Option<String> = row.try_get("archived_at").map_err(storage_error)?;
        if archived_at.is_none() {
            return Err(RepositoryError::DeleteRequiresArchivedHostSession);
        }
        let status = FeedbackStatus::try_from(
            row.try_get::<String, _>("status")
                .map_err(storage_error)?
                .as_str(),
        )?;
        if matches!(status, FeedbackStatus::Waiting | FeedbackStatus::InProgress) {
            return Err(RepositoryError::RequestNotTerminal);
        }
        let record_id: String = row
            .try_get("host_session_record_id")
            .map_err(storage_error)?;
        let artifacts = collect_deletion_artifacts(
            &mut transaction,
            &self.library_root(),
            &[request_id.to_owned()],
        )
        .await?;
        delete_feedback_request_rows(&mut transaction, request_id).await?;
        sqlx::query(
            "DELETE FROM host_sessions \
             WHERE id = ?1 \
               AND NOT EXISTS(SELECT 1 FROM feedback_requests WHERE host_session_record_id = ?1) \
               AND NOT EXISTS(SELECT 1 FROM managed_sessions WHERE session_id = ?1)",
        )
        .bind(&record_id)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        cleanup_deletion_artifacts(artifacts).await;
        Ok(())
    }
}

async fn host_session_record_id(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    host_id: &str,
    host_session_id: &str,
) -> Result<String, RepositoryError> {
    sqlx::query_scalar("SELECT id FROM host_sessions WHERE host_id = ?1 AND host_session_id = ?2")
        .bind(host_id)
        .bind(host_session_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(storage_error)?
        .ok_or(RepositoryError::HostSessionNotFound)
}

async fn load_host_session_summary(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    host_session_record_id: &str,
) -> Result<HostSessionSummary, RepositoryError> {
    sqlx::query(HOST_SESSION_SUMMARY_BY_ID)
        .bind(host_session_record_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(storage_error)?
        .map(|row| host_session_summary_from_row(&row))
        .transpose()?
        .ok_or(RepositoryError::HostSessionNotFound)
}

struct DeletionArtifacts {
    directories: Vec<PathBuf>,
}

async fn collect_deletion_artifacts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    library_root: &Path,
    request_ids: &[String],
) -> Result<DeletionArtifacts, RepositoryError> {
    let canonical_library_root = tokio::fs::canonicalize(library_root)
        .await
        .unwrap_or_else(|_| library_root.to_path_buf());
    let mut directories = Vec::new();
    for request_id in request_ids {
        directories.push(library_root.join("drafts").join(request_id));
        for row in sqlx::query(
            "SELECT directory_path, NULL AS temp_directory_path \
             FROM feedback_results WHERE request_id = ?1 \
             UNION ALL \
             SELECT directory_path, temp_directory_path \
             FROM submission_plans WHERE request_id = ?1",
        )
        .bind(request_id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(storage_error)?
        {
            if let Some(path) = row
                .try_get::<Option<String>, _>("directory_path")
                .map_err(storage_error)?
                .and_then(|value| {
                    removable_library_directory(library_root, &canonical_library_root, &value)
                })
            {
                directories.push(path);
            }
            if let Some(path) = row
                .try_get::<Option<String>, _>("temp_directory_path")
                .map_err(storage_error)?
                .and_then(|value| {
                    removable_library_directory(library_root, &canonical_library_root, &value)
                })
            {
                directories.push(path);
            }
        }
    }
    directories.sort();
    directories.dedup();
    Ok(DeletionArtifacts { directories })
}

fn removable_library_directory(
    library_root: &Path,
    canonical_library_root: &Path,
    value: &str,
) -> Option<PathBuf> {
    let path = PathBuf::from(value);
    if path.starts_with(library_root) || path.starts_with(canonical_library_root) {
        Some(path)
    } else {
        None
    }
}

pub(super) async fn delete_feedback_request_rows(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
) -> Result<(), RepositoryError> {
    sqlx::query("DELETE FROM feedback_results WHERE request_id = ?1")
        .bind(request_id)
        .execute(&mut **transaction)
        .await
        .map_err(storage_error)?;
    sqlx::query("DELETE FROM submission_plans WHERE request_id = ?1")
        .bind(request_id)
        .execute(&mut **transaction)
        .await
        .map_err(storage_error)?;
    let deleted = sqlx::query("DELETE FROM feedback_requests WHERE id = ?1")
        .bind(request_id)
        .execute(&mut **transaction)
        .await
        .map_err(storage_error)?;
    if deleted.rows_affected() != 1 {
        return Err(RepositoryError::RequestNotFound);
    }
    Ok(())
}

async fn cleanup_deletion_artifacts(artifacts: DeletionArtifacts) {
    for directory in artifacts.directories {
        let _ = tokio::fs::remove_dir_all(directory).await;
    }
}
