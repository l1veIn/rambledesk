use super::*;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

struct StagedRequestAttachment {
    attachment_id: String,
    draft_path: std::path::PathBuf,
}

#[derive(Serialize)]
struct LegacyImmutableRequest<'a> {
    host_id: &'a str,
    host_session_id: &'a str,
    title: &'a str,
    what_happened: &'a str,
    actions: &'a [ActionInput],
    context_refs: &'a [ContextRef],
    source_hint: Option<&'a str>,
}

#[derive(Serialize)]
struct ImmutableRequest<'a> {
    host_id: &'a str,
    host_session_id: &'a str,
    title: &'a str,
    what_happened: &'a str,
    actions: &'a [ActionInput],
    context_refs: &'a [ContextRef],
    source_hint: Option<&'a str>,
    allow_finish: bool,
    final_summary: Option<&'a str>,
}

#[derive(Serialize)]
struct ImmutableRequestAttachment<'a> {
    file_name: &'a str,
    media_type: &'a str,
    byte_size: usize,
    sha256: &'a str,
}

#[derive(Serialize)]
struct ImmutableRequestWithAttachments<'a> {
    host_id: &'a str,
    host_session_id: &'a str,
    title: &'a str,
    what_happened: &'a str,
    actions: &'a [ActionInput],
    context_refs: &'a [ContextRef],
    attachments: Vec<ImmutableRequestAttachment<'a>>,
    source_hint: Option<&'a str>,
    allow_finish: bool,
    final_summary: Option<&'a str>,
}

