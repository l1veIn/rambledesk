use std::{path::Path, sync::Arc, time::Duration};

use async_trait::async_trait;
use rambledesk_core::{
    FeedbackRepository, FeedbackStatus, NewFeedbackRequest, ProjectInput, RepositoryError,
    StoredFeedbackRequest,
};
use sqlx::{
    Row, SqlitePool,
    migrate::Migrator,
    sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow, SqliteSynchronous,
    },
};
use thiserror::Error;

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
    #[error("no local application data directory is available")]
    DataDirectoryUnavailable,
}

#[derive(Clone)]
pub struct SqliteFeedbackStore {
    pool: SqlitePool,
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
        Ok(Self { pool })
    }

    pub fn into_application(self) -> rambledesk_core::FeedbackApplication {
        rambledesk_core::FeedbackApplication::new(Arc::new(self))
    }

    pub async fn close(&self) {
        self.pool.close().await;
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
        };
        transaction.commit().await.map_err(storage_error)?;
        Ok(stored)
    }

    async fn get_request(
        &self,
        request_id: &str,
    ) -> Result<StoredFeedbackRequest, RepositoryError> {
        let row = sqlx::query(
            "SELECT r.id, s.project_id, r.status, r.created_at, r.updated_at, r.input_hash \
             FROM feedback_requests r \
             JOIN agent_sessions s ON s.id = r.session_id \
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
                r.status, r.created_at, r.updated_at, r.input_hash \
         FROM feedback_requests r \
         JOIN agent_sessions s ON s.id = r.session_id \
         JOIN projects p ON p.id = s.project_id \
         WHERE r.id = ?1",
    )
    .bind(request_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(storage_error)
}

fn stored_request_from_row(row: &SqliteRow) -> Result<StoredFeedbackRequest, RepositoryError> {
    Ok(StoredFeedbackRequest {
        request_id: row.try_get("id").map_err(storage_error)?,
        project_id: row.try_get("project_id").map_err(storage_error)?,
        status: stored_status(row)?,
        created_at: row.try_get("created_at").map_err(storage_error)?,
        updated_at: row.try_get("updated_at").map_err(storage_error)?,
    })
}

fn stored_status(row: &SqliteRow) -> Result<FeedbackStatus, RepositoryError> {
    let status: String = row.try_get("status").map_err(storage_error)?;
    FeedbackStatus::try_from(status.as_str())
}

fn storage_error<T>(_error: T) -> RepositoryError {
    RepositoryError::Storage
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
        ActionInput, CancelFeedbackInput, ContextRef, FeedbackStatus, GetFeedbackInput,
        ProjectInput, RequestFeedbackInput,
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
            "outbox_events",
            "feedback_requests_completed_is_terminal",
            "feedback_requests_cancelled_is_terminal",
            "feedback_requests_status_updated",
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
