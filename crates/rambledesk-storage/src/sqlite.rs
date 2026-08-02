use std::{collections::HashSet, path::Path, sync::Arc, time::Duration};

use async_trait::async_trait;
use rambledesk_core::{
    ActionInput, AttachmentView, ContextRef, DraftView, FeedbackRepository, FeedbackRequestQuery,
    FeedbackRequestSummary, FeedbackResolution, FeedbackResultView, FeedbackStatus,
    HostSessionSummary, MAX_ATTACHMENT_COUNT, NewAttachment, NewFeedbackRequest,
    PublishedFeedbackPackage, RepositoryError, StoredFeedbackRequest, StoredFeedbackWorkspace,
    SubmissionAttachment, SubmissionPlan,
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

mod publication_paths;
mod request_ops;
mod submission_ops;
mod workspace_ops;

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
    library_root: std::path::PathBuf,
    pub(crate) publish_lock: Arc<tokio::sync::Mutex<()>>,
}

impl SqliteFeedbackStore {
    pub async fn connect(path: &Path) -> Result<Self, StorageOpenError> {
        let library_root = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        Self::connect_with_library(path, library_root).await
    }

    pub async fn connect_with_library(
        path: &Path,
        library_root: &Path,
    ) -> Result<Self, StorageOpenError> {
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
        let library_existed = tokio::fs::try_exists(library_root)
            .await
            .map_err(StorageOpenError::CreateDirectory)?;
        tokio::fs::create_dir_all(library_root)
            .await
            .map_err(StorageOpenError::CreateDirectory)?;
        secure_new_path(library_root, library_existed, 0o700).await?;
        let store = Self {
            pool,
            library_root: library_root.to_path_buf(),
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
        rambledesk_core::FeedbackApplication::new(store.clone(), store.clone(), store)
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

pub fn default_app_data_root() -> Result<std::path::PathBuf, StorageOpenError> {
    dirs::data_local_dir()
        .map(|root| root.join("RambleDesk"))
        .ok_or(StorageOpenError::DataDirectoryUnavailable)
}

pub fn default_database_path() -> Result<std::path::PathBuf, StorageOpenError> {
    default_app_data_root().map(|root| root.join("state").join("feedback.sqlite3"))
}

pub fn default_library_path() -> Result<std::path::PathBuf, StorageOpenError> {
    default_app_data_root().map(|root| root.join("library"))
}

#[async_trait]
impl FeedbackRepository for SqliteFeedbackStore {
    async fn create_or_get_request(
        &self,
        request: NewFeedbackRequest,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        self.create_or_get_request_impl(request).await
    }

    async fn get_request(
        &self,
        request_id: &str,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        self.get_request_impl(request_id).await
    }

    async fn cancel_request(
        &self,
        request_id: &str,
        reason: &str,
        now: &str,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        self.cancel_request_impl(request_id, reason, now).await
    }

    async fn approve_request(
        &self,
        request_id: &str,
        now: &str,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        self.approve_request_impl(request_id, now).await
    }

    async fn list_open_requests(&self) -> Result<Vec<FeedbackRequestSummary>, RepositoryError> {
        self.list_open_requests_impl().await
    }

    async fn list_requests(
        &self,
        query: FeedbackRequestQuery,
    ) -> Result<Vec<FeedbackRequestSummary>, RepositoryError> {
        self.list_requests_impl(query).await
    }

    async fn list_host_sessions(&self) -> Result<Vec<HostSessionSummary>, RepositoryError> {
        self.list_host_sessions_impl().await
    }

    async fn get_workspace(
        &self,
        request_id: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError> {
        self.get_workspace_impl(request_id).await
    }

    async fn save_draft(
        &self,
        request_id: &str,
        body_markdown: &str,
        expected_revision: u64,
        now: &str,
    ) -> Result<DraftView, RepositoryError> {
        self.save_draft_impl(request_id, body_markdown, expected_revision, now)
            .await
    }

    async fn add_attachment(
        &self,
        request_id: &str,
        attachment: NewAttachment,
        expected_revision: u64,
        now: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError> {
        self.add_attachment_impl(request_id, attachment, expected_revision, now)
            .await
    }

    async fn remove_attachment(
        &self,
        request_id: &str,
        attachment_id: &str,
        expected_revision: u64,
        now: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError> {
        self.remove_attachment_impl(request_id, attachment_id, expected_revision, now)
            .await
    }

    async fn reorder_attachments(
        &self,
        request_id: &str,
        attachment_ids: &[String],
        expected_revision: u64,
        now: &str,
    ) -> Result<StoredFeedbackWorkspace, RepositoryError> {
        self.reorder_attachments_impl(request_id, attachment_ids, expected_revision, now)
            .await
    }

    async fn read_attachment(
        &self,
        request_id: &str,
        attachment_id: &str,
    ) -> Result<Vec<u8>, RepositoryError> {
        self.read_attachment_impl(request_id, attachment_id).await
    }

    async fn plan_submission(
        &self,
        request_id: &str,
        expected_revision: u64,
        publication_id: &str,
        now: &str,
    ) -> Result<SubmissionPlan, RepositoryError> {
        self.plan_submission_impl(request_id, expected_revision, publication_id, now)
            .await
    }

    async fn complete_submission(
        &self,
        plan: &SubmissionPlan,
        published: &PublishedFeedbackPackage,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        self.complete_submission_impl(plan, published).await
    }
}

mod row_mapping;

use publication_paths::*;
use row_mapping::*;

async fn load_request_row(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
) -> Result<Option<SqliteRow>, RepositoryError> {
    sqlx::query(
        "SELECT r.id, hs.host_id, hs.host_session_id, \
                r.status, r.resolution, r.allow_finish, r.final_summary, \
                r.created_at, r.updated_at, r.input_hash, \
                fr.package_uri, fr.directory_path, fr.markdown_path, fr.manifest_path \
         FROM feedback_requests r \
         JOIN host_sessions hs ON hs.id = r.host_session_record_id \
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
    let resolution = row
        .try_get::<Option<String>, _>("resolution")
        .map_err(storage_error)?
        .map(|value| FeedbackResolution::try_from(value.as_str()))
        .transpose()?;
    if status == FeedbackStatus::Completed
        && resolution != Some(FeedbackResolution::Approved)
        && feedback.is_none()
    {
        return Err(RepositoryError::CorruptData);
    }
    Ok(StoredFeedbackRequest {
        request_id: row.try_get("id").map_err(storage_error)?,
        host_id: row.try_get("host_id").map_err(storage_error)?,
        host_session_id: row.try_get("host_session_id").map_err(storage_error)?,
        status,
        created_at: row.try_get("created_at").map_err(storage_error)?,
        updated_at: row.try_get("updated_at").map_err(storage_error)?,
        feedback,
        resolution,
        allow_finish: row.try_get("allow_finish").map_err(storage_error)?,
        final_summary: row.try_get("final_summary").map_err(storage_error)?,
    })
}

fn host_session_summary_from_row(row: &SqliteRow) -> Result<HostSessionSummary, RepositoryError> {
    let request_count = row
        .try_get::<i64, _>("request_count")
        .map_err(storage_error)?;
    let pending_count = row
        .try_get::<i64, _>("pending_count")
        .map_err(storage_error)?;
    Ok(HostSessionSummary {
        host_id: row.try_get("host_id").map_err(storage_error)?,
        host_session_id: row.try_get("host_session_id").map_err(storage_error)?,
        title: row.try_get("title").map_err(storage_error)?,
        source_hint: row.try_get("source_hint").map_err(storage_error)?,
        request_count: u64::try_from(request_count).map_err(|_| RepositoryError::CorruptData)?,
        pending_count: u64::try_from(pending_count).map_err(|_| RepositoryError::CorruptData)?,
        updated_at: row.try_get("updated_at").map_err(storage_error)?,
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
        RepositoryError::PackageRead => "FEEDBACK_PACKAGE_READ_FAILURE",
        RepositoryError::DraftConflict => "DRAFT_CONFLICT",
        RepositoryError::RequestNotFound => "REQUEST_NOT_FOUND",
        RepositoryError::RequestTerminal | RepositoryError::RequestAlreadyCompleted => {
            "REQUEST_TERMINAL"
        }
        RepositoryError::CorruptData | RepositoryError::Storage => "STORAGE_FAILURE",
        RepositoryError::AttachmentNotFound | RepositoryError::AttachmentLimit => {
            "RECOVERY_FAILURE"
        }
        RepositoryError::RequestConflict | RepositoryError::DraftEmpty => "RECOVERY_FAILURE",
    }
}

mod security;

use security::{secure_new_path, secure_path};

#[cfg(test)]
mod tests;
