use super::*;

/// Validate the trusted delivery marker before lookup or insertion, inside the
/// same write transaction. A model-controlled external correlation pair cannot
/// acquire a managed session merely by guessing the pair or a request id.
pub(super) async fn validate_request_scope(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request: &NewFeedbackRequest,
) -> Result<(), RepositoryError> {
    match request.managed_session_id.as_deref() {
        Some(session_id) => {
            if session_id != request.host_session_record_id {
                return Err(RepositoryError::RequestNotFound);
            }
            let matches: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM managed_sessions ms \
                 JOIN host_sessions hs ON hs.id = ms.session_id \
                 WHERE ms.session_id = ?1 AND hs.host_id = ?2 AND hs.host_session_id = ?3 \
                 AND ms.lifecycle = 'active' \
                 AND NOT EXISTS(SELECT 1 FROM session_deletions sd WHERE sd.session_id = ms.session_id))",
            )
            .bind(session_id)
            .bind(&request.host_id)
            .bind(&request.host_session_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(storage_error)?;
            if !matches {
                return Err(RepositoryError::RequestNotFound);
            }
        }
        None => {
            let managed: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM managed_sessions ms \
                 JOIN host_sessions hs ON hs.id = ms.session_id \
                 WHERE hs.host_id = ?1 AND hs.host_session_id = ?2)",
            )
            .bind(&request.host_id)
            .bind(&request.host_session_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(storage_error)?;
            if managed {
                return Err(RepositoryError::RequestNotFound);
            }
        }
    }
    Ok(())
}

pub(super) fn validate_existing_request_scope(
    existing: &SqliteRow,
    request: &NewFeedbackRequest,
) -> Result<(), RepositoryError> {
    let managed_session_id: Option<String> = existing
        .try_get("managed_session_id")
        .map_err(storage_error)?;
    if managed_session_id != request.managed_session_id {
        return Err(RepositoryError::RequestNotFound);
    }
    Ok(())
}
