use std::{collections::HashSet, path::Path, sync::Arc, time::Duration};

use async_trait::async_trait;
use rambledesk_core::{
    ActionInput, AttachmentView, ContextRef, DraftView, FeedbackRepository, FeedbackRequestQuery,
    FeedbackRequestSummary, FeedbackResultView, FeedbackStatus, MAX_ATTACHMENT_COUNT,
    NewAttachment, NewFeedbackRequest, ProjectInput, PublishedFeedbackPackage, RepositoryError,
    StoredFeedbackRequest, StoredFeedbackWorkspace, SubmissionAttachment, SubmissionPlan,
};
use sha2::{Digest, Sha256};
use sqlx::{
    Row, SqlitePool,
    migrate::Migrator,
    sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow, SqliteSynchronous,
    },
};
use thiserror::Error;
use tokio::io::AsyncWriteExt;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Error)]
pub enum StorageOpenError {
    #[error("failed to create the RambleDesk data directory")]
    CreateDirectory(#[source] std::io::Error),
    #[error("failed to secure the RambleDesk data path")]
    SecurePath(#[source] std::io::Error),
    #[error("failed to open the RambleDesk SQLite database")]
    Connect(#[source] sqlx::Error),
    #[error("failed to migrate the RambleDesk SQLite database")]
    Migrate(#[source] sqlx::migrate::MigrateError),
    #[error("failed to inspect interrupted feedback publications")]
    Recovery(RepositoryError),
    #[error("no local application data directory is available")]
    DataDirectoryUnavailable,
}

#[derive(Clone)]
pub struct SqliteFeedbackStore {
    pool: SqlitePool,
    app_data_root: std::path::PathBuf,
    pub(crate) publish_lock: Arc<tokio::sync::Mutex<()>>,
}

impl SqliteFeedbackStore {
    pub async fn connect(path: &Path) -> Result<Self, StorageOpenError> {
        if let Some(parent) = path.parent() {
            let parent_existed = tokio::fs::try_exists(parent)
                .await
                .map_err(StorageOpenError::CreateDirectory)?;
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(StorageOpenError::CreateDirectory)?;
            secure_new_path(parent, parent_existed, 0o700).await?;
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
            .map_err(StorageOpenError::Connect)?;
        secure_path(path, 0o600).await?;
        MIGRATOR
            .run(&pool)
            .await
            .map_err(StorageOpenError::Migrate)?;
        let app_data_root = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let store = Self {
            pool,
            app_data_root,
            publish_lock: Arc::new(tokio::sync::Mutex::new(())),
        };
        store
            .recover_pending_submissions()
            .await
            .map_err(StorageOpenError::Recovery)?;
        Ok(store)
    }

    pub fn into_application(self) -> rambledesk_core::FeedbackApplication {
        let store = Arc::new(self);
        rambledesk_core::FeedbackApplication::new(store.clone(), store)
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    async fn recover_pending_submissions(&self) -> Result<(), RepositoryError> {
        let pending: Vec<(String, i64)> = sqlx::query_as(
            "SELECT request_id, source_revision FROM submission_plans \
             WHERE state = 'preparing' ORDER BY submitted_at, request_id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;
        for (request_id, source_revision) in pending {
            let recovery = async {
                let plan = self
                    .plan_submission(&request_id, source_revision as u64, "", "")
                    .await?;
                let published =
                    rambledesk_core::FeedbackPackagePublisher::publish(self, &plan).await?;
                self.complete_submission(&plan, &published).await
            }
            .await;
            if let Err(error) = recovery {
                sqlx::query(
                    "UPDATE submission_plans SET last_error_code = ?2, \
                         last_error_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                     WHERE request_id = ?1 AND state = 'preparing'",
                )
                .bind(&request_id)
                .bind(repository_error_code(error))
                .execute(&self.pool)
                .await
                .map_err(storage_error)?;
            }
        }
        Ok(())
    }
}

pub fn default_database_path() -> Result<std::path::PathBuf, StorageOpenError> {
    dirs::data_local_dir()
        .map(|root| root.join("RambleDesk").join("rambledesk.sqlite3"))
        .ok_or(StorageOpenError::DataDirectoryUnavailable)
}

#[async_trait]
impl FeedbackRepository for SqliteFeedbackStore {
    async fn create_or_get_request(
        &self,
        request: NewFeedbackRequest,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        if let Some(existing) = load_request_row(&mut transaction, &request.request_id).await? {
            if !project_input_matches_existing(&request.project, &existing).await? {
                return Err(RepositoryError::RequestConflict);
            }
            let project_id: String = existing.try_get("project_id").map_err(storage_error)?;
            let input_hash = request.immutable_input_hash(&project_id);
            let stored_hash: String = existing.try_get("input_hash").map_err(storage_error)?;
            return if stored_hash == input_hash {
                stored_request_from_row(&existing)
            } else {
                Err(RepositoryError::RequestConflict)
            };
        }

        let project_id = resolve_project_in_transaction(
            &mut transaction,
            &request.project,
            &request.candidate_project_id,
            &request.created_at,
        )
        .await?;
        let input_hash = request.immutable_input_hash(&project_id);

        sqlx::query(
            "INSERT INTO agent_sessions \
             (id, project_id, agent, external_session_id, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(project_id, agent, external_session_id) DO NOTHING",
        )
        .bind(&request.session_record_id)
        .bind(&project_id)
        .bind(&request.agent)
        .bind(&request.external_session_id)
        .bind(&request.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        let session_id: String = sqlx::query_scalar(
            "SELECT id FROM agent_sessions \
             WHERE project_id = ?1 AND agent = ?2 AND external_session_id = ?3",
        )
        .bind(&project_id)
        .bind(&request.agent)
        .bind(&request.external_session_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;

        let inserted = sqlx::query(
            "INSERT INTO feedback_requests \
             (id, session_id, what_happened, status, input_hash, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'waiting', ?4, ?5, ?5) \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(&request.request_id)
        .bind(session_id)
        .bind(&request.what_happened)
        .bind(&input_hash)
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
            project_id,
            status: FeedbackStatus::Waiting,
            created_at: request.created_at.clone(),
            updated_at: request.created_at,
            feedback: None,
        };
        transaction.commit().await.map_err(storage_error)?;
        Ok(stored)
    }

    async fn get_request(
        &self,
        request_id: &str,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        let row = sqlx::query(
            "SELECT r.id, s.project_id, r.status, r.created_at, r.updated_at, r.input_hash, \
                    fr.package_uri, fr.directory_path, fr.markdown_path, fr.manifest_path \
             FROM feedback_requests r \
             JOIN agent_sessions s ON s.id = r.session_id \
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

    async fn cancel_request(
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
             SET status = 'cancelled', cancelled_at = ?2, cancel_reason = ?3, \
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

    async fn list_open_requests(&self) -> Result<Vec<FeedbackRequestSummary>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT r.id, s.project_id, p.name AS project_name, s.agent, \
                    s.external_session_id, r.what_happened, r.status, \
                    r.revision, r.created_at, r.updated_at \
             FROM feedback_requests r \
             JOIN agent_sessions s ON s.id = r.session_id \
             JOIN projects p ON p.id = s.project_id \
             WHERE r.status IN ('waiting', 'in_progress') \
             ORDER BY r.updated_at DESC, r.id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;
        rows.iter().map(summary_from_row).collect()
    }

    async fn list_requests(
        &self,
        query: FeedbackRequestQuery,
    ) -> Result<Vec<FeedbackRequestSummary>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT r.id, s.project_id, p.name AS project_name, s.agent, \
                    s.external_session_id, r.what_happened, r.status, \
                    r.revision, r.created_at, r.updated_at \
             FROM feedback_requests r \
             JOIN agent_sessions s ON s.id = r.session_id \
             JOIN projects p ON p.id = s.project_id \
             WHERE (?1 IS NULL OR s.project_id = ?1) \
               AND (?2 IS NULL OR s.agent = ?2) \
               AND (?3 IS NULL OR s.external_session_id = ?3) \
               AND ((?4 AND r.status = 'waiting') \
                 OR (?5 AND r.status = 'in_progress') \
                 OR (?6 AND r.status = 'completed') \
                 OR (?7 AND r.status = 'cancelled')) \
               AND (?8 IS NULL OR r.updated_at < ?8 \
                 OR (r.updated_at = ?8 AND r.id < ?9)) \
             ORDER BY r.updated_at DESC, r.id DESC \
             LIMIT ?10",
        )
        .bind(query.project_id.as_deref())
        .bind(query.agent.as_deref())
        .bind(query.session_id.as_deref())
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

    async fn get_workspace(
        &self,
        request_id: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError> {
        load_workspace_from_pool(&self.pool, request_id).await
    }

    async fn save_draft(
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

    async fn add_attachment(
        &self,
        request_id: &str,
        attachment: NewAttachment,
        expected_revision: u64,
        now: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError> {
        let directory = self
            .app_data_root
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

    async fn remove_attachment(
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

    async fn reorder_attachments(
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

    async fn read_attachment(
        &self,
        request_id: &str,
        attachment_id: &str,
    ) -> Result<Vec<u8>, RepositoryError> {
        let draft_path: String = sqlx::query_scalar(
            "SELECT draft_path FROM attachments WHERE request_id = ?1 AND id = ?2",
        )
        .bind(request_id)
        .bind(attachment_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?
        .ok_or(RepositoryError::AttachmentNotFound)?;
        tokio::fs::read(draft_path).await.map_err(storage_error)
    }

    async fn plan_submission(
        &self,
        request_id: &str,
        expected_revision: u64,
        publication_id: &str,
        now: &str,
    ) -> Result<SubmissionPlan, RepositoryError> {
        let preflight = sqlx::query(
            "SELECT r.status, p.root_path_canonical, \
                    EXISTS(SELECT 1 FROM submission_plans sp WHERE sp.request_id = r.id) AS planned \
             FROM feedback_requests r \
             JOIN agent_sessions s ON s.id = r.session_id \
             JOIN projects p ON p.id = s.project_id \
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
            let project_root: String = preflight
                .try_get("root_path_canonical")
                .map_err(storage_error)?;
            Some(
                prepare_publication_paths(
                    request_id,
                    publication_id,
                    now,
                    Path::new(&project_root),
                    &self.app_data_root,
                )
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
            if stored_hash != body_sha256 {
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
             (request_id, publication_id, source_revision, body_sha256, submitted_at, \
              package_uri, directory_path, temp_directory_path, markdown_path, manifest_path) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(request_id)
        .bind(publication_id)
        .bind(aggregate_revision)
        .bind(&body_sha256)
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
            project_id: row.try_get("project_id").map_err(storage_error)?,
            agent: row.try_get("agent").map_err(storage_error)?,
            session_id: row.try_get("external_session_id").map_err(storage_error)?,
            what_happened: row.try_get("what_happened").map_err(storage_error)?,
            actions,
            attachments,
            body_markdown,
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

    async fn complete_submission(
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
                 status = 'completed', completed_at = ?2, updated_at = ?2, revision = revision + 1 \
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
        Ok(stored)
    }
}

fn summary_from_row(row: &SqliteRow) -> Result<FeedbackRequestSummary, RepositoryError> {
    let status: String = row.try_get("status").map_err(storage_error)?;
    Ok(FeedbackRequestSummary {
        request_id: row.try_get("id").map_err(storage_error)?,
        project_id: row.try_get("project_id").map_err(storage_error)?,
        project_name: row.try_get("project_name").map_err(storage_error)?,
        agent: row.try_get("agent").map_err(storage_error)?,
        session_id: row.try_get("external_session_id").map_err(storage_error)?,
        what_happened: row.try_get("what_happened").map_err(storage_error)?,
        status: FeedbackStatus::try_from(status.as_str())?,
        revision: row.try_get::<i64, _>("revision").map_err(storage_error)? as u64,
        created_at: row.try_get("created_at").map_err(storage_error)?,
        updated_at: row.try_get("updated_at").map_err(storage_error)?,
    })
}

async fn ensure_attachment_mutable(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
    expected_revision: u64,
) -> Result<i64, RepositoryError> {
    let row = sqlx::query(
        "SELECT status, revision, \
                EXISTS(SELECT 1 FROM submission_plans WHERE request_id = ?1) AS planned \
         FROM feedback_requests WHERE id = ?1",
    )
    .bind(request_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(storage_error)?
    .ok_or(RepositoryError::RequestNotFound)?;
    let status: String = row.try_get("status").map_err(storage_error)?;
    if matches!(
        FeedbackStatus::try_from(status.as_str())?,
        FeedbackStatus::Completed | FeedbackStatus::Cancelled
    ) {
        return Err(RepositoryError::RequestTerminal);
    }
    let planned: bool = row.try_get("planned").map_err(storage_error)?;
    let current_revision: i64 = row.try_get("revision").map_err(storage_error)?;
    if planned || current_revision != expected_revision as i64 {
        return Err(RepositoryError::DraftConflict);
    }
    Ok(current_revision)
}

async fn advance_attachment_revision(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
    current_revision: i64,
    now: &str,
) -> Result<(), RepositoryError> {
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
    .execute(&mut **transaction)
    .await
    .map_err(storage_error)?;
    if updated.rows_affected() != 1 {
        return Err(RepositoryError::DraftConflict);
    }
    sqlx::query(
        "INSERT INTO drafts (request_id, body_markdown, revision, updated_at) \
         VALUES (?1, '', ?2, ?3) \
         ON CONFLICT(request_id) DO UPDATE SET \
             revision = excluded.revision, updated_at = excluded.updated_at",
    )
    .bind(request_id)
    .bind(next_revision)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(storage_error)?;
    Ok(())
}

async fn load_workspace_from_pool(
    pool: &SqlitePool,
    request_id: &str,
) -> Result<StoredFeedbackWorkspace, RepositoryError> {
    let row = sqlx::query(
        "SELECT r.id, s.project_id, p.name AS project_name, s.agent, \
                s.external_session_id, r.what_happened, r.status, \
                r.revision, r.created_at, r.updated_at, \
                fr.package_uri, fr.directory_path, fr.markdown_path, fr.manifest_path \
         FROM feedback_requests r \
         JOIN agent_sessions s ON s.id = r.session_id \
         JOIN projects p ON p.id = s.project_id \
         LEFT JOIN feedback_results fr ON fr.request_id = r.id \
         WHERE r.id = ?1",
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await
    .map_err(storage_error)?
    .ok_or(RepositoryError::RequestNotFound)?;
    let action_rows = sqlx::query(
        "SELECT action_id, instruction FROM request_actions \
         WHERE request_id = ?1 ORDER BY position",
    )
    .bind(request_id)
    .fetch_all(pool)
    .await
    .map_err(storage_error)?;
    let actions = action_rows
        .iter()
        .map(|row| {
            Ok(ActionInput {
                id: row.try_get("action_id").map_err(storage_error)?,
                instruction: row.try_get("instruction").map_err(storage_error)?,
            })
        })
        .collect::<Result<Vec<_>, RepositoryError>>()?;
    let context_rows = sqlx::query(
        "SELECT label, uri FROM request_context_refs \
         WHERE request_id = ?1 ORDER BY position",
    )
    .bind(request_id)
    .fetch_all(pool)
    .await
    .map_err(storage_error)?;
    let context_refs = context_rows
        .iter()
        .map(|row| {
            Ok(ContextRef {
                label: row.try_get("label").map_err(storage_error)?,
                uri: row.try_get("uri").map_err(storage_error)?,
            })
        })
        .collect::<Result<Vec<_>, RepositoryError>>()?;
    let draft_row =
        sqlx::query("SELECT body_markdown, revision, updated_at FROM drafts WHERE request_id = ?1")
            .bind(request_id)
            .fetch_optional(pool)
            .await
            .map_err(storage_error)?;
    let draft = match draft_row {
        Some(row) => DraftView {
            body_markdown: row.try_get("body_markdown").map_err(storage_error)?,
            saved_revision: row.try_get::<i64, _>("revision").map_err(storage_error)? as u64,
            updated_at: Some(row.try_get("updated_at").map_err(storage_error)?),
        },
        None => DraftView {
            body_markdown: String::new(),
            saved_revision: 0,
            updated_at: None,
        },
    };
    let attachment_rows = sqlx::query(
        "SELECT id, file_name, media_type, byte_size, sha256, position \
         FROM attachments WHERE request_id = ?1 ORDER BY position, id",
    )
    .bind(request_id)
    .fetch_all(pool)
    .await
    .map_err(storage_error)?;
    let attachments = attachment_rows
        .iter()
        .map(attachment_view_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StoredFeedbackWorkspace {
        request: summary_from_row(&row)?,
        actions,
        context_refs,
        draft,
        attachments,
        feedback: feedback_result_from_row(&row)?,
    })
}

fn attachment_view_from_row(row: &SqliteRow) -> Result<AttachmentView, RepositoryError> {
    Ok(AttachmentView {
        attachment_id: row.try_get("id").map_err(storage_error)?,
        file_name: row.try_get("file_name").map_err(storage_error)?,
        media_type: row.try_get("media_type").map_err(storage_error)?,
        byte_size: row.try_get::<i64, _>("byte_size").map_err(storage_error)? as u64,
        sha256: row.try_get("sha256").map_err(storage_error)?,
        position: row.try_get::<i64, _>("position").map_err(storage_error)? as u32,
    })
}

async fn load_submission_row(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
) -> Result<Option<SqliteRow>, RepositoryError> {
    sqlx::query(
        "SELECT r.id, r.status, r.revision AS request_revision, r.what_happened, \
                s.project_id, s.agent, \
                s.external_session_id, p.root_path_canonical, \
                d.body_markdown, d.revision AS draft_revision, \
                sp.publication_id, sp.source_revision, sp.body_sha256, sp.submitted_at, \
                sp.package_uri, sp.directory_path, sp.temp_directory_path, \
                sp.markdown_path, sp.manifest_path \
         FROM feedback_requests r \
         JOIN agent_sessions s ON s.id = r.session_id \
         JOIN projects p ON p.id = s.project_id \
         LEFT JOIN drafts d ON d.request_id = r.id \
         LEFT JOIN submission_plans sp ON sp.request_id = r.id \
         WHERE r.id = ?1",
    )
    .bind(request_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(storage_error)
}

async fn load_actions(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
) -> Result<Vec<ActionInput>, RepositoryError> {
    let rows = sqlx::query(
        "SELECT action_id, instruction FROM request_actions \
         WHERE request_id = ?1 ORDER BY position",
    )
    .bind(request_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(storage_error)?;
    rows.iter()
        .map(|row| {
            Ok(ActionInput {
                id: row.try_get("action_id").map_err(storage_error)?,
                instruction: row.try_get("instruction").map_err(storage_error)?,
            })
        })
        .collect()
}

async fn load_submission_attachments(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
) -> Result<Vec<SubmissionAttachment>, RepositoryError> {
    let rows = sqlx::query(
        "SELECT id, draft_path, file_name, media_type, byte_size, sha256 \
         FROM attachments WHERE request_id = ?1 ORDER BY position, id",
    )
    .bind(request_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(storage_error)?;
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let file_name: String = row.try_get("file_name").map_err(storage_error)?;
            Ok(SubmissionAttachment {
                attachment_id: row.try_get("id").map_err(storage_error)?,
                relative_path: format!(
                    "attachments/{:03}-{}",
                    index + 1,
                    portable_file_name(&file_name)
                ),
                file_name,
                media_type: row.try_get("media_type").map_err(storage_error)?,
                byte_size: row.try_get::<i64, _>("byte_size").map_err(storage_error)? as u64,
                sha256: row.try_get("sha256").map_err(storage_error)?,
                draft_path: row.try_get("draft_path").map_err(storage_error)?,
            })
        })
        .collect()
}

fn submission_plan_from_row(
    row: &SqliteRow,
    actions: Vec<ActionInput>,
    attachments: Vec<SubmissionAttachment>,
    body_markdown: String,
) -> Result<SubmissionPlan, RepositoryError> {
    Ok(SubmissionPlan {
        request_id: row.try_get("id").map_err(storage_error)?,
        project_id: row.try_get("project_id").map_err(storage_error)?,
        agent: row.try_get("agent").map_err(storage_error)?,
        session_id: row.try_get("external_session_id").map_err(storage_error)?,
        what_happened: row.try_get("what_happened").map_err(storage_error)?,
        actions,
        attachments,
        body_markdown,
        source_revision: row
            .try_get::<i64, _>("source_revision")
            .map_err(storage_error)? as u64,
        publication_id: row.try_get("publication_id").map_err(storage_error)?,
        body_sha256: row.try_get("body_sha256").map_err(storage_error)?,
        submitted_at: row.try_get("submitted_at").map_err(storage_error)?,
        package_uri: row.try_get("package_uri").map_err(storage_error)?,
        directory_path: row.try_get("directory_path").map_err(storage_error)?,
        temp_directory_path: row.try_get("temp_directory_path").map_err(storage_error)?,
        markdown_path: row.try_get("markdown_path").map_err(storage_error)?,
        manifest_path: row.try_get("manifest_path").map_err(storage_error)?,
    })
}

struct PreparedPublicationPaths {
    package_uri: String,
    directory_path: String,
    temp_directory_path: String,
    markdown_path: String,
    manifest_path: String,
}

async fn prepare_publication_paths(
    request_id: &str,
    publication_id: &str,
    now: &str,
    project_root: &Path,
    app_data_root: &Path,
) -> Result<PreparedPublicationPaths, RepositoryError> {
    let feedback_root = select_feedback_root(project_root, app_data_root, publication_id).await?;
    let directory_name = format!("{}-{request_id}", compact_timestamp(now));
    let directory_path = feedback_root.join(directory_name);
    let temp_directory_path = feedback_root.join(format!(".{request_id}.tmp-{publication_id}"));
    let markdown_path = directory_path.join("feedback.md");
    let manifest_path = directory_path.join("manifest.json");
    Ok(PreparedPublicationPaths {
        package_uri: format!("rambledesk://feedback/{request_id}"),
        directory_path: path_string(&directory_path)?,
        temp_directory_path: path_string(&temp_directory_path)?,
        markdown_path: path_string(&markdown_path)?,
        manifest_path: path_string(&manifest_path)?,
    })
}

async fn select_feedback_root(
    project_root: &Path,
    app_data_root: &Path,
    publication_id: &str,
) -> Result<std::path::PathBuf, RepositoryError> {
    if let Ok(project_feedback) = prepare_project_feedback_root(project_root, publication_id).await
    {
        return Ok(project_feedback);
    }
    let fallback = app_data_root.join("feedback");
    tokio::fs::create_dir_all(&fallback)
        .await
        .map_err(storage_error)?;
    assert_not_symlink(&fallback).await?;
    let canonical_fallback = tokio::fs::canonicalize(&fallback)
        .await
        .map_err(package_error)?;
    verify_writable(&canonical_fallback, publication_id).await?;
    Ok(canonical_fallback)
}

async fn prepare_project_feedback_root(
    project_root: &Path,
    publication_id: &str,
) -> Result<std::path::PathBuf, RepositoryError> {
    let canonical_root = tokio::fs::canonicalize(project_root)
        .await
        .map_err(package_error)?;
    if canonical_root != project_root {
        return Err(RepositoryError::PackagePublish);
    }
    assert_not_symlink(&canonical_root).await?;

    let rambledesk_root = canonical_root.join(".rambledesk");
    create_safe_directory(&rambledesk_root).await?;
    let feedback_root = rambledesk_root.join("feedback");
    create_safe_directory(&feedback_root).await?;

    let canonical_feedback = tokio::fs::canonicalize(&feedback_root)
        .await
        .map_err(package_error)?;
    if !canonical_feedback.starts_with(&canonical_root) {
        return Err(RepositoryError::PackagePublish);
    }
    verify_writable(&canonical_feedback, publication_id).await?;
    Ok(canonical_feedback)
}

async fn create_safe_directory(path: &Path) -> Result<(), RepositoryError> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(RepositoryError::PackagePublish);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tokio::fs::create_dir(path).await.map_err(package_error)?;
        }
        Err(error) => return Err(package_error(error)),
    }
    Ok(())
}

async fn assert_not_symlink(path: &Path) -> Result<(), RepositoryError> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(package_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(RepositoryError::PackagePublish);
    }
    Ok(())
}

async fn verify_writable(directory: &Path, publication_id: &str) -> Result<(), RepositoryError> {
    let probe = directory.join(format!(".write-probe-{publication_id}"));
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    let file = options.open(&probe).await.map_err(package_error)?;
    file.sync_all().await.map_err(package_error)?;
    drop(file);
    tokio::fs::remove_file(&probe).await.map_err(package_error)
}

fn compact_timestamp(timestamp: &str) -> String {
    timestamp
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

fn portable_file_name(file_name: &str) -> String {
    let mut value = file_name
        .chars()
        .map(|character| {
            if character.is_control() || "<>:\"/\\|?*".contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    while value.ends_with([' ', '.']) {
        value.pop();
    }
    if value.is_empty() {
        "attachment".to_owned()
    } else {
        value
    }
}

fn path_string(path: &Path) -> Result<String, RepositoryError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or(RepositoryError::PackagePublish)
}

fn package_error<T>(_error: T) -> RepositoryError {
    RepositoryError::PackagePublish
}

async fn resolve_project_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    input: &ProjectInput,
    candidate_project_id: &str,
    now: &str,
) -> Result<String, RepositoryError> {
    let canonical = match input.root_path.as_deref() {
        Some(root_path) => Some(canonical_project_path(root_path).await?),
        None => None,
    };

    if let Some(project_id) = input.project_id.as_deref() {
        let existing = sqlx::query("SELECT id, root_path_canonical FROM projects WHERE id = ?1")
            .bind(project_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(storage_error)?;
        if let Some(row) = existing {
            if let Some(canonical) = canonical.as_deref() {
                let stored: String = row.try_get("root_path_canonical").map_err(storage_error)?;
                if stored != canonical {
                    return Err(RepositoryError::ProjectConflict);
                }
            }
            return Ok(project_id.to_owned());
        }

        let Some(canonical) = canonical.as_deref() else {
            return Err(RepositoryError::ProjectNotFound);
        };
        if canonical_project_id(transaction, canonical)
            .await?
            .is_some()
        {
            return Err(RepositoryError::ProjectConflict);
        }
        let root_path = input
            .root_path
            .as_deref()
            .ok_or(RepositoryError::ProjectPathUnavailable)?;
        insert_project(
            transaction,
            project_id,
            &input.name,
            root_path,
            canonical,
            now,
        )
        .await?;
        return Ok(project_id.to_owned());
    }

    let canonical = canonical
        .as_deref()
        .ok_or(RepositoryError::ProjectPathUnavailable)?;
    if let Some(project_id) = canonical_project_id(transaction, canonical).await? {
        return Ok(project_id);
    }
    let root_path = input
        .root_path
        .as_deref()
        .ok_or(RepositoryError::ProjectPathUnavailable)?;
    insert_project(
        transaction,
        candidate_project_id,
        &input.name,
        root_path,
        canonical,
        now,
    )
    .await?;
    Ok(candidate_project_id.to_owned())
}

async fn project_input_matches_existing(
    input: &ProjectInput,
    row: &SqliteRow,
) -> Result<bool, RepositoryError> {
    let stored_project_id: String = row.try_get("project_id").map_err(storage_error)?;
    if input
        .project_id
        .as_deref()
        .is_some_and(|project_id| project_id != stored_project_id)
    {
        return Ok(false);
    }

    let Some(root_path) = input.root_path.as_deref() else {
        return Ok(true);
    };
    let stored_root_path: String = row.try_get("root_path").map_err(storage_error)?;
    let stored_canonical: String = row.try_get("root_path_canonical").map_err(storage_error)?;
    match canonical_project_path(root_path).await {
        Ok(canonical) => Ok(canonical == stored_canonical),
        Err(RepositoryError::ProjectPathUnavailable) => Ok(root_path == stored_root_path),
        Err(error) => Err(error),
    }
}

async fn canonical_project_path(root_path: &str) -> Result<String, RepositoryError> {
    let canonical = tokio::fs::canonicalize(root_path)
        .await
        .map_err(|_| RepositoryError::ProjectPathUnavailable)?;
    let metadata = tokio::fs::metadata(&canonical)
        .await
        .map_err(|_| RepositoryError::ProjectPathUnavailable)?;
    if !metadata.is_dir() {
        return Err(RepositoryError::ProjectPathUnavailable);
    }
    canonical
        .to_str()
        .map(ToOwned::to_owned)
        .ok_or(RepositoryError::ProjectPathUnavailable)
}

async fn canonical_project_id(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    canonical: &str,
) -> Result<Option<String>, RepositoryError> {
    sqlx::query_scalar("SELECT id FROM projects WHERE root_path_canonical = ?1")
        .bind(canonical)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(storage_error)
}

async fn insert_project(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_id: &str,
    name: &str,
    root_path: &str,
    canonical: &str,
    now: &str,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO projects \
         (id, name, root_path, root_path_canonical, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
    )
    .bind(project_id)
    .bind(name)
    .bind(root_path)
    .bind(canonical)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(storage_error)?;
    Ok(())
}

async fn load_request_row(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
) -> Result<Option<SqliteRow>, RepositoryError> {
    sqlx::query(
        "SELECT r.id, s.project_id, p.root_path, p.root_path_canonical, \
                r.status, r.created_at, r.updated_at, r.input_hash, \
                fr.package_uri, fr.directory_path, fr.markdown_path, fr.manifest_path \
         FROM feedback_requests r \
         JOIN agent_sessions s ON s.id = r.session_id \
         JOIN projects p ON p.id = s.project_id \
         LEFT JOIN feedback_results fr ON fr.request_id = r.id \
         WHERE r.id = ?1",
    )
    .bind(request_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(storage_error)
}

fn stored_request_from_row(row: &SqliteRow) -> Result<StoredFeedbackRequest, RepositoryError> {
    let status = stored_status(row)?;
    let feedback = feedback_result_from_row(row)?;
    if status == FeedbackStatus::Completed && feedback.is_none() {
        return Err(RepositoryError::CorruptData);
    }
    Ok(StoredFeedbackRequest {
        request_id: row.try_get("id").map_err(storage_error)?,
        project_id: row.try_get("project_id").map_err(storage_error)?,
        status,
        created_at: row.try_get("created_at").map_err(storage_error)?,
        updated_at: row.try_get("updated_at").map_err(storage_error)?,
        feedback,
    })
}

fn feedback_result_from_row(
    row: &SqliteRow,
) -> Result<Option<FeedbackResultView>, RepositoryError> {
    let package_uri: Option<String> = row.try_get("package_uri").map_err(storage_error)?;
    let feedback = match package_uri {
        Some(package_uri) => Some(FeedbackResultView {
            package_uri,
            directory_path: row
                .try_get::<Option<String>, _>("directory_path")
                .map_err(storage_error)?
                .ok_or(RepositoryError::CorruptData)?,
            markdown_path: row
                .try_get::<Option<String>, _>("markdown_path")
                .map_err(storage_error)?
                .ok_or(RepositoryError::CorruptData)?,
            manifest_path: row
                .try_get::<Option<String>, _>("manifest_path")
                .map_err(storage_error)?
                .ok_or(RepositoryError::CorruptData)?,
        }),
        None => None,
    };
    Ok(feedback)
}

fn stored_status(row: &SqliteRow) -> Result<FeedbackStatus, RepositoryError> {
    let status: String = row.try_get("status").map_err(storage_error)?;
    FeedbackStatus::try_from(status.as_str())
}

fn storage_error<T>(_error: T) -> RepositoryError {
    RepositoryError::Storage
}

fn repository_error_code(error: RepositoryError) -> &'static str {
    match error {
        RepositoryError::PackagePublish => "PACKAGE_PUBLISH_FAILURE",
        RepositoryError::DraftConflict => "DRAFT_CONFLICT",
        RepositoryError::RequestNotFound => "REQUEST_NOT_FOUND",
        RepositoryError::RequestTerminal | RepositoryError::RequestAlreadyCompleted => {
            "REQUEST_TERMINAL"
        }
        RepositoryError::CorruptData | RepositoryError::Storage => "STORAGE_FAILURE",
        RepositoryError::AttachmentNotFound | RepositoryError::AttachmentLimit => {
            "RECOVERY_FAILURE"
        }
        RepositoryError::ProjectNotFound
        | RepositoryError::ProjectPathUnavailable
        | RepositoryError::ProjectConflict
        | RepositoryError::RequestConflict
        | RepositoryError::DraftEmpty => "RECOVERY_FAILURE",
    }
}

#[cfg(unix)]
async fn secure_new_path(path: &Path, existed: bool, mode: u32) -> Result<(), StorageOpenError> {
    if existed {
        return Ok(());
    }
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .await
        .map_err(StorageOpenError::SecurePath)
}

#[cfg(not(unix))]
async fn secure_new_path(_path: &Path, _existed: bool, _mode: u32) -> Result<(), StorageOpenError> {
    Ok(())
}

#[cfg(unix)]
async fn secure_path(path: &Path, mode: u32) -> Result<(), StorageOpenError> {
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .await
        .map_err(StorageOpenError::SecurePath)
}

#[cfg(not(unix))]
async fn secure_path(_path: &Path, _mode: u32) -> Result<(), StorageOpenError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rambledesk_core::{
        ActionInput, AddAttachmentInput, CancelFeedbackInput, ContextRef, FeedbackStatus,
        GetFeedbackInput, ListFeedbackRequestsInput, ProjectInput, RemoveAttachmentInput,
        ReorderAttachmentsInput, RequestFeedbackInput, SaveDraftInput, SubmitFeedbackInput,
    };
    use tempfile::TempDir;
    use uuid::Uuid;

    struct TestWorkspace {
        _temp: TempDir,
        database: std::path::PathBuf,
        project: std::path::PathBuf,
    }

    impl TestWorkspace {
        async fn new() -> Self {
            let temp = tempfile::tempdir().expect("temporary directory");
            let project = temp.path().join("project");
            tokio::fs::create_dir(&project)
                .await
                .expect("project directory");
            Self {
                database: temp.path().join("state").join("rambledesk.sqlite3"),
                project,
                _temp: temp,
            }
        }

        fn request(&self, request_id: String) -> RequestFeedbackInput {
            RequestFeedbackInput {
                request_id: Some(request_id),
                agent: "test-agent".to_owned(),
                session_id: "test-session".to_owned(),
                project: ProjectInput {
                    project_id: None,
                    name: "Test project".to_owned(),
                    root_path: Some(self.project.to_string_lossy().into_owned()),
                },
                what_happened: "Implemented the persistence kernel.".to_owned(),
                actions: vec![ActionInput {
                    id: "review".to_owned(),
                    instruction: "Review the implementation.".to_owned(),
                }],
                context_refs: vec![ContextRef {
                    label: "diff".to_owned(),
                    uri: "file:///tmp/change.diff".to_owned(),
                }],
            }
        }
    }

    #[tokio::test]
    async fn request_is_idempotent_conflict_safe_and_survives_restart() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        let input = workspace.request(request_id.clone());

        let created = application
            .request_feedback(input.clone())
            .await
            .expect("create request");
        let retried = application
            .request_feedback(input.clone())
            .await
            .expect("retry request");
        assert_eq!(created, retried);
        assert_eq!(created.status, FeedbackStatus::Waiting);

        tokio::fs::remove_dir(&workspace.project)
            .await
            .expect("remove project after request creation");
        let recovered_without_path = application
            .request_feedback(input.clone())
            .await
            .expect("retry request after project path disappears");
        assert_eq!(created, recovered_without_path);

        let mut conflicting = input;
        conflicting.context_refs[0].uri = "file:///tmp/other.diff".to_owned();
        let conflict = application
            .request_feedback(conflicting)
            .await
            .expect_err("changed immutable input must conflict");
        assert_eq!(conflict.code(), "REQUEST_CONFLICT");

        store.close().await;
        let reopened = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("reopen store");
        let recovered = reopened
            .clone()
            .into_application()
            .get_feedback(GetFeedbackInput { request_id })
            .await
            .expect("recover request");
        assert_eq!(created, recovered);
        reopened.close().await;
    }

    #[tokio::test]
    async fn conflicting_retry_does_not_leave_an_orphan_project() {
        let workspace = TestWorkspace::new().await;
        let other_project = workspace._temp.path().join("other-project");
        tokio::fs::create_dir(&other_project)
            .await
            .expect("other project directory");
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create request");

        let mut conflicting = workspace.request(request_id);
        conflicting.project.root_path = Some(other_project.to_string_lossy().into_owned());
        let conflict = application
            .request_feedback(conflicting)
            .await
            .expect_err("different project must conflict");
        assert_eq!(conflict.code(), "REQUEST_CONFLICT");

        let project_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(&store.pool)
            .await
            .expect("project count");
        assert_eq!(project_count, 1);
        store.close().await;
    }

    #[tokio::test]
    async fn explicit_project_ids_are_stored_and_compared_canonically() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let canonical_project_id = Uuid::now_v7().to_string();
        let mut input = workspace.request(request_id);
        input.project.project_id = Some(canonical_project_id.to_uppercase());
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();

        let created = application
            .request_feedback(input.clone())
            .await
            .expect("create request");
        assert_eq!(created.project_id, canonical_project_id);

        input.project.project_id = Some(created.project_id.clone());
        input.project.root_path = None;
        let retried = application
            .request_feedback(input)
            .await
            .expect("retry with canonical project id");
        assert_eq!(created, retried);
        store.close().await;
    }

    #[tokio::test]
    async fn repeated_cancel_preserves_the_first_reason_and_terminal_state() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create request");

        let first = application
            .cancel_feedback(CancelFeedbackInput {
                request_id: request_id.clone(),
                reason: "The agent no longer needs feedback.".to_owned(),
            })
            .await
            .expect("cancel request");
        let repeated = application
            .cancel_feedback(CancelFeedbackInput {
                request_id: request_id.clone(),
                reason: "This must not overwrite the original reason.".to_owned(),
            })
            .await
            .expect("repeat cancel");
        assert_eq!(first, repeated);
        assert_eq!(first.status, FeedbackStatus::Cancelled);

        let reason: String =
            sqlx::query_scalar("SELECT cancel_reason FROM feedback_requests WHERE id = ?1")
                .bind(&request_id)
                .fetch_one(&store.pool)
                .await
                .expect("stored cancel reason");
        assert_eq!(reason, "The agent no longer needs feedback.");

        let terminal_update =
            sqlx::query("UPDATE feedback_requests SET status = 'waiting' WHERE id = ?1")
                .bind(&request_id)
                .execute(&store.pool)
                .await;
        assert!(terminal_update.is_err(), "cancelled state must be terminal");
        store.close().await;
    }

    #[tokio::test]
    async fn concurrent_retries_converge_on_one_request() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        let left = application.request_feedback(workspace.request(request_id.clone()));
        let right = application.request_feedback(workspace.request(request_id.clone()));
        let (left, right) = tokio::join!(left, right);
        assert_eq!(left.expect("left retry"), right.expect("right retry"));

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feedback_requests WHERE id = ?1")
            .bind(request_id)
            .fetch_one(&store.pool)
            .await
            .expect("request count");
        assert_eq!(count, 1);
        store.close().await;
    }

    #[tokio::test]
    async fn draft_uses_aggregate_revision_and_idempotent_replay() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create request");

        let first = application
            .save_feedback_draft(SaveDraftInput {
                request_id: request_id.clone(),
                body_markdown: "The primary flow is clear.".to_owned(),
                expected_revision: 0,
            })
            .await
            .expect("save draft");
        assert_eq!(first.saved_revision, 1);
        let replay = application
            .save_feedback_draft(SaveDraftInput {
                request_id: request_id.clone(),
                body_markdown: first.body_markdown.clone(),
                expected_revision: 0,
            })
            .await
            .expect("replay lost response");
        assert_eq!(first, replay);

        let conflict = application
            .save_feedback_draft(SaveDraftInput {
                request_id: request_id.clone(),
                body_markdown: "A conflicting edit.".to_owned(),
                expected_revision: 0,
            })
            .await
            .expect_err("stale different body must conflict");
        assert_eq!(conflict.code(), "DRAFT_CONFLICT");

        let opened = application
            .get_feedback_workspace(request_id.clone())
            .await
            .expect("open workspace");
        assert_eq!(opened.request.revision, 1);
        assert_eq!(opened.request.status, FeedbackStatus::InProgress);
        assert_eq!(opened.draft, first);

        store.close().await;
        let reopened = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("reopen store");
        let recovered = reopened
            .clone()
            .into_application()
            .get_feedback_workspace(request_id)
            .await
            .expect("recover draft");
        assert_eq!(recovered.draft.saved_revision, 1);
        assert_eq!(recovered.draft.body_markdown, "The primary flow is clear.");
        reopened.close().await;
    }

    #[tokio::test]
    async fn concurrent_different_drafts_have_one_cas_winner() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create request");

        let left = application.save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: "left".to_owned(),
            expected_revision: 0,
        });
        let right = application.save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: "right".to_owned(),
            expected_revision: 0,
        });
        let (left, right) = tokio::join!(left, right);
        assert_ne!(left.is_ok(), right.is_ok());
        let loser = left.err().or_else(|| right.err()).expect("one loser");
        assert_eq!(loser.code(), "DRAFT_CONFLICT");
        let saved = application
            .get_feedback_workspace(request_id)
            .await
            .expect("winner persisted");
        assert_eq!(saved.request.revision, 1);
        assert!(matches!(
            saved.draft.body_markdown.as_str(),
            "left" | "right"
        ));
        store.close().await;
    }

    #[tokio::test]
    async fn submit_is_idempotent_and_publishes_one_immutable_package() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create request");
        let draft = application
            .save_feedback_draft(SaveDraftInput {
                request_id: request_id.clone(),
                body_markdown: "Ship it after tightening the empty state.".to_owned(),
                expected_revision: 0,
            })
            .await
            .expect("save draft");

        let submitted = application
            .submit_feedback(SubmitFeedbackInput {
                request_id: request_id.clone(),
                expected_revision: draft.saved_revision,
            })
            .await
            .expect("submit");
        let replay = application
            .submit_feedback(SubmitFeedbackInput {
                request_id: request_id.clone(),
                expected_revision: 0,
            })
            .await
            .expect("completed submit replay");
        assert_eq!(submitted, replay);
        assert_eq!(submitted.status, FeedbackStatus::Completed);
        let result = submitted.feedback.expect("published feedback");
        assert!(Path::new(&result.markdown_path).is_file());
        let manifest: serde_json::Value = serde_json::from_str(
            &tokio::fs::read_to_string(&result.manifest_path)
                .await
                .expect("manifest"),
        )
        .expect("valid manifest");
        assert_eq!(manifest["request_id"], request_id);
        assert_eq!(manifest["source_revision"], 1);
        assert_eq!(manifest["draft_revision"], 1);
        assert_eq!(manifest["feedback_markdown"], "feedback.md");
        assert!(manifest["feedback_sha256"].as_str().is_some());

        let directory_count =
            std::fs::read_dir(workspace.project.join(".rambledesk").join("feedback"))
                .expect("feedback root")
                .filter_map(Result::ok)
                .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
                .count();
        assert_eq!(directory_count, 1);
        store.close().await;
    }

    #[tokio::test]
    async fn restart_reconciles_package_published_before_database_completion() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create request");
        let draft = application
            .save_feedback_draft(SaveDraftInput {
                request_id: request_id.clone(),
                body_markdown: "Recovery must converge on this package.".to_owned(),
                expected_revision: 0,
            })
            .await
            .expect("save draft");
        let plan = store
            .plan_submission(
                &request_id,
                draft.saved_revision,
                &Uuid::now_v7().to_string(),
                "2026-07-29T14:00:00Z",
            )
            .await
            .expect("persist intent");
        rambledesk_core::FeedbackPackagePublisher::publish(&store, &plan)
            .await
            .expect("publish before simulated crash");
        store.close().await;

        let reopened = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("startup reconciliation");
        let completed = reopened
            .clone()
            .into_application()
            .get_feedback(GetFeedbackInput { request_id })
            .await
            .expect("completed after recovery");
        assert_eq!(completed.status, FeedbackStatus::Completed);
        assert_eq!(
            completed.feedback.expect("feedback result").directory_path,
            plan.directory_path
        );
        reopened.close().await;
    }

    #[tokio::test]
    async fn missing_project_root_uses_frozen_app_data_fallback() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create request");
        let draft = application
            .save_feedback_draft(SaveDraftInput {
                request_id: request_id.clone(),
                body_markdown: "The project directory disappeared.".to_owned(),
                expected_revision: 0,
            })
            .await
            .expect("save draft");
        tokio::fs::remove_dir(&workspace.project)
            .await
            .expect("remove project");
        let completed = application
            .submit_feedback(SubmitFeedbackInput {
                request_id,
                expected_revision: draft.saved_revision,
            })
            .await
            .expect("submit via fallback");
        let directory = completed.feedback.expect("feedback result").directory_path;
        let fallback_root =
            tokio::fs::canonicalize(workspace.database.parent().unwrap().join("feedback"))
                .await
                .expect("canonical fallback");
        assert!(Path::new(&directory).starts_with(fallback_root));
        store.close().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlinked_project_feedback_directory_uses_app_data_fallback() {
        use std::os::unix::fs::symlink;

        let workspace = TestWorkspace::new().await;
        let outside = workspace._temp.path().join("outside");
        tokio::fs::create_dir(&outside)
            .await
            .expect("outside directory");
        tokio::fs::create_dir(workspace.project.join(".rambledesk"))
            .await
            .expect("metadata directory");
        symlink(
            &outside,
            workspace.project.join(".rambledesk").join("feedback"),
        )
        .expect("feedback symlink");

        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create request");
        let draft = application
            .save_feedback_draft(SaveDraftInput {
                request_id: request_id.clone(),
                body_markdown: "A symlink must never escape the project.".to_owned(),
                expected_revision: 0,
            })
            .await
            .expect("save draft");
        let completed = application
            .submit_feedback(SubmitFeedbackInput {
                request_id,
                expected_revision: draft.saved_revision,
            })
            .await
            .expect("submit via fallback");
        let directory = completed.feedback.expect("feedback").directory_path;
        let fallback_root =
            tokio::fs::canonicalize(workspace.database.parent().unwrap().join("feedback"))
                .await
                .expect("canonical fallback");
        assert!(Path::new(&directory).starts_with(fallback_root));
        assert_eq!(
            std::fs::read_dir(&outside)
                .expect("outside remains readable")
                .count(),
            0
        );
        store.close().await;
    }

    #[tokio::test]
    async fn mismatched_existing_final_package_is_never_overwritten() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create request");
        let draft = application
            .save_feedback_draft(SaveDraftInput {
                request_id: request_id.clone(),
                body_markdown: "Do not overwrite an unexpected package.".to_owned(),
                expected_revision: 0,
            })
            .await
            .expect("save draft");
        let plan = store
            .plan_submission(
                &request_id,
                draft.saved_revision,
                &Uuid::now_v7().to_string(),
                "2026-07-29T15:00:00Z",
            )
            .await
            .expect("plan");
        tokio::fs::create_dir_all(&plan.directory_path)
            .await
            .expect("unexpected final directory");
        tokio::fs::write(&plan.manifest_path, "owned by someone else\n")
            .await
            .expect("unexpected manifest");
        tokio::fs::write(&plan.markdown_path, "do not replace\n")
            .await
            .expect("unexpected markdown");

        let error = rambledesk_core::FeedbackPackagePublisher::publish(&store, &plan)
            .await
            .expect_err("mismatch must fail");
        assert_eq!(error, RepositoryError::PackagePublish);
        assert_eq!(
            tokio::fs::read_to_string(&plan.manifest_path)
                .await
                .expect("manifest preserved"),
            "owned by someone else\n"
        );
        assert_eq!(
            tokio::fs::read_to_string(&plan.markdown_path)
                .await
                .expect("markdown preserved"),
            "do not replace\n"
        );
        store.close().await;
    }

    #[tokio::test]
    async fn mismatched_pending_package_does_not_block_startup() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create request");
        let draft = application
            .save_feedback_draft(SaveDraftInput {
                request_id: request_id.clone(),
                body_markdown: "Keep the workbench available for repair.".to_owned(),
                expected_revision: 0,
            })
            .await
            .expect("save draft");
        let plan = store
            .plan_submission(
                &request_id,
                draft.saved_revision,
                &Uuid::now_v7().to_string(),
                "2026-07-29T15:30:00Z",
            )
            .await
            .expect("plan");
        tokio::fs::create_dir_all(&plan.directory_path)
            .await
            .expect("unexpected final directory");
        tokio::fs::write(&plan.manifest_path, "mismatch\n")
            .await
            .expect("unexpected manifest");
        tokio::fs::write(&plan.markdown_path, "preserve\n")
            .await
            .expect("unexpected markdown");
        store.close().await;

        let reopened = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("one failed recovery must not block startup");
        let error_code: String = sqlx::query_scalar(
            "SELECT last_error_code FROM submission_plans WHERE request_id = ?1",
        )
        .bind(&request_id)
        .fetch_one(&reopened.pool)
        .await
        .expect("diagnostic recovery error");
        assert_eq!(error_code, "PACKAGE_PUBLISH_FAILURE");
        let request = reopened
            .clone()
            .into_application()
            .get_feedback(GetFeedbackInput { request_id })
            .await
            .expect("request remains visible");
        assert_eq!(request.status, FeedbackStatus::InProgress);
        reopened.close().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn publisher_rejects_feedback_parent_replaced_by_symlink_after_plan() {
        use std::os::unix::fs::symlink;

        let workspace = TestWorkspace::new().await;
        let outside = workspace._temp.path().join("outside-after-plan");
        tokio::fs::create_dir(&outside)
            .await
            .expect("outside directory");
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create request");
        let draft = application
            .save_feedback_draft(SaveDraftInput {
                request_id: request_id.clone(),
                body_markdown: "Revalidate the frozen target before writing.".to_owned(),
                expected_revision: 0,
            })
            .await
            .expect("save draft");
        let plan = store
            .plan_submission(
                &request_id,
                draft.saved_revision,
                &Uuid::now_v7().to_string(),
                "2026-07-29T16:00:00Z",
            )
            .await
            .expect("plan");
        let feedback_root = Path::new(&plan.directory_path)
            .parent()
            .expect("feedback root");
        tokio::fs::remove_dir(feedback_root)
            .await
            .expect("replace empty feedback root");
        symlink(&outside, feedback_root).expect("replacement symlink");

        let error = rambledesk_core::FeedbackPackagePublisher::publish(&store, &plan)
            .await
            .expect_err("publisher must reject swapped parent");
        assert_eq!(error, RepositoryError::PackagePublish);
        assert_eq!(
            std::fs::read_dir(&outside)
                .expect("outside remains readable")
                .count(),
            0
        );
        store.close().await;
    }

    #[tokio::test]
    async fn attachments_share_revision_publish_in_order_and_survive_restart() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        let mut request = workspace.request(request_id.clone());
        request.what_happened = "中文反馈请求：检查图片和正文是否完整".to_owned();
        request.actions[0].instruction = "边截图边记录中文说明".to_owned();
        application
            .request_feedback(request)
            .await
            .expect("create request");

        let first_bytes = b"\x89PNG\r\n\x1a\nfirst-image".to_vec();
        let first = application
            .add_feedback_attachment(AddAttachmentInput {
                request_id: request_id.clone(),
                file_name: "first.png".to_owned(),
                contents: first_bytes.clone(),
                expected_revision: 0,
            })
            .await
            .expect("add first attachment");
        assert_eq!(first.request.revision, 1);
        assert_eq!(first.draft.saved_revision, 1);
        assert_eq!(first.attachments.len(), 1);
        let first_id = first.attachments[0].attachment_id.clone();
        assert_eq!(
            application
                .read_feedback_attachment(request_id.clone(), first_id.clone())
                .await
                .expect("read attachment"),
            first_bytes
        );

        let stale = application
            .add_feedback_attachment(AddAttachmentInput {
                request_id: request_id.clone(),
                file_name: "stale.gif".to_owned(),
                contents: b"GIF89astale".to_vec(),
                expected_revision: 0,
            })
            .await
            .expect_err("stale aggregate revision must conflict");
        assert_eq!(stale.code(), "DRAFT_CONFLICT");

        let second_bytes = b"\xff\xd8\xffsecond-image".to_vec();
        let second = application
            .add_feedback_attachment(AddAttachmentInput {
                request_id: request_id.clone(),
                file_name: "second.jpg".to_owned(),
                contents: second_bytes.clone(),
                expected_revision: 1,
            })
            .await
            .expect("add second attachment");
        let second_id = second.attachments[1].attachment_id.clone();
        let reordered = application
            .reorder_feedback_attachments(ReorderAttachmentsInput {
                request_id: request_id.clone(),
                attachment_ids: vec![second_id.clone(), first_id.clone()],
                expected_revision: 2,
            })
            .await
            .expect("reorder attachments");
        assert_eq!(reordered.request.revision, 3);
        assert_eq!(reordered.attachments[0].attachment_id, second_id);

        let removed = application
            .remove_feedback_attachment(RemoveAttachmentInput {
                request_id: request_id.clone(),
                attachment_id: first_id,
                expected_revision: 3,
            })
            .await
            .expect("remove attachment");
        assert_eq!(removed.request.revision, 4);
        assert_eq!(removed.attachments.len(), 1);
        let draft = application
            .save_feedback_draft(SaveDraftInput {
                request_id: request_id.clone(),
                body_markdown: format!(
                    "图片前的中文说明。\n\n![中文截图](attachment://{second_id})\n\n图片后的中文结论。"
                ),
                expected_revision: 4,
            })
            .await
            .expect("save feedback");
        let submitted = application
            .submit_feedback(SubmitFeedbackInput {
                request_id: request_id.clone(),
                expected_revision: draft.saved_revision,
            })
            .await
            .expect("publish feedback");
        let result = submitted.feedback.expect("feedback package");
        let published_markdown = tokio::fs::read_to_string(&result.markdown_path)
            .await
            .expect("published Markdown");
        assert!(published_markdown.contains("中文反馈请求：检查图片和正文是否完整"));
        assert!(published_markdown.contains("边截图边记录中文说明"));
        assert!(published_markdown.contains("图片前的中文说明。"));
        assert!(published_markdown.contains("![中文截图](attachments/001-second.jpg)"));
        assert!(published_markdown.contains("图片后的中文结论。"));
        assert!(!published_markdown.contains("attachment://"));
        assert!(!published_markdown.contains("## Attachments"));
        let manifest: serde_json::Value = serde_json::from_str(
            &tokio::fs::read_to_string(&result.manifest_path)
                .await
                .expect("manifest"),
        )
        .expect("valid manifest");
        assert_eq!(manifest["attachments"][0]["file_name"], "second.jpg");
        assert_eq!(
            manifest["attachments"][0]["path"],
            "attachments/001-second.jpg"
        );
        let published_attachment =
            Path::new(&result.directory_path).join("attachments/001-second.jpg");
        assert_eq!(
            tokio::fs::read(published_attachment)
                .await
                .expect("published attachment"),
            second_bytes
        );
        store.close().await;

        let reopened = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("reopen store");
        let recovered = reopened
            .clone()
            .into_application()
            .get_feedback_workspace(request_id)
            .await
            .expect("recover workspace");
        assert_eq!(recovered.attachments.len(), 1);
        assert_eq!(recovered.attachments[0].file_name, "second.jpg");
        assert_eq!(recovered.feedback.as_ref(), Some(&result));
        reopened.close().await;
    }

    #[tokio::test]
    async fn feedback_history_filters_and_paginates_without_duplicates() {
        let workspace = TestWorkspace::new().await;
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        for _ in 0..3 {
            application
                .request_feedback(workspace.request(Uuid::now_v7().to_string()))
                .await
                .expect("create request");
        }

        let first = application
            .list_feedback_requests(ListFeedbackRequestsInput {
                agent: Some("test-agent".to_owned()),
                limit: Some(2),
                ..Default::default()
            })
            .await
            .expect("first page");
        assert_eq!(first.requests.len(), 2);
        let cursor = first.next_cursor.expect("next cursor");
        let second = application
            .list_feedback_requests(ListFeedbackRequestsInput {
                agent: Some("test-agent".to_owned()),
                limit: Some(2),
                cursor: Some(cursor),
                ..Default::default()
            })
            .await
            .expect("second page");
        assert_eq!(second.requests.len(), 1);
        assert!(second.next_cursor.is_none());
        assert!(first.requests.iter().all(|left| {
            second
                .requests
                .iter()
                .all(|right| left.request_id != right.request_id)
        }));

        let invalid = application
            .list_feedback_requests(ListFeedbackRequestsInput {
                cursor: Some("not-a-cursor".to_owned()),
                ..Default::default()
            })
            .await
            .expect_err("invalid cursor");
        assert_eq!(invalid.code(), "INVALID_ARGUMENT");
        store.close().await;
    }

    #[tokio::test]
    async fn completed_without_result_is_reported_as_corrupt() {
        let workspace = TestWorkspace::new().await;
        let request_id = Uuid::now_v7().to_string();
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let application = store.clone().into_application();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create request");
        sqlx::query(
            "UPDATE feedback_requests SET status = 'completed', completed_at = ?2, \
             updated_at = ?2 WHERE id = ?1",
        )
        .bind(&request_id)
        .bind("2026-07-29T14:30:00Z")
        .execute(&store.pool)
        .await
        .expect("corrupt completed fixture");
        let error = application
            .get_feedback(GetFeedbackInput { request_id })
            .await
            .expect_err("missing result must not look completed");
        assert_eq!(error.code(), "STORAGE_FAILURE");
        store.close().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn existing_database_permissions_are_repaired() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temporary directory");
        let database = directory.path().join("rambledesk.sqlite3");
        tokio::fs::write(&database, [])
            .await
            .expect("empty database file");
        tokio::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o644))
            .await
            .expect("permissive fixture");

        let store = SqliteFeedbackStore::connect(&database)
            .await
            .expect("open store");
        let mode = tokio::fs::metadata(&database)
            .await
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        store.close().await;
    }

    #[tokio::test]
    async fn migration_installs_the_full_foundation_contract() {
        let workspace = TestWorkspace::new().await;
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .expect("open store");
        let names: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master \
             WHERE type IN ('table', 'trigger', 'index') AND name NOT LIKE 'sqlite_%'",
        )
        .fetch_all(&store.pool)
        .await
        .expect("schema objects");
        for expected in [
            "projects",
            "agent_sessions",
            "feedback_requests",
            "request_actions",
            "request_context_refs",
            "drafts",
            "attachments",
            "invocation_attempts",
            "completion_notifications",
            "feedback_results",
            "submission_plans",
            "outbox_events",
            "feedback_requests_completed_is_terminal",
            "feedback_requests_cancelled_is_terminal",
            "feedback_requests_status_updated",
            "drafts_locked_after_submission_plan_update",
            "drafts_locked_after_submission_plan_delete",
            "agent_sessions_project",
            "outbox_events_pending",
        ] {
            assert!(
                names.iter().any(|name| name == expected),
                "missing {expected}"
            );
        }

        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&store.pool)
            .await
            .expect("foreign_keys pragma");
        let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&store.pool)
            .await
            .expect("journal_mode pragma");
        assert_eq!(foreign_keys, 1);
        assert_eq!(journal_mode, "wal");
        store.close().await;
    }
}
