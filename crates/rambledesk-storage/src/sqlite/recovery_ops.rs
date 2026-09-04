use async_trait::async_trait;
use rambledesk_core::{
    SessionRecovery, SessionRecoveryRepository, SessionRecoveryStatus, SessionRepositoryError,
    SessionRunEnd,
};
use sha2::{Digest, Sha256};
use sqlx::{Row, sqlite::SqliteRow};

use super::SqliteFeedbackStore;

const RECOVERY_SELECT: &str = "SELECT ms.session_id, \
    COALESCE(sr.status, CASE WHEN ms.remote_session_id IS NULL THEN 'never_started' ELSE 'interrupted' END) AS status, \
    sr.run_id, sr.active_turn_id, sr.interrupted_turn_id, sr.last_error, \
    COALESCE(sr.updated_at, hs.updated_at) AS updated_at \
    FROM managed_sessions ms JOIN host_sessions hs ON hs.id = ms.session_id \
    LEFT JOIN session_recovery sr ON sr.session_id = ms.session_id";

#[async_trait]
impl SessionRecoveryRepository for SqliteFeedbackStore {
    async fn get_session_recovery(
        &self,
        session_id: &str,
    ) -> Result<SessionRecovery, SessionRepositoryError> {
        let row = sqlx::query(&format!("{RECOVERY_SELECT} WHERE ms.session_id = ?1"))
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?
            .ok_or(SessionRepositoryError::SessionNotFound)?;
        from_row(&row)
    }

    async fn begin_run(
        &self,
        session_id: &str,
        run_id: &str,
        now: &str,
    ) -> Result<SessionRecovery, SessionRepositoryError> {
        validate(&[session_id, run_id, now])?;
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let existing = load(&mut transaction, session_id).await?;
        if existing.status == SessionRecoveryStatus::Unclosed {
            if existing.run_id.as_deref() != Some(run_id) {
                return Err(SessionRepositoryError::Conflict);
            }
            transaction.commit().await.map_err(storage_error)?;
            return Ok(existing);
        }
        // Reusing the identity of a closed run would let a late callback from that
        // process act on a replacement. Every new process launch needs a fresh id.
        if existing.run_id.as_deref() == Some(run_id) {
            return Err(SessionRepositoryError::Conflict);
        }
        sqlx::query(
            "INSERT INTO session_recovery(session_id,status,run_id,updated_at) VALUES (?1,'unclosed',?2,?3) \
             ON CONFLICT(session_id) DO UPDATE SET status='unclosed',run_id=excluded.run_id, \
             active_turn_id=NULL,last_error=NULL,updated_at=excluded.updated_at",
        ).bind(session_id).bind(run_id).bind(now).execute(&mut *transaction).await.map_err(storage_error)?;
        let checkpoint = load(&mut transaction, session_id).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(checkpoint)
    }

