use super::*;

impl SqliteFeedbackStore {
    pub(super) async fn create_or_get_request_impl(
        &self,
        request: NewFeedbackRequest,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        if let Some(existing) = load_request_row(&mut transaction, &request.request_id).await? {
            let input_hash = request.immutable_input_hash();
            let stored_hash: String = existing.try_get("input_hash").map_err(storage_error)?;
            return if stored_hash == input_hash {
                stored_request_from_row(&existing)
            } else {
                Err(RepositoryError::RequestConflict)
            };
        }

        let input_hash = request.immutable_input_hash();

        sqlx::query(
            "INSERT INTO host_sessions \
             (id, host_id, host_session_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?4) \
             ON CONFLICT(host_id, host_session_id) DO UPDATE SET updated_at = excluded.updated_at",
        )
        .bind(&request.host_session_record_id)
        .bind(&request.host_id)
        .bind(&request.host_session_id)
        .bind(&request.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        let host_session_record_id: String = sqlx::query_scalar(
            "SELECT id FROM host_sessions \
             WHERE host_id = ?1 AND host_session_id = ?2",
        )
        .bind(&request.host_id)
        .bind(&request.host_session_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;

        let inserted = sqlx::query(
            "INSERT INTO feedback_requests \
             (id, host_session_record_id, title, what_happened, source_hint, status, input_hash, allow_finish, final_summary, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, 'waiting', ?6, ?7, ?8, ?9, ?9) \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(&request.request_id)
        .bind(host_session_record_id)
        .bind(&request.title)
        .bind(&request.what_happened)
        .bind(request.source_hint.as_deref())
        .bind(&input_hash)
        .bind(request.allow_finish)
        .bind(request.final_summary.as_deref())
        .bind(&request.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if inserted.rows_affected() != 1 {
            return Err(RepositoryError::Storage);
        }

        for (position, action) in request.actions.iter().enumerate() {
            sqlx::query(
                "INSERT INTO request_actions \
                 (request_id, action_id, position, instruction) VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(&request.request_id)
            .bind(&action.id)
            .bind(position as i64)
            .bind(&action.instruction)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
        }
        for (position, context_ref) in request.context_refs.iter().enumerate() {
            sqlx::query(
                "INSERT INTO request_context_refs \
                 (request_id, position, label, uri) VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(&request.request_id)
            .bind(position as i64)
            .bind(&context_ref.label)
            .bind(&context_ref.uri)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
        }

        let stored = StoredFeedbackRequest {
            request_id: request.request_id,
            host_id: request.host_id,
            host_session_id: request.host_session_id,
            status: FeedbackStatus::Waiting,
            created_at: request.created_at.clone(),
            updated_at: request.created_at,
            feedback: None,
            resolution: None,
            allow_finish: request.allow_finish,
            final_summary: request.final_summary,
        };
        transaction.commit().await.map_err(storage_error)?;
        Ok(stored)
    }

    pub(super) async fn get_request_impl(
        &self,
        request_id: &str,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        let row = sqlx::query(
            "SELECT r.id, hs.host_id, hs.host_session_id, r.status, r.resolution, r.allow_finish, r.final_summary, \
                    r.created_at, r.updated_at, r.input_hash, fr.package_uri, fr.directory_path, fr.markdown_path, fr.manifest_path \
             FROM feedback_requests r \
             JOIN host_sessions hs ON hs.id = r.host_session_record_id \
             LEFT JOIN feedback_results fr ON fr.request_id = r.id \
             WHERE r.id = ?1",
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?
        .ok_or(RepositoryError::RequestNotFound)?;
        stored_request_from_row(&row)
    }

    pub(super) async fn cancel_request_impl(
        &self,
        request_id: &str,
        reason: &str,
        now: &str,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let row = load_request_row(&mut transaction, request_id)
            .await?
            .ok_or(RepositoryError::RequestNotFound)?;
        match stored_status(&row)? {
            FeedbackStatus::Completed => return Err(RepositoryError::RequestAlreadyCompleted),
            FeedbackStatus::Cancelled => return stored_request_from_row(&row),
            FeedbackStatus::Waiting | FeedbackStatus::InProgress => {}
        }
        let planned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM submission_plans WHERE request_id = ?1)",
        )
        .bind(request_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if planned {
            return Err(RepositoryError::RequestTerminal);
        }

        sqlx::query(
            "UPDATE feedback_requests \
             SET status = 'cancelled', resolution = 'cancelled', cancelled_at = ?2, cancel_reason = ?3, \
                 updated_at = ?2, revision = revision + 1 \
             WHERE id = ?1 AND status IN ('waiting', 'in_progress')",
        )
        .bind(request_id)
        .bind(now)
        .bind(reason)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        let updated = load_request_row(&mut transaction, request_id)
            .await?
            .ok_or(RepositoryError::RequestNotFound)?;
        transaction.commit().await.map_err(storage_error)?;
        stored_request_from_row(&updated)
    }

    pub(super) async fn approve_request_impl(
        &self,
        request_id: &str,
        now: &str,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        let updated = sqlx::query(
            "UPDATE feedback_requests SET status = 'completed', resolution = 'approved', \
             completed_at = ?2, updated_at = ?2, revision = revision + 1 \
             WHERE id = ?1 AND status IN ('waiting', 'in_progress') \
               AND allow_finish = 1 AND final_summary IS NOT NULL",
        )
        .bind(request_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            let existing = self.get_request_impl(request_id).await?;
            return if existing.status == FeedbackStatus::Completed
                && existing.resolution == Some(FeedbackResolution::Approved)
            {
                Ok(existing)
            } else {
                Err(RepositoryError::RequestTerminal)
            };
        }
        self.get_request_impl(request_id).await
    }

    pub(super) async fn list_open_requests_impl(
        &self,
    ) -> Result<Vec<FeedbackRequestSummary>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT r.id, hs.host_id, hs.host_session_id, r.source_hint, \
                    r.title, r.what_happened, r.status, r.resolution, r.allow_finish, r.final_summary, \
                    r.revision, r.created_at, r.updated_at \
             FROM feedback_requests r \
             JOIN host_sessions hs ON hs.id = r.host_session_record_id \
             WHERE r.status IN ('waiting', 'in_progress') \
             ORDER BY r.updated_at DESC, r.id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;
        rows.iter().map(summary_from_row).collect()
    }

    pub(super) async fn list_requests_impl(
        &self,
        query: FeedbackRequestQuery,
    ) -> Result<Vec<FeedbackRequestSummary>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT r.id, hs.host_id, hs.host_session_id, r.source_hint, \
                    r.title, r.what_happened, r.status, r.resolution, r.allow_finish, r.final_summary, \
                    r.revision, r.created_at, r.updated_at \
             FROM feedback_requests r \
             JOIN host_sessions hs ON hs.id = r.host_session_record_id \
             WHERE (?1 IS NULL OR hs.host_id = ?1) \
               AND (?2 IS NULL OR hs.host_session_id = ?2) \
               AND ((?3 AND r.status = 'waiting') \
                 OR (?4 AND r.status = 'in_progress') \
                 OR (?5 AND r.status = 'completed') \
                 OR (?6 AND r.status = 'cancelled')) \
               AND (?7 IS NULL OR r.updated_at < ?7 \
                 OR (r.updated_at = ?7 AND r.id < ?8)) \
             ORDER BY r.updated_at DESC, r.id DESC \
             LIMIT ?9",
        )
        .bind(query.host_id.as_deref())
        .bind(query.host_session_id.as_deref())
        .bind(query.statuses.contains(&FeedbackStatus::Waiting))
        .bind(query.statuses.contains(&FeedbackStatus::InProgress))
        .bind(query.statuses.contains(&FeedbackStatus::Completed))
        .bind(query.statuses.contains(&FeedbackStatus::Cancelled))
        .bind(query.before_updated_at.as_deref())
        .bind(query.before_request_id.as_deref())
        .bind(query.limit as i64 + 1)
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;
        rows.iter().map(summary_from_row).collect()
    }

    pub(super) async fn list_host_sessions_impl(
        &self,
    ) -> Result<Vec<HostSessionSummary>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT hs.host_id, hs.host_session_id, \
                    (SELECT first_request.title \
                     FROM feedback_requests first_request \
                     WHERE first_request.host_session_record_id = hs.id \
                     ORDER BY first_request.created_at, first_request.id LIMIT 1) AS title, \
                    (SELECT first_request.source_hint \
                     FROM feedback_requests first_request \
                     WHERE first_request.host_session_record_id = hs.id \
                     ORDER BY first_request.created_at, first_request.id LIMIT 1) AS source_hint, \
                    COUNT(r.id) AS request_count, \
                    SUM(CASE WHEN r.status IN ('waiting', 'in_progress') THEN 1 ELSE 0 END) AS pending_count, \
                    MAX(r.updated_at) AS updated_at \
             FROM host_sessions hs \
             JOIN feedback_requests r ON r.host_session_record_id = hs.id \
             GROUP BY hs.id, hs.host_id, hs.host_session_id \
             ORDER BY updated_at DESC, hs.host_id, hs.host_session_id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;
        rows.iter().map(host_session_summary_from_row).collect()
    }
}
