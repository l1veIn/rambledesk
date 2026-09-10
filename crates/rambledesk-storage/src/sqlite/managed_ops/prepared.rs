use super::*;
use rambledesk_core::SessionActivityKind;

impl SqliteFeedbackStore {
    pub(super) async fn promote_prepared_session_impl(
        &self,
        user: NewSessionActivity,
        turn: NewSessionActivity,
        fallback_title: &str,
    ) -> Result<(), SessionRepositoryError> {
        if user.kind != SessionActivityKind::UserMessage
            || turn.kind != SessionActivityKind::Status
            || user.session_id != turn.session_id
            || user.turn_id.is_none()
            || user.turn_id != turn.turn_id
            || !nonempty(fallback_title)
            || fallback_title.chars().count() > 80
            || user.text.trim().is_empty()
        {
            return Err(SessionRepositoryError::InvalidInput);
        }
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let updated = sqlx::query(
            "UPDATE managed_sessions SET lifecycle = 'active' WHERE session_id = ?1 \
             AND lifecycle = 'prepared' AND remote_session_id IS NOT NULL \
             AND NOT EXISTS(SELECT 1 FROM session_deletions WHERE session_id = ?1)",
        )
        .bind(&user.session_id)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(SessionRepositoryError::Conflict);
        }
        let first: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM session_activity WHERE session_id = ?1",
        )
        .bind(&user.session_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;
        for (sequence, activity) in [(first, &user), (first + 1, &turn)] {
            if !nonempty(&activity.id)
                || !nonempty(&activity.session_id)
                || !nonempty(&activity.created_at)
                || activity.turn_id.as_deref().is_none_or(|id| !nonempty(id))
                || activity.tool_call_id.is_some()
            {
                return Err(SessionRepositoryError::InvalidInput);
            }
            let content = activity
                .content
                .as_ref()
                .map(super::super::activity_ops::serialize_content)
                .transpose()?;
            sqlx::query(
                "INSERT INTO session_activity (id,session_id,sequence,turn_id,kind,text,tool_call_id,created_at,content_json) \
                 VALUES (?1,?2,?3,?4,?5,?6,NULL,?7,?8)",
            ).bind(&activity.id).bind(&activity.session_id).bind(sequence).bind(&activity.turn_id)
                .bind(activity.kind.as_str()).bind(&activity.text).bind(&activity.created_at).bind(content)
                .execute(&mut *transaction).await.map_err(write_error)?;
        }
        // A title already supplied by the user always wins over the fallback.
        sqlx::query(
            "UPDATE host_sessions SET display_title = CASE WHEN length(trim(COALESCE(display_title, ''))) = 0 \
             THEN ?2 ELSE display_title END, updated_at = MAX(updated_at, ?3) WHERE id = ?1",
        ).bind(&user.session_id).bind(fallback_title).bind(&user.created_at)
            .execute(&mut *transaction).await.map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)
    }

    pub(super) async fn discard_prepared_session_impl(
        &self,
        session_id: &str,
    ) -> Result<(), SessionRepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let row = sqlx::query("SELECT ms.lifecycle FROM host_sessions hs LEFT JOIN managed_sessions ms ON ms.session_id=hs.id WHERE hs.id=?1")
            .bind(session_id).fetch_optional(&mut *transaction).await.map_err(storage_error)?;
        if let Some(row) = row {
            if row
                .try_get::<Option<&str>, _>("lifecycle")
                .map_err(storage_error)?
                != Some("prepared")
            {
                return Err(SessionRepositoryError::Conflict);
            }
            sqlx::query("DELETE FROM host_sessions WHERE id=?1")
                .bind(session_id)
                .execute(&mut *transaction)
                .await
                .map_err(storage_error)?;
        }
        transaction.commit().await.map_err(storage_error)
    }
}