fn immutable_input_hash(request: &NewFeedbackRequest) -> Result<String, RepositoryError> {
    let bytes = if !request.attachments.is_empty() {
        serde_json::to_vec(&ImmutableRequestWithAttachments {
            host_id: &request.host_id,
            host_session_id: &request.host_session_id,
            title: &request.title,
            what_happened: &request.what_happened,
            actions: &request.actions,
            context_refs: &request.context_refs,
            attachments: request
                .attachments
                .iter()
                .map(|attachment| ImmutableRequestAttachment {
                    file_name: &attachment.file_name,
                    media_type: &attachment.media_type,
                    byte_size: attachment.contents.len(),
                    sha256: &attachment.sha256,
                })
                .collect(),
            source_hint: request.source_hint.as_deref(),
            allow_finish: request.allow_finish,
            final_summary: request.final_summary.as_deref(),
        })
    } else if request.allow_finish || request.final_summary.is_some() {
        serde_json::to_vec(&ImmutableRequest {
            host_id: &request.host_id,
            host_session_id: &request.host_session_id,
            title: &request.title,
            what_happened: &request.what_happened,
            actions: &request.actions,
            context_refs: &request.context_refs,
            source_hint: request.source_hint.as_deref(),
            allow_finish: request.allow_finish,
            final_summary: request.final_summary.as_deref(),
        })
    } else {
        // Preserve the original persisted hash for pre-final-approval requests.
        serde_json::to_vec(&LegacyImmutableRequest {
            host_id: &request.host_id,
            host_session_id: &request.host_session_id,
            title: &request.title,
            what_happened: &request.what_happened,
            actions: &request.actions,
            context_refs: &request.context_refs,
            source_hint: request.source_hint.as_deref(),
        })
    }
    .map_err(|_| RepositoryError::Storage)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

impl SqliteFeedbackStore {
    async fn stage_request_attachments(
        &self,
        request: &NewFeedbackRequest,
    ) -> Result<Vec<StagedRequestAttachment>, RepositoryError> {
        if request.attachments.is_empty() {
            return Ok(Vec::new());
        }
        let directory = self
            .library_root()
            .join("drafts")
            .join(&request.request_id)
            .join("request-attachments");
        tokio::fs::create_dir_all(&directory)
            .await
            .map_err(storage_error)?;
        let mut staged: Vec<StagedRequestAttachment> =
            Vec::with_capacity(request.attachments.len());
        for attachment in &request.attachments {
            let path = directory.join(format!(
                "{}-{}",
                attachment.attachment_id,
                portable_file_name(&attachment.file_name)
            ));
            let result = async {
                let mut file = tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .await
                    .map_err(storage_error)?;
                file.write_all(&attachment.contents)
                    .await
                    .map_err(storage_error)?;
                file.flush().await.map_err(storage_error)?;
                file.sync_all().await.map_err(storage_error)
            }
            .await;
            if let Err(error) = result {
                for item in &staged {
                    let _ = tokio::fs::remove_file(&item.draft_path).await;
                }
                let _ = tokio::fs::remove_file(&path).await;
                return Err(error);
            }
            staged.push(StagedRequestAttachment {
                attachment_id: attachment.attachment_id.clone(),
                draft_path: path,
            });
        }
        Ok(staged)
    }

    async fn cleanup_staged_request_attachments(&self, staged: &[StagedRequestAttachment]) {
        for attachment in staged {
            let _ = tokio::fs::remove_file(&attachment.draft_path).await;
        }
    }

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
            let input_hash = immutable_input_hash(&request)?;
            let stored_hash: String = existing.try_get("input_hash").map_err(storage_error)?;
            return if stored_hash == input_hash {
                stored_request_from_row(&existing)
            } else {
                Err(RepositoryError::RequestConflict)
            };
        }

        let input_hash = immutable_input_hash(&request)?;

        sqlx::query(
            "INSERT INTO host_sessions \
             (id, host_id, host_session_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?4) \
             ON CONFLICT(host_id, host_session_id) DO UPDATE SET \
                 updated_at = excluded.updated_at, archived_at = NULL",
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
        let staged = self.stage_request_attachments(&request).await?;
        for (position, attachment) in request.attachments.iter().enumerate() {
            let staged_attachment = staged
                .iter()
                .find(|item| item.attachment_id == attachment.attachment_id)
                .ok_or(RepositoryError::CorruptData)?;
            let insert = sqlx::query(
                "INSERT INTO request_attachments \
                 (id, request_id, file_name, byte_size, media_type, sha256, position, contents, created_at, draft_path) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .bind(&attachment.attachment_id)
            .bind(&request.request_id)
            .bind(&attachment.file_name)
            .bind(attachment.contents.len() as i64)
            .bind(&attachment.media_type)
            .bind(&attachment.sha256)
            .bind(position as i64)
            .bind(Vec::<u8>::new())
            .bind(&request.created_at)
            .bind(path_string(&staged_attachment.draft_path)?)
            .execute(&mut *transaction)
            .await;
            if let Err(error) = insert {
                self.cleanup_staged_request_attachments(&staged).await;
                return Err(storage_error(error));
            }
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
        if let Err(error) = transaction.commit().await {
            self.cleanup_staged_request_attachments(&staged).await;
            return Err(storage_error(error));
        }
        Ok(stored)
    }

    pub(super) async fn externalize_legacy_request_attachments(
        &self,
    ) -> Result<(), RepositoryError> {
        let mut migrated = false;
        loop {
            let Some(row) = sqlx::query(
                "SELECT id, request_id, file_name, byte_size, sha256, contents \
                 FROM request_attachments \
                 WHERE (draft_path IS NULL OR draft_path = '') AND length(contents) > 0 \
                 ORDER BY request_id, position, id LIMIT 1",
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?
            else {
                break;
            };
            let attachment_id: String = row.try_get("id").map_err(storage_error)?;
            let request_id: String = row.try_get("request_id").map_err(storage_error)?;
            let file_name: String = row.try_get("file_name").map_err(storage_error)?;
            let byte_size: i64 = row.try_get("byte_size").map_err(storage_error)?;
            let sha256: String = row.try_get("sha256").map_err(storage_error)?;
            let contents: Vec<u8> = row.try_get("contents").map_err(storage_error)?;
            if contents.len() as i64 != byte_size
                || hex::encode(Sha256::digest(&contents)) != sha256
            {
                return Err(RepositoryError::CorruptData);
            }
            let directory = self
                .library_root()
                .join("drafts")
                .join(&request_id)
                .join("request-attachments");
            tokio::fs::create_dir_all(&directory)
                .await
                .map_err(storage_error)?;
            let path = directory.join(format!(
                "{}-{}",
                attachment_id,
                portable_file_name(&file_name)
            ));
            if tokio::fs::try_exists(&path).await.map_err(storage_error)? {
                let existing = tokio::fs::read(&path).await.map_err(storage_error)?;
                if existing != contents {
                    return Err(RepositoryError::CorruptData);
                }
            } else {
                let mut file = tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .await
                    .map_err(storage_error)?;
                file.write_all(&contents).await.map_err(storage_error)?;
                file.flush().await.map_err(storage_error)?;
                file.sync_all().await.map_err(storage_error)?;
            }
            let updated = sqlx::query(
                "UPDATE request_attachments SET draft_path = ?2, contents = x'' \
                 WHERE id = ?1 AND (draft_path IS NULL OR draft_path = '') \
                   AND sha256 = ?3",
            )
            .bind(&attachment_id)
            .bind(path_string(&path)?)
            .bind(&sha256)
            .execute(&self.pool)
            .await
            .map_err(storage_error)?;
            if updated.rows_affected() != 1 {
                return Err(RepositoryError::CorruptData);
            }
            migrated = true;
        }
        if migrated {
            sqlx::query("VACUUM")
                .execute(&self.pool)
                .await
                .map_err(storage_error)?;
            sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(&self.pool)
                .await
                .map_err(storage_error)?;
        }
        Ok(())
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
               AND hs.archived_at IS NULL \
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
               AND (?9 = (hs.archived_at IS NOT NULL)) \
               AND (?10 IS NULL \
                 OR r.title LIKE ?10 \
                 OR r.what_happened LIKE ?10 \
                 OR COALESCE(r.source_hint, '') LIKE ?10 \
                 OR r.id LIKE ?10 \
                 OR hs.host_id LIKE ?10 \
                 OR hs.host_session_id LIKE ?10) \
             ORDER BY r.updated_at DESC, r.id DESC \
             LIMIT ?11",
        )
        .bind(query.host_id.as_deref())
        .bind(query.host_session_id.as_deref())
        .bind(query.statuses.contains(&FeedbackStatus::Waiting))
        .bind(query.statuses.contains(&FeedbackStatus::InProgress))
        .bind(query.statuses.contains(&FeedbackStatus::Completed))
        .bind(query.statuses.contains(&FeedbackStatus::Cancelled))
        .bind(query.before_updated_at.as_deref())
        .bind(query.before_request_id.as_deref())
        .bind(query.archived)
        .bind(search_pattern(query.search.as_deref()))
        .bind(query.limit as i64 + 1)
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;
        rows.iter().map(summary_from_row).collect()
    }

    pub(super) async fn list_host_sessions_impl(
        &self,
        query: HostSessionQuery,
    ) -> Result<Vec<HostSessionSummary>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT hs.host_id, hs.host_session_id, \
                    COALESCE(NULLIF(hs.display_title, ''), (SELECT first_request.title \
                     FROM feedback_requests first_request \
                     WHERE first_request.host_session_record_id = hs.id \
                     ORDER BY first_request.created_at, first_request.id LIMIT 1)) AS title, \
                    (SELECT first_request.source_hint \
                     FROM feedback_requests first_request \
                     WHERE first_request.host_session_record_id = hs.id \
                     ORDER BY first_request.created_at, first_request.id LIMIT 1) AS source_hint, \
                    COUNT(r.id) AS request_count, \
                    SUM(CASE WHEN r.status IN ('waiting', 'in_progress') THEN 1 ELSE 0 END) AS pending_count, \
                    MAX(r.updated_at) AS updated_at, \
                    hs.pinned_at, hs.archived_at, hp.pinned_at AS host_pinned_at \
             FROM host_sessions hs \
             JOIN feedback_requests r ON r.host_session_record_id = hs.id \
             LEFT JOIN host_preferences hp ON hp.host_id = hs.host_id \
             WHERE (?1 = (hs.archived_at IS NOT NULL)) \
               AND (?2 IS NULL \
                 OR COALESCE(NULLIF(hs.display_title, ''), '') LIKE ?2 \
                 OR hs.host_id LIKE ?2 \
                 OR hs.host_session_id LIKE ?2 \
                 OR EXISTS( \
                    SELECT 1 FROM feedback_requests matching_request \
                    WHERE matching_request.host_session_record_id = hs.id \
                      AND (matching_request.title LIKE ?2 \
                        OR matching_request.what_happened LIKE ?2 \
                        OR COALESCE(matching_request.source_hint, '') LIKE ?2) \
                 )) \
             GROUP BY hs.id, hs.host_id, hs.host_session_id \
             ORDER BY hp.pinned_at IS NULL, hp.pinned_at DESC, hs.host_id, \
                      hs.pinned_at IS NULL, hs.pinned_at DESC, updated_at DESC, hs.host_session_id",
        )
        .bind(query.archived)
        .bind(search_pattern(query.search.as_deref()))
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;
        rows.iter().map(host_session_summary_from_row).collect()
    }
}

