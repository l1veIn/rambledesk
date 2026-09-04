use async_trait::async_trait;
use rambledesk_core::{
    MAX_SESSION_ACTIVITY_PAGE_SIZE, NewSessionActivity, SessionActivity, SessionActivityKind,
    SessionActivityRepository, SessionRepositoryError,
};
use sqlx::{Row, sqlite::SqliteRow};

use super::SqliteFeedbackStore;

#[async_trait]
impl SessionActivityRepository for SqliteFeedbackStore {
    async fn append_activity(
        &self,
        activity: NewSessionActivity,
    ) -> Result<SessionActivity, SessionRepositoryError> {
        if !valid_id(&activity.id)
            || !valid_id(&activity.session_id)
            || !valid_id(&activity.created_at)
            || activity.turn_id.as_deref().is_some_and(|id| !valid_id(id))
            || activity
                .tool_call_id
                .as_deref()
                .is_some_and(|id| !valid_id(id))
        {
            return Err(SessionRepositoryError::InvalidInput);
        }
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        if let Some(row) = sqlx::query("SELECT * FROM session_activity WHERE id = ?1")
            .bind(&activity.id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(storage_error)?
        {
            let stored = activity_from_row(&row)?;
            if stored.session_id != activity.session_id
                || stored.turn_id != activity.turn_id
                || stored.kind != activity.kind
                || stored.text != activity.text
                || stored.tool_call_id != activity.tool_call_id
                || stored.created_at != activity.created_at
            {
                return Err(SessionRepositoryError::Conflict);
            }
            transaction.commit().await.map_err(storage_error)?;
            return Ok(stored);
        }
        let managed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM managed_sessions WHERE session_id = ?1)",
        )
        .bind(&activity.session_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if !managed {
            return Err(SessionRepositoryError::SessionNotFound);
        }
        let sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM session_activity WHERE session_id = ?1",
        )
        .bind(&activity.session_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;
        sqlx::query(
            "INSERT INTO session_activity (id, session_id, sequence, turn_id, kind, text, tool_call_id, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&activity.id).bind(&activity.session_id).bind(sequence).bind(&activity.turn_id)
        .bind(activity.kind.as_str()).bind(&activity.text).bind(&activity.tool_call_id)
        .bind(&activity.created_at).execute(&mut *transaction).await.map_err(storage_error)?;
        sqlx::query("UPDATE host_sessions SET updated_at = MAX(updated_at, ?2) WHERE id = ?1")
            .bind(&activity.session_id)
            .bind(&activity.created_at)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(SessionActivity {
            id: activity.id,
            session_id: activity.session_id,
            sequence: sequence
                .try_into()
                .map_err(|_| SessionRepositoryError::CorruptData)?,
            turn_id: activity.turn_id,
            kind: activity.kind,
            text: activity.text,
            tool_call_id: activity.tool_call_id,
            created_at: activity.created_at,
        })
    }

    async fn list_session_activity(
        &self,
        session_id: &str,
        after_sequence: Option<u64>,
        limit: u32,
    ) -> Result<Vec<SessionActivity>, SessionRepositoryError> {
        if limit == 0 || limit > MAX_SESSION_ACTIVITY_PAGE_SIZE {
            return Err(SessionRepositoryError::InvalidInput);
        }
        let after: i64 = after_sequence
            .unwrap_or(0)
            .try_into()
            .map_err(|_| SessionRepositoryError::InvalidInput)?;
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let managed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM managed_sessions WHERE session_id = ?1)",
        )
        .bind(session_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if !managed {
            return Err(SessionRepositoryError::SessionNotFound);
        }
        let rows = sqlx::query(
            "SELECT * FROM session_activity WHERE session_id = ?1 AND sequence > ?2 \
             ORDER BY sequence LIMIT ?3",
        )
        .bind(session_id)
        .bind(after)
        .bind(i64::from(limit))
        .fetch_all(&mut *transaction)
        .await
        .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        rows.iter().map(activity_from_row).collect()
    }

    async fn list_recent_session_activity(
        &self,
        session_id: &str,
        limit: u32,
    ) -> Result<Vec<SessionActivity>, SessionRepositoryError> {
        if limit == 0 || limit > MAX_SESSION_ACTIVITY_PAGE_SIZE {
            return Err(SessionRepositoryError::InvalidInput);
        }
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let managed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM managed_sessions WHERE session_id = ?1)",
        )
        .bind(session_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if !managed {
            return Err(SessionRepositoryError::SessionNotFound);
        }
        let rows = sqlx::query(
            "SELECT * FROM (SELECT * FROM session_activity WHERE session_id = ?1 \
             ORDER BY sequence DESC LIMIT ?2) ORDER BY sequence",
        )
        .bind(session_id)
        .bind(i64::from(limit))
        .fetch_all(&mut *transaction)
        .await
        .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        rows.iter().map(activity_from_row).collect()
    }

    async fn update_activity_text(
        &self,
        id: &str,
        session_id: &str,
        text: &str,
    ) -> Result<SessionActivity, SessionRepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let row = sqlx::query(
            "UPDATE session_activity SET text = ?3 WHERE id = ?1 AND session_id = ?2 RETURNING *",
        )
        .bind(id)
        .bind(session_id)
        .bind(text)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage_error)?
        .ok_or(SessionRepositoryError::SessionNotFound)?;
        let updated = activity_from_row(&row)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(updated)
    }
}

fn activity_from_row(row: &SqliteRow) -> Result<SessionActivity, SessionRepositoryError> {
    let sequence: i64 = row.try_get("sequence").map_err(storage_error)?;
    let kind: String = row.try_get("kind").map_err(storage_error)?;
    Ok(SessionActivity {
        id: row.try_get("id").map_err(storage_error)?,
        session_id: row.try_get("session_id").map_err(storage_error)?,
        sequence: sequence
            .try_into()
            .map_err(|_| SessionRepositoryError::CorruptData)?,
        turn_id: row.try_get("turn_id").map_err(storage_error)?,
        kind: SessionActivityKind::try_from(kind.as_str())?,
        text: row.try_get("text").map_err(storage_error)?,
        tool_call_id: row.try_get("tool_call_id").map_err(storage_error)?,
        created_at: row.try_get("created_at").map_err(storage_error)?,
    })
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && !value.contains('\0')
}

fn storage_error<T>(_error: T) -> SessionRepositoryError {
    SessionRepositoryError::Storage
}
