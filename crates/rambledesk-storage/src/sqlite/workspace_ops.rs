use super::*;
use tokio::io::AsyncWriteExt;

impl SqliteFeedbackStore {
    pub(super) async fn get_workspace_impl(
        &self,
        request_id: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError> {
        load_workspace_from_pool(&self.pool, request_id).await
    }

    pub(super) async fn save_draft_impl(
        &self,
        request_id: &str,
        body_markdown: &str,
        expected_revision: u64,
        now: &str,
    ) -> Result<DraftView, RepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let request_row =
            sqlx::query("SELECT status, revision FROM feedback_requests WHERE id = ?1")
                .bind(request_id)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(storage_error)?
                .ok_or(RepositoryError::RequestNotFound)?;
        let status: String = request_row.try_get("status").map_err(storage_error)?;
        if matches!(
            FeedbackStatus::try_from(status.as_str())?,
            FeedbackStatus::Completed | FeedbackStatus::Cancelled
        ) {
            return Err(RepositoryError::RequestTerminal);
        }
        let planned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM submission_plans WHERE request_id = ?1)",
        )
        .bind(request_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if planned {
            return Err(RepositoryError::DraftConflict);
        }

        let current_revision: i64 = request_row.try_get("revision").map_err(storage_error)?;
        let stored_draft = sqlx::query(
            "SELECT body_markdown, revision, updated_at FROM drafts WHERE request_id = ?1",
        )
        .bind(request_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if current_revision != expected_revision as i64 {
            if let Some(row) = stored_draft {
                let stored_body: String = row.try_get("body_markdown").map_err(storage_error)?;
                if stored_body == body_markdown {
                    return Ok(DraftView {
                        body_markdown: stored_body,
                        saved_revision: row.try_get::<i64, _>("revision").map_err(storage_error)?
                            as u64,
                        updated_at: Some(row.try_get("updated_at").map_err(storage_error)?),
                    });
                }
            }
            return Err(RepositoryError::DraftConflict);
        }
        let next_revision = current_revision + 1;
        let updated = sqlx::query(
            "UPDATE feedback_requests SET \
                 status = 'in_progress', started_at = COALESCE(started_at, ?3), \
                 updated_at = ?3, revision = ?2 \
             WHERE id = ?1 AND revision = ?4 AND status IN ('waiting', 'in_progress')",
        )
        .bind(request_id)
        .bind(next_revision)
        .bind(now)
        .bind(current_revision)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(RepositoryError::DraftConflict);
        }
        sqlx::query(
            "INSERT INTO drafts (request_id, body_markdown, revision, updated_at) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(request_id) DO UPDATE SET \
                 body_markdown = excluded.body_markdown, \
                 revision = excluded.revision, \
                 updated_at = excluded.updated_at",
        )
        .bind(request_id)
        .bind(body_markdown)
        .bind(next_revision)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(DraftView {
            body_markdown: body_markdown.to_owned(),
            saved_revision: next_revision as u64,
            updated_at: Some(now.to_owned()),
        })
    }

    pub(super) async fn add_attachment_impl(
        &self,
        request_id: &str,
        attachment: NewAttachment,
        expected_revision: u64,
        now: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError> {
        let directory = self
            .library_root
            .join("drafts")
            .join(request_id)
            .join("attachments");
        tokio::fs::create_dir_all(&directory)
            .await
            .map_err(storage_error)?;
        let stored_name = format!(
            "{}-{}",
            attachment.attachment_id,
            portable_file_name(&attachment.file_name)
        );
        let draft_path = directory.join(stored_name);
        let write_result = async {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&draft_path)
                .await
                .map_err(storage_error)?;
            file.write_all(&attachment.contents)
                .await
                .map_err(storage_error)?;
            file.flush().await.map_err(storage_error)?;
            file.sync_all().await.map_err(storage_error)
        }
        .await;
        if let Err(error) = write_result {
            let _ = tokio::fs::remove_file(&draft_path).await;
            return Err(error);
        }

        let mutation = async {
            let mut transaction = self
                .pool
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(storage_error)?;
            let current_revision = ensure_attachment_mutable(
                &mut transaction,
                request_id,
                expected_revision,
            )
            .await?;
            let attachment_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM attachments WHERE request_id = ?1")
                    .bind(request_id)
                    .fetch_one(&mut *transaction)
                    .await
                    .map_err(storage_error)?;
            if attachment_count >= MAX_ATTACHMENT_COUNT as i64 {
                return Err(RepositoryError::AttachmentLimit);
            }
            let next_position: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM attachments WHERE request_id = ?1",
            )
            .bind(request_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(storage_error)?;
            sqlx::query(
                "INSERT INTO attachments \
                 (id, request_id, draft_path, file_name, byte_size, media_type, sha256, position, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .bind(&attachment.attachment_id)
            .bind(request_id)
            .bind(path_string(&draft_path)?)
            .bind(&attachment.file_name)
            .bind(attachment.contents.len() as i64)
            .bind(&attachment.media_type)
            .bind(&attachment.sha256)
            .bind(next_position)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
            advance_attachment_revision(&mut transaction, request_id, current_revision, now)
                .await?;
            transaction.commit().await.map_err(storage_error)
        }
        .await;
        if let Err(error) = mutation {
            let _ = tokio::fs::remove_file(&draft_path).await;
            return Err(error);
        }
        load_workspace_from_pool(&self.pool, request_id).await
    }

    pub(super) async fn remove_attachment_impl(
        &self,
        request_id: &str,
        attachment_id: &str,
        expected_revision: u64,
        now: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let current_revision =
            ensure_attachment_mutable(&mut transaction, request_id, expected_revision).await?;
        let draft_path: String = sqlx::query_scalar(
            "SELECT draft_path FROM attachments WHERE request_id = ?1 AND id = ?2",
        )
        .bind(request_id)
        .bind(attachment_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage_error)?
        .ok_or(RepositoryError::AttachmentNotFound)?;
        sqlx::query("DELETE FROM attachments WHERE request_id = ?1 AND id = ?2")
            .bind(request_id)
            .bind(attachment_id)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
        advance_attachment_revision(&mut transaction, request_id, current_revision, now).await?;
        transaction.commit().await.map_err(storage_error)?;
        let _ = tokio::fs::remove_file(draft_path).await;
        load_workspace_from_pool(&self.pool, request_id).await
    }

    pub(super) async fn reorder_attachments_impl(
        &self,
        request_id: &str,
        attachment_ids: &[String],
        expected_revision: u64,
        now: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError> {
        let unique = attachment_ids.iter().collect::<HashSet<_>>();
        if unique.len() != attachment_ids.len() {
            return Err(RepositoryError::AttachmentNotFound);
        }
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let current_revision =
            ensure_attachment_mutable(&mut transaction, request_id, expected_revision).await?;
        let stored_ids: Vec<String> =
            sqlx::query_scalar("SELECT id FROM attachments WHERE request_id = ?1")
                .bind(request_id)
                .fetch_all(&mut *transaction)
                .await
                .map_err(storage_error)?;
        if stored_ids.len() != attachment_ids.len()
            || !stored_ids.iter().all(|id| unique.contains(id))
        {
            return Err(RepositoryError::AttachmentNotFound);
        }
        let unchanged = sqlx::query_scalar::<_, String>(
            "SELECT id FROM attachments WHERE request_id = ?1 ORDER BY position",
        )
        .bind(request_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(storage_error)?
            == attachment_ids;
        if unchanged {
            transaction.commit().await.map_err(storage_error)?;
            return load_workspace_from_pool(&self.pool, request_id).await;
        }
        sqlx::query("UPDATE attachments SET position = position + 100000 WHERE request_id = ?1")
            .bind(request_id)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
        for (position, attachment_id) in attachment_ids.iter().enumerate() {
            sqlx::query("UPDATE attachments SET position = ?3 WHERE request_id = ?1 AND id = ?2")
                .bind(request_id)
                .bind(attachment_id)
                .bind(position as i64)
                .execute(&mut *transaction)
                .await
                .map_err(storage_error)?;
        }
        advance_attachment_revision(&mut transaction, request_id, current_revision, now).await?;
        transaction.commit().await.map_err(storage_error)?;
        load_workspace_from_pool(&self.pool, request_id).await
    }

    pub(super) async fn read_attachment_impl(
        &self,
        request_id: &str,
        attachment_id: &str,
    ) -> Result<Vec<u8>, RepositoryError> {
        let draft_path: String = sqlx::query_scalar(
            "SELECT COALESCE(published_path, draft_path) FROM attachments WHERE request_id = ?1 AND id = ?2",
        )
        .bind(request_id)
        .bind(attachment_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?
        .ok_or(RepositoryError::AttachmentNotFound)?;
        tokio::fs::read(draft_path).await.map_err(storage_error)
    }
}
