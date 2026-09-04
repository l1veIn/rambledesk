use async_trait::async_trait;
use rambledesk_core::{
    FeedbackDelivery, FeedbackDeliveryRepository, FeedbackDeliveryState, FeedbackResolution,
    ResolveDeliveryAction, SessionRepositoryError,
};
use sqlx::{Row, sqlite::SqliteRow};

use super::{RepositoryError, SqliteFeedbackStore};

/// Called inside the transaction that publishes the terminal request. Replays
/// preserve the existing delivery state, including uncertain and discarded sends.
pub(super) async fn enqueue_terminal_delivery(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO feedback_deliveries (request_id, session_id, resolution, created_at, updated_at) \
         SELECT id, managed_session_id, resolution, updated_at, updated_at FROM feedback_requests \
         WHERE id = ?1 AND managed_session_id IS NOT NULL AND status IN ('completed', 'cancelled') \
           AND resolution IN ('feedback_submitted', 'approved', 'cancelled') \
         ON CONFLICT(request_id) DO NOTHING",
    )
    .bind(request_id).execute(&mut **transaction).await.map_err(super::storage_error)?;
    Ok(())
}

#[async_trait]
impl FeedbackDeliveryRepository for SqliteFeedbackStore {
    async fn list_session_deliveries(
        &self,
        session_id: &str,
    ) -> Result<Vec<FeedbackDelivery>, SessionRepositoryError> {
        let rows = sqlx::query(
            "SELECT * FROM feedback_deliveries WHERE session_id = ?1 ORDER BY created_at, request_id",
        ).bind(session_id).fetch_all(&self.pool).await.map_err(storage_error)?;
        rows.iter().map(delivery_from_row).collect()
    }

    async fn list_pending_deliveries(
        &self,
    ) -> Result<Vec<FeedbackDelivery>, SessionRepositoryError> {
        let rows = sqlx::query(
            "SELECT * FROM feedback_deliveries WHERE state = 'pending' ORDER BY created_at, request_id",
        ).fetch_all(&self.pool).await.map_err(storage_error)?;
        rows.iter().map(delivery_from_row).collect()
    }

    async fn claim_delivery(
        &self,
        request_id: &str,
        attempt_id: &str,
        now: &str,
    ) -> Result<Option<FeedbackDelivery>, SessionRepositoryError> {
        validate_attempt_time(attempt_id, now)?;
        let row = sqlx::query(
            "UPDATE feedback_deliveries SET state = 'sending', attempt_id = ?2, updated_at = ?3, last_error = NULL \
             WHERE request_id = ?1 AND state = 'pending' RETURNING *",
        ).bind(request_id).bind(attempt_id).bind(now)
            .fetch_optional(&self.pool).await.map_err(storage_error)?;
        row.as_ref().map(delivery_from_row).transpose()
    }