    async fn begin_turn(
        &self,
        session_id: &str,
        run_id: &str,
        turn_id: &str,
        now: &str,
    ) -> Result<SessionRecovery, SessionRepositoryError> {
        validate(&[session_id, run_id, turn_id, now])?;
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let checkpoint = load(&mut transaction, session_id).await?;
        if checkpoint.status != SessionRecoveryStatus::Unclosed
            || checkpoint.run_id.as_deref() != Some(run_id)
            || checkpoint
                .active_turn_id
                .as_deref()
                .is_some_and(|id| id != turn_id)
        {
            return Err(SessionRepositoryError::Conflict);
        }
        if checkpoint.active_turn_id.as_deref() == Some(turn_id) {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(checkpoint);
        }
        sqlx::query(
            "UPDATE session_recovery SET active_turn_id=?2, updated_at=?3 WHERE session_id=?1",
        )
        .bind(session_id)
        .bind(turn_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        let checkpoint = load(&mut transaction, session_id).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(checkpoint)
    }

    async fn finish_turn(
        &self,
        session_id: &str,
        run_id: &str,
        turn_id: &str,
        now: &str,
    ) -> Result<SessionRecovery, SessionRepositoryError> {
        validate(&[session_id, run_id, turn_id, now])?;
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let changed = sqlx::query(
            "UPDATE session_recovery SET active_turn_id=NULL,updated_at=?4 \
             WHERE session_id=?1 AND run_id=?2 AND active_turn_id=?3 AND status='unclosed'",
        )
        .bind(session_id)
        .bind(run_id)
        .bind(turn_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if changed.rows_affected() != 1 {
            return Err(SessionRepositoryError::Conflict);
        }
        let checkpoint = load(&mut transaction, session_id).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(checkpoint)
    }

    async fn close_run(
        &self,
        session_id: &str,
        run_id: &str,
        end: SessionRunEnd,
        last_error: Option<&str>,
        now: &str,
    ) -> Result<SessionRecovery, SessionRepositoryError> {
        validate(&[session_id, run_id, now])?;
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let checkpoint = load(&mut transaction, session_id).await?;
        if checkpoint.run_id.as_deref() != Some(run_id) {
            return Err(SessionRepositoryError::Conflict);
        }
        let target = SessionRecoveryStatus::try_from(end.as_str())?;
        if checkpoint.status == target {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(checkpoint);
        }
        if checkpoint.status != SessionRecoveryStatus::Unclosed {
            return Err(SessionRepositoryError::Conflict);
        }
        close_checkpoint(&mut transaction, &checkpoint, end, last_error, now).await?;
        let checkpoint = load(&mut transaction, session_id).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(checkpoint)
    }

    async fn recover_open_runs(
        &self,
        now: &str,
    ) -> Result<Vec<SessionRecovery>, SessionRepositoryError> {
        validate(&[now])?;
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let rows = sqlx::query(&format!(
            "{RECOVERY_SELECT} WHERE sr.status='unclosed' ORDER BY ms.session_id"
        ))
        .fetch_all(&mut *transaction)
        .await
        .map_err(storage_error)?;
        let mut recovered = Vec::with_capacity(rows.len());
        for row in rows {
            let checkpoint = from_row(&row)?;
            close_checkpoint(
                &mut transaction,
                &checkpoint,
                SessionRunEnd::Interrupted,
                Some("Application stopped before the agent run was closed."),
                now,
            )
            .await?;
            recovered.push(load(&mut transaction, &checkpoint.session_id).await?);
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(recovered)
    }
}

async fn close_checkpoint(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    checkpoint: &SessionRecovery,
    end: SessionRunEnd,
    last_error: Option<&str>,
    now: &str,
) -> Result<(), SessionRepositoryError> {
    if let Some(turn_id) = &checkpoint.active_turn_id {
        let identity = serde_json::to_vec(&[
            Some(checkpoint.session_id.as_str()),
            checkpoint.run_id.as_deref(),
            Some(turn_id.as_str()),
        ])
        .map_err(storage_error)?;
        let activity_id = format!("interrupted-turn-{}", hex::encode(Sha256::digest(identity)));
        sqlx::query(
            "INSERT INTO session_activity(id,session_id,sequence,turn_id,kind,text,created_at) \
             SELECT ?1,?2,COALESCE(MAX(sequence),0)+1,?3,'error','Turn interrupted before completion.',?4 \
             FROM session_activity WHERE session_id=?2 ON CONFLICT(id) DO NOTHING",
        ).bind(activity_id).bind(&checkpoint.session_id).bind(turn_id).bind(now)
            .execute(&mut **transaction).await.map_err(storage_error)?;
    }
    sqlx::query(
        "UPDATE session_recovery SET status=?2,interrupted_turn_id=COALESCE(active_turn_id,interrupted_turn_id), \
         active_turn_id=NULL,last_error=?3,updated_at=?4 WHERE session_id=?1",
    ).bind(&checkpoint.session_id).bind(end.as_str()).bind(last_error).bind(now)
        .execute(&mut **transaction).await.map_err(storage_error)?;
    sqlx::query("UPDATE host_sessions SET updated_at=MAX(updated_at,?2) WHERE id=?1")
        .bind(&checkpoint.session_id)
        .bind(now)
        .execute(&mut **transaction)
        .await
        .map_err(storage_error)?;
    Ok(())
}

async fn load(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    session_id: &str,
) -> Result<SessionRecovery, SessionRepositoryError> {
    let row = sqlx::query(&format!("{RECOVERY_SELECT} WHERE ms.session_id=?1"))
        .bind(session_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(storage_error)?
        .ok_or(SessionRepositoryError::SessionNotFound)?;
    from_row(&row)
}

fn from_row(row: &SqliteRow) -> Result<SessionRecovery, SessionRepositoryError> {
    let status: String = row.try_get("status").map_err(storage_error)?;
    Ok(SessionRecovery {
        session_id: row.try_get("session_id").map_err(storage_error)?,
        status: SessionRecoveryStatus::try_from(status.as_str())?,
        run_id: row.try_get("run_id").map_err(storage_error)?,
        active_turn_id: row.try_get("active_turn_id").map_err(storage_error)?,
        interrupted_turn_id: row.try_get("interrupted_turn_id").map_err(storage_error)?,
        last_error: row.try_get("last_error").map_err(storage_error)?,
        updated_at: row.try_get("updated_at").map_err(storage_error)?,
    })
}

fn validate(values: &[&str]) -> Result<(), SessionRepositoryError> {
    if values
        .iter()
        .any(|value| value.trim().is_empty() || value.contains('\0'))
    {
        Err(SessionRepositoryError::InvalidInput)
    } else {
        Ok(())
    }
}

fn storage_error<T>(_error: T) -> SessionRepositoryError {
    SessionRepositoryError::Storage
}
