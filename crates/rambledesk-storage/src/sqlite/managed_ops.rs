use std::path::Path;

use async_trait::async_trait;
use rambledesk_core::{
    AgentConfig, NewManagedSession, SessionManagement, SessionProtocol, SessionRecord,
    SessionRepository, SessionRepositoryError,
};
use sqlx::{Row, sqlite::SqliteRow};

use super::SqliteFeedbackStore;

mod config;

const SESSION_SELECT: &str = "SELECT hs.id AS session_id, hs.host_id, hs.host_session_id, hs.created_at, hs.updated_at, \
     COALESCE(NULLIF(hs.display_title, ''), \
       (SELECT r.title FROM feedback_requests r WHERE r.host_session_record_id = hs.id \
        ORDER BY r.created_at, r.id LIMIT 1), hs.host_session_id) AS title, \
     ms.protocol, ms.agent_config_id, ms.cwd, ms.remote_session_id \
     FROM host_sessions hs LEFT JOIN managed_sessions ms ON ms.session_id = hs.id";

#[async_trait]
impl SessionRepository for SqliteFeedbackStore {
    async fn save_agent_config(
        &self,
        config: AgentConfig,
    ) -> Result<AgentConfig, SessionRepositoryError> {
        self.save_agent_config_impl(config).await
    }

    async fn get_agent_config(&self, id: &str) -> Result<AgentConfig, SessionRepositoryError> {
        self.get_agent_config_impl(id).await
    }

    async fn list_agent_configs(&self) -> Result<Vec<AgentConfig>, SessionRepositoryError> {
        self.list_agent_configs_impl().await
    }

    async fn delete_agent_config(&self, id: &str) -> Result<(), SessionRepositoryError> {
        self.delete_agent_config_impl(id).await
    }

    async fn create_managed_session(
        &self,
        session: NewManagedSession,
    ) -> Result<SessionRecord, SessionRepositoryError> {
        if !nonempty(&session.session_id)
            || !nonempty(&session.agent_config_id)
            || !nonempty(&session.title)
            || !nonempty(&session.created_at)
            || !nonempty(&session.cwd)
            || !Path::new(&session.cwd).is_absolute()
        {
            return Err(SessionRepositoryError::InvalidInput);
        }
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let config =
            sqlx::query("SELECT host_id, protocol, enabled FROM agent_configs WHERE id = ?1")
                .bind(&session.agent_config_id)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(storage_error)?
                .ok_or(SessionRepositoryError::AgentConfigNotFound)?;
        let enabled: bool = config.try_get("enabled").map_err(storage_error)?;
        if !enabled {
            return Err(SessionRepositoryError::AgentConfigDisabled);
        }
        let host_id: String = config.try_get("host_id").map_err(storage_error)?;
        let protocol: String = config.try_get("protocol").map_err(storage_error)?;
        sqlx::query(
            "INSERT INTO host_sessions \
             (id, host_id, host_session_id, display_title, created_at, updated_at) \
             VALUES (?1, ?2, ?1, ?3, ?4, ?4)",
        )
        .bind(&session.session_id)
        .bind(&host_id)
        .bind(&session.title)
        .bind(&session.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(write_error)?;
        sqlx::query(
            "INSERT INTO managed_sessions (session_id, protocol, agent_config_id, cwd) \
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&session.session_id)
        .bind(&protocol)
        .bind(&session.agent_config_id)
        .bind(&session.cwd)
        .execute(&mut *transaction)
        .await
        .map_err(write_error)?;
        transaction.commit().await.map_err(storage_error)?;
        self.get_session(&session.session_id).await
    }

    async fn get_session(&self, session_id: &str) -> Result<SessionRecord, SessionRepositoryError> {
        let row = sqlx::query(&format!("{SESSION_SELECT} WHERE hs.id = ?1"))
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?
            .ok_or(SessionRepositoryError::SessionNotFound)?;
        record_from_row(&row)
    }

    async fn list_managed_sessions(&self) -> Result<Vec<SessionRecord>, SessionRepositoryError> {
        let rows = sqlx::query(&format!(
            "{SESSION_SELECT} WHERE ms.session_id IS NOT NULL ORDER BY hs.updated_at DESC, hs.id"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;
        rows.iter().map(record_from_row).collect()
    }

    async fn bind_remote_session(
        &self,
        session_id: &str,
        remote_session_id: &str,
        now: &str,
    ) -> Result<SessionRecord, SessionRepositoryError> {
        if !nonempty(remote_session_id) || !nonempty(now) {
            return Err(SessionRepositoryError::InvalidInput);
        }
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let result = sqlx::query(
            "UPDATE managed_sessions SET remote_session_id = ?2 \
             WHERE session_id = ?1 AND (remote_session_id IS NULL OR remote_session_id = ?2)",
        )
        .bind(session_id)
        .bind(remote_session_id)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if result.rows_affected() == 0 {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM managed_sessions WHERE session_id = ?1)",
            )
            .bind(session_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(storage_error)?;
            return Err(if exists {
                SessionRepositoryError::Conflict
            } else {
                SessionRepositoryError::SessionNotFound
            });
        }
        sqlx::query("UPDATE host_sessions SET updated_at = ?2 WHERE id = ?1")
            .bind(session_id)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        self.get_session(session_id).await
    }
}

fn record_from_row(row: &SqliteRow) -> Result<SessionRecord, SessionRepositoryError> {
    Ok(SessionRecord {
        session_id: row.try_get("session_id").map_err(storage_error)?,
        host_id: row.try_get("host_id").map_err(storage_error)?,
        host_session_id: row.try_get("host_session_id").map_err(storage_error)?,
        title: row.try_get("title").map_err(storage_error)?,
        created_at: row.try_get("created_at").map_err(storage_error)?,
        updated_at: row.try_get("updated_at").map_err(storage_error)?,
        management: management_from_row(row)?,
    })
}

pub(super) fn management_from_row(
    row: &SqliteRow,
) -> Result<SessionManagement, SessionRepositoryError> {
    match row
        .try_get::<Option<String>, _>("protocol")
        .map_err(storage_error)?
    {
        None => Ok(SessionManagement::External),
        Some(protocol) if protocol == "acp" => Ok(SessionManagement::Managed {
            protocol: SessionProtocol::Acp,
            agent_config_id: row.try_get("agent_config_id").map_err(storage_error)?,
            cwd: row.try_get("cwd").map_err(storage_error)?,
            remote_session_id: row.try_get("remote_session_id").map_err(storage_error)?,
        }),
        Some(_) => Err(SessionRepositoryError::CorruptData),
    }
}

fn nonempty(value: &str) -> bool {
    !value.trim().is_empty() && !value.contains('\0')
}

fn storage_error<T>(_error: T) -> SessionRepositoryError {
    SessionRepositoryError::Storage
}

fn write_error(error: sqlx::Error) -> SessionRepositoryError {
    if error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation())
    {
        SessionRepositoryError::Conflict
    } else {
        SessionRepositoryError::Storage
    }
}
