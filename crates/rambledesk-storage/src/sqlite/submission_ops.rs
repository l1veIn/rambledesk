use super::*;

impl SqliteFeedbackStore {
    pub(super) async fn plan_submission_impl(
        &self,
        request_id: &str,
        expected_revision: u64,
        cooked_markdown: Option<&str>,
        cooking_model: Option<&str>,
        publication_id: &str,
        now: &str,
    ) -> Result<SubmissionPlan, RepositoryError> {
        let preflight = sqlx::query(
            "SELECT r.status, \
                    EXISTS(SELECT 1 FROM submission_plans sp WHERE sp.request_id = r.id) AS planned \
             FROM feedback_requests r \
             WHERE r.id = ?1",
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?
        .ok_or(RepositoryError::RequestNotFound)?;
        let preflight_status: String = preflight.try_get("status").map_err(storage_error)?;
        if matches!(
            FeedbackStatus::try_from(preflight_status.as_str())?,
            FeedbackStatus::Completed | FeedbackStatus::Cancelled
        ) {
            return Err(RepositoryError::RequestTerminal);
        }
        let already_planned: bool = preflight.try_get("planned").map_err(storage_error)?;
        let prepared_paths = if already_planned {
            None
        } else {
            Some(
                prepare_publication_paths(request_id, publication_id, now, &self.library_root)
                    .await?,
            )
        };

        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let row = load_submission_row(&mut transaction, request_id)
            .await?
            .ok_or(RepositoryError::RequestNotFound)?;
        let status: String = row.try_get("status").map_err(storage_error)?;
        if matches!(
            FeedbackStatus::try_from(status.as_str())?,
            FeedbackStatus::Completed | FeedbackStatus::Cancelled
        ) {
            return Err(RepositoryError::RequestTerminal);
        }
        let body_markdown: String = row
            .try_get::<Option<String>, _>("body_markdown")
            .map_err(storage_error)?
            .ok_or(RepositoryError::DraftEmpty)?;
        if body_markdown.trim().is_empty() {
            return Err(RepositoryError::DraftEmpty);
        }
        let aggregate_revision: i64 = row.try_get("request_revision").map_err(storage_error)?;
        let saved_revision: i64 = row
            .try_get::<Option<i64>, _>("draft_revision")
            .map_err(storage_error)?
            .ok_or(RepositoryError::DraftEmpty)?;
        let body_sha256 = hex::encode(Sha256::digest(body_markdown.as_bytes()));
        let actions = load_actions(&mut transaction, request_id).await?;
        let attachments = load_submission_attachments(&mut transaction, request_id).await?;

        if let Some(source_revision) = row
            .try_get::<Option<i64>, _>("source_revision")
            .map_err(storage_error)?
        {
            if source_revision != expected_revision as i64 {
                return Err(RepositoryError::DraftConflict);
            }
            let stored_hash: String = row.try_get("body_sha256").map_err(storage_error)?;
            let stored_cooked: Option<String> =
                row.try_get("cooked_markdown").map_err(storage_error)?;
            let stored_model: Option<String> =
                row.try_get("cooking_model").map_err(storage_error)?;
            if stored_hash != body_sha256
                || stored_cooked.as_deref() != cooked_markdown
                || stored_model.as_deref() != cooking_model
            {
                return Err(RepositoryError::DraftConflict);
            }
            let plan = submission_plan_from_row(&row, actions, attachments, body_markdown)?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(plan);
        }
        if aggregate_revision != expected_revision as i64 || saved_revision != aggregate_revision {
            return Err(RepositoryError::DraftConflict);
        }

        let prepared_paths = prepared_paths.ok_or(RepositoryError::CorruptData)?;

        sqlx::query(
            "INSERT INTO submission_plans \
             (request_id, publication_id, source_revision, body_sha256, cooked_markdown, \
              cooking_model, submitted_at, package_uri, directory_path, temp_directory_path, \
              markdown_path, manifest_path) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )
        .bind(request_id)
        .bind(publication_id)
        .bind(aggregate_revision)
        .bind(&body_sha256)
        .bind(cooked_markdown)
        .bind(cooking_model)
        .bind(now)
        .bind(&prepared_paths.package_uri)
        .bind(&prepared_paths.directory_path)
        .bind(&prepared_paths.temp_directory_path)
        .bind(&prepared_paths.markdown_path)
        .bind(&prepared_paths.manifest_path)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;

        let plan = SubmissionPlan {
            request_id: request_id.to_owned(),
            host_id: row.try_get("host_id").map_err(storage_error)?,
            host_session_id: row.try_get("host_session_id").map_err(storage_error)?,
            source_hint: row.try_get("source_hint").map_err(storage_error)?,
            title: row.try_get("title").map_err(storage_error)?,
            what_happened: row.try_get("what_happened").map_err(storage_error)?,
            actions,
            attachments,
            body_markdown: cooked_markdown.unwrap_or(&body_markdown).to_owned(),
            uncooked_markdown: body_markdown,
            cooking_model: cooking_model.map(ToOwned::to_owned),
            source_revision: aggregate_revision as u64,
            publication_id: publication_id.to_owned(),
            body_sha256,
            submitted_at: now.to_owned(),
            package_uri: prepared_paths.package_uri,
            directory_path: prepared_paths.directory_path,
            temp_directory_path: prepared_paths.temp_directory_path,
            markdown_path: prepared_paths.markdown_path,
            manifest_path: prepared_paths.manifest_path,
        };
        transaction.commit().await.map_err(storage_error)?;
        Ok(plan)
    }

    pub(super) async fn complete_submission_impl(
        &self,
        plan: &SubmissionPlan,
        published: &PublishedFeedbackPackage,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let status: String =
            sqlx::query_scalar("SELECT status FROM feedback_requests WHERE id = ?1")
                .bind(&plan.request_id)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(storage_error)?
                .ok_or(RepositoryError::RequestNotFound)?;
        if status == "completed" {
            let stored = load_request_row(&mut transaction, &plan.request_id)
                .await?
                .ok_or(RepositoryError::CorruptData)
                .and_then(|row| stored_request_from_row(&row))?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(stored);
        }
        if status == "cancelled" {
            return Err(RepositoryError::RequestTerminal);
        }
        if published.result.package_uri != plan.package_uri
            || published.result.directory_path != plan.directory_path
            || published.result.markdown_path != plan.markdown_path
            || published.result.manifest_path != plan.manifest_path
        {
            return Err(RepositoryError::PackagePublish);
        }
        let stored_plan: (String, i64, String) = sqlx::query_as(
            "SELECT publication_id, source_revision, body_sha256 \
             FROM submission_plans WHERE request_id = ?1",
        )
        .bind(&plan.request_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage_error)?
        .ok_or(RepositoryError::CorruptData)?;
        if stored_plan.0 != plan.publication_id
            || stored_plan.1 != plan.source_revision as i64
            || stored_plan.2 != plan.body_sha256
        {
            return Err(RepositoryError::DraftConflict);
        }

        sqlx::query(
            "INSERT INTO feedback_results \
             (request_id, package_uri, directory_path, markdown_path, manifest_path, \
              manifest_sha256, published_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(&plan.request_id)
        .bind(&published.result.package_uri)
        .bind(&published.result.directory_path)
        .bind(&published.result.markdown_path)
        .bind(&published.result.manifest_path)
        .bind(&published.manifest_sha256)
        .bind(&published.published_at)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        sqlx::query(
            "UPDATE submission_plans SET state = 'published', manifest_sha256 = ?2, \
                 published_at = ?3, last_error_code = NULL, last_error_at = NULL \
             WHERE request_id = ?1 AND state = 'preparing'",
        )
        .bind(&plan.request_id)
        .bind(&published.manifest_sha256)
        .bind(&published.published_at)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        for attachment in &plan.attachments {
            let published_path = Path::new(&plan.directory_path).join(&attachment.relative_path);
            let updated = sqlx::query(
                "UPDATE attachments SET published_path = ?3 \
                 WHERE request_id = ?1 AND id = ?2 AND sha256 = ?4",
            )
            .bind(&plan.request_id)
            .bind(&attachment.attachment_id)
            .bind(path_string(&published_path)?)
            .bind(&attachment.sha256)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
            if updated.rows_affected() != 1 {
                return Err(RepositoryError::CorruptData);
            }
        }
        let completed = sqlx::query(
            "UPDATE feedback_requests SET \
                 status = 'completed', resolution = 'feedback_submitted', completed_at = ?2, updated_at = ?2, revision = revision + 1 \
             WHERE id = ?1 AND status = 'in_progress' AND revision = ?3",
        )
        .bind(&plan.request_id)
        .bind(&published.published_at)
        .bind(plan.source_revision as i64)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if completed.rows_affected() != 1 {
            return Err(RepositoryError::DraftConflict);
        }
        let stored = load_request_row(&mut transaction, &plan.request_id)
            .await?
            .ok_or(RepositoryError::CorruptData)
            .and_then(|row| stored_request_from_row(&row))?;
        transaction.commit().await.map_err(storage_error)?;
        for attachment in &plan.attachments {
            let _ = tokio::fs::remove_file(&attachment.draft_path).await;
        }
        let _ = tokio::fs::remove_dir_all(self.library_root.join("drafts").join(&plan.request_id))
            .await;
        Ok(stored)
    }
}