pub(super) fn search_pattern(value: Option<&str>) -> Option<String> {
    value.map(|value| format!("%{value}%"))
}

#[cfg(test)]
mod hash_tests {
    use super::*;

    fn request(allow_finish: bool, final_summary: Option<&str>) -> NewFeedbackRequest {
        NewFeedbackRequest {
            request_id: "request-id".to_owned(),
            host_session_record_id: "host-session-record-id".to_owned(),
            host_id: "generic".to_owned(),
            host_session_id: "session-1".to_owned(),
            title: "Review".to_owned(),
            what_happened: "Changed settings".to_owned(),
            actions: vec![ActionInput {
                id: "inspect".to_owned(),
                instruction: "Inspect settings".to_owned(),
            }],
            context_refs: vec![ContextRef {
                label: "diff".to_owned(),
                uri: "file:///tmp/change.diff".to_owned(),
            }],
            attachments: Vec::new(),
            source_hint: None,
            allow_finish,
            final_summary: final_summary.map(str::to_owned),
            created_at: "2026-08-03T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn immutable_hash_preserves_legacy_json_bytes() {
        assert_eq!(
            immutable_input_hash(&request(false, None)).expect("hash"),
            "53a9ef638f879a3d1790b8ef0d5ddf547405d569f5f29e6f33b32b93fe4f5b74"
        );
    }

    #[test]
    fn immutable_hash_covers_final_approval_fields() {
        assert_eq!(
            immutable_input_hash(&request(true, Some("Done"))).expect("hash"),
            "3abb4165d83f342b7a0f1f0f7218261c987a6abb19c4bf3b2f2c27b1c4ee212b"
        );
    }
}
