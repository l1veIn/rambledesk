use rambledesk_core::{MAX_SESSION_ACTIVITY_PAGE_SIZE, SessionActivity, SessionRepositoryError};
use sqlx::Row;

use super::{SqliteFeedbackStore, activity_ops::activity_from_row};

// Bound history transfer independently from turn count. Keep one oversize row so
// every cursor advances and the reader can still reach that activity.
const HISTORY_PAYLOAD_BUDGET: usize = 2 * 1024 * 1024;

impl SqliteFeedbackStore {
    pub(super) async fn turn_activity_history(
        &self,
        session_id: &str,
        before_sequence: u64,
        turn_limit: u32,
        limit: u32,
    ) -> Result<Vec<SessionActivity>, SessionRepositoryError> {
        if limit == 0
            || limit > MAX_SESSION_ACTIVITY_PAGE_SIZE
            || !(1..=50).contains(&turn_limit)
            || before_sequence == 0
        {
            return Err(SessionRepositoryError::InvalidInput);
        }
        let before: i64 = before_sequence
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
        // The partial user-message index finds the Nth prompt without decoding
        // thousands of tool outputs. Both boundaries use the same DB snapshot.
        let start: i64 = sqlx::query_scalar(
            "SELECT COALESCE((SELECT sequence FROM session_activity WHERE session_id = ?1 AND sequence < ?2 AND kind = 'user_message' ORDER BY sequence DESC LIMIT 1 OFFSET ?3), 1)"
        ).bind(session_id).bind(before).bind(i64::from(turn_limit - 1))
            .fetch_one(&mut *transaction).await.map_err(storage_error)?;
        let sizes = sqlx::query(
            "SELECT sequence, LENGTH(CAST(text AS BLOB)) + COALESCE(LENGTH(CAST(content_json AS BLOB)), 0) AS payload_bytes FROM session_activity WHERE session_id = ?1 AND sequence >= ?2 AND sequence < ?3 ORDER BY sequence DESC LIMIT ?4"
        ).bind(session_id).bind(start).bind(before).bind(i64::from(limit))
            .fetch_all(&mut *transaction).await.map_err(storage_error)?;
        let mut bounded_start = None;
        let mut payload_bytes = 0;
        for row in &sizes {
            let size: i64 = row.try_get("payload_bytes").map_err(storage_error)?;
            // Include allowance for IDs/timestamps/JSON framing, not only text.
            payload_bytes += usize::try_from(size)
                .map_err(storage_error)?
                .saturating_add(512);
            if bounded_start.is_some() && payload_bytes > HISTORY_PAYLOAD_BUDGET {
                break;
            }
            bounded_start = Some(row.try_get::<i64, _>("sequence").map_err(storage_error)?);
        }
        // Fetch/decode only the selected payload, never materialize all candidate
        // tool outputs before applying the byte budget.
        let rows = if let Some(start) = bounded_start {
            sqlx::query("SELECT * FROM session_activity WHERE session_id = ?1 AND sequence >= ?2 AND sequence < ?3 ORDER BY sequence")
                .bind(session_id).bind(start).bind(before).fetch_all(&mut *transaction).await.map_err(storage_error)?
        } else {
            Vec::new()
        };
        transaction.commit().await.map_err(storage_error)?;
        rows.iter().map(activity_from_row).collect()
    }
}

fn storage_error<T>(_: T) -> SessionRepositoryError {
    SessionRepositoryError::Storage
}