    async fn finish_delivery(
        &self,
        request_id: &str,
        attempt_id: &str,
        state: FeedbackDeliveryState,
        last_error: Option<&str>,
        now: &str,
    ) -> Result<FeedbackDelivery, SessionRepositoryError> {
        validate_attempt_time(attempt_id, now)?;
        if !matches!(
            state,
            FeedbackDeliveryState::Pending
                | FeedbackDeliveryState::Delivered
                | FeedbackDeliveryState::Uncertain
        ) {
            return Err(SessionRepositoryError::InvalidInput);
        }
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let row = sqlx::query(
            "UPDATE feedback_deliveries SET state = ?3, last_error = ?4, updated_at = ?5 \
             WHERE request_id = ?1 AND state = 'sending' AND attempt_id = ?2 RETURNING *",
        )
        .bind(request_id)
        .bind(attempt_id)
        .bind(state.as_str())
        .bind(last_error)
        .bind(now)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage_error)?;
        let result = if let Some(row) = row {
            delivery_from_row(&row)?
        } else {
            let row = sqlx::query("SELECT * FROM feedback_deliveries WHERE request_id = ?1")
                .bind(request_id)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(storage_error)?
                .ok_or(SessionRepositoryError::SessionNotFound)?;
            let existing = delivery_from_row(&row)?;
            if existing.state != state
                || existing.attempt_id.as_deref() != Some(attempt_id)
                || existing.last_error.as_deref() != last_error
            {
                return Err(SessionRepositoryError::Conflict);
            }
            existing
        };
        transaction.commit().await.map_err(storage_error)?;
        Ok(result)
    }

    async fn recover_interrupted_deliveries(
        &self,
        now: &str,
    ) -> Result<u64, SessionRepositoryError> {
        validate_attempt_time("recovery", now)?;
        Ok(sqlx::query(
            "UPDATE feedback_deliveries SET state = 'uncertain', updated_at = ?1, \
             last_error = 'Application stopped before delivery confirmation.' WHERE state = 'sending'",
        ).bind(now).execute(&self.pool).await.map_err(storage_error)?.rows_affected())
    }

    async fn discard_session_deliveries(
        &self,
        session_id: &str,
        now: &str,
    ) -> Result<u64, SessionRepositoryError> {
        validate_attempt_time("discard", now)?;
        Ok(sqlx::query(
            "UPDATE feedback_deliveries SET state = 'discarded', updated_at = ?2 \
             WHERE session_id = ?1 AND state IN ('pending', 'sending', 'uncertain')",
        )
        .bind(session_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(storage_error)?
        .rows_affected())
    }

    async fn resolve_delivery(
        &self,
        request_id: &str,
        session_id: &str,
        action: ResolveDeliveryAction,
        now: &str,
    ) -> Result<FeedbackDelivery, SessionRepositoryError> {
        validate_attempt_time("resolve", now)?;
        let target = match action {
            ResolveDeliveryAction::Retry => "pending",
            ResolveDeliveryAction::Acknowledge => "delivered",
        };
        let row = sqlx::query(
            "UPDATE feedback_deliveries SET state = ?3, updated_at = ?4, last_error = NULL, \
             attempt_id = CASE WHEN ?3 = 'pending' THEN NULL ELSE attempt_id END \
             WHERE request_id = ?1 AND session_id = ?2 AND state = 'uncertain' RETURNING *",
        )
        .bind(request_id)
        .bind(session_id)
        .bind(target)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?
        .ok_or(SessionRepositoryError::Conflict)?;
        delivery_from_row(&row)
    }
}

fn delivery_from_row(row: &SqliteRow) -> Result<FeedbackDelivery, SessionRepositoryError> {
    let resolution: String = row.try_get("resolution").map_err(storage_error)?;
    let state: String = row.try_get("state").map_err(storage_error)?;
    Ok(FeedbackDelivery {
        request_id: row.try_get("request_id").map_err(storage_error)?,
        session_id: row.try_get("session_id").map_err(storage_error)?,
        resolution: FeedbackResolution::try_from(resolution.as_str())
            .map_err(|_| SessionRepositoryError::CorruptData)?,
        state: FeedbackDeliveryState::try_from(state.as_str())?,
        attempt_id: row.try_get("attempt_id").map_err(storage_error)?,
        created_at: row.try_get("created_at").map_err(storage_error)?,
        updated_at: row.try_get("updated_at").map_err(storage_error)?,
        last_error: row.try_get("last_error").map_err(storage_error)?,
    })
}

fn validate_attempt_time(attempt: &str, now: &str) -> Result<(), SessionRepositoryError> {
    if attempt.trim().is_empty()
        || attempt.contains('\0')
        || now.trim().is_empty()
        || now.contains('\0')
    {
        Err(SessionRepositoryError::InvalidInput)
    } else {
        Ok(())
    }
}

fn storage_error<T>(_error: T) -> SessionRepositoryError {
    SessionRepositoryError::Storage
}
