use rambledesk_core::kernel::ports::FactStoreError;
use rambledesk_core::kernel::{
    AcpSessionLinkId, AgentWorkId, DeliveryId, DraftId, FactQuery, FactQueryOutcome,
    FeedbackLookup, FeedbackRequestStatus, PendingFeedbackRecovery, RequestId, SessionId,
    SessionKind, SessionRecoverySnapshot, SubmissionId, WorkbenchSnapshot,
};
use sqlx::Row;

use super::read::{
    load_delivery, load_delivery_for_request, load_draft, load_feedback_request, load_link,
    load_session, load_submission, load_work,
};
use super::{SqliteV3Store, storage_error};

impl SqliteV3Store {
    pub(super) async fn query_facts(
        &self,
        query: FactQuery,
    ) -> Result<FactQueryOutcome, FactStoreError> {
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let connection = transaction.as_mut();
        let outcome = match query {
            FactQuery::Feedback(request_id) => {
                let request = load_feedback_request(connection, &request_id)
                    .await?
                    .ok_or(FactStoreError::RequestNotFound)?;
                let session = load_session(connection, &request.session_id)
                    .await?
                    .ok_or(FactStoreError::CorruptData)?;
                if request.status == FeedbackRequestStatus::Waiting {
                    Ok(FactQueryOutcome::Feedback(FeedbackLookup::Waiting {
                        request,
                        session,
                    }))
                } else {
                    let delivery = load_delivery_for_request(connection, &request_id)
                        .await?
                        .ok_or(FactStoreError::CorruptData)?;
                    Ok(FactQueryOutcome::Feedback(FeedbackLookup::Terminal {
                        request,
                        session,
                        delivery: Box::new(delivery),
                    }))
                }
            }
            FactQuery::Workbench(query) => {
                let session_filter = query.session_id.as_ref().map(SessionId::as_str);
                let session_rows = sqlx::query(
                    "SELECT session_id FROM sessions_v3
                     WHERE (?1 IS NULL OR session_id = ?1)
                     ORDER BY updated_at DESC, session_id DESC",
                )
                .bind(session_filter)
                .fetch_all(&mut *connection)
                .await
                .map_err(storage_error)?;
                let mut sessions = Vec::with_capacity(session_rows.len());
                for row in session_rows {
                    sessions.push(
                        load_session(connection, &SessionId::new(row.get::<String, _>(0)))
                            .await?
                            .ok_or(FactStoreError::CorruptData)?,
                    );
                }

                let request_rows = sqlx::query(
                    "SELECT request_id FROM feedback_requests_v3
                     WHERE resolution IS NULL AND (?1 IS NULL OR session_id = ?1)
                     ORDER BY created_at, request_id",
                )
                .bind(session_filter)
                .fetch_all(&mut *connection)
                .await
                .map_err(storage_error)?;
                let mut waiting_feedback = Vec::with_capacity(request_rows.len());
                for row in request_rows {
                    waiting_feedback.push(
                        load_feedback_request(connection, &RequestId::new(row.get::<String, _>(0)))
                            .await?
                            .ok_or(FactStoreError::CorruptData)?,
                    );
                }

                let draft_rows = sqlx::query(
                    "SELECT draft_id FROM ramble_drafts_v3
                     WHERE (?1 IS NULL OR session_id = ?1)
                     ORDER BY updated_at DESC, draft_id DESC",
                )
                .bind(session_filter)
                .fetch_all(&mut *connection)
                .await
                .map_err(storage_error)?;
                let mut drafts = Vec::with_capacity(draft_rows.len());
                for row in draft_rows {
                    drafts.push(
                        load_draft(connection, &DraftId::new(row.get::<String, _>(0)))
                            .await?
                            .ok_or(FactStoreError::CorruptData)?,
                    );
                }

                let delivery_rows = sqlx::query(
                    "SELECT delivery_id FROM feedback_deliveries_v3
                     WHERE state = 'pending' AND (?1 IS NULL OR session_id = ?1)
                     ORDER BY created_at, delivery_id",
                )
                .bind(session_filter)
                .fetch_all(&mut *connection)
                .await
                .map_err(storage_error)?;
                let mut pending_deliveries = Vec::with_capacity(delivery_rows.len());
                for row in delivery_rows {
                    pending_deliveries.push(
                        load_delivery(connection, &DeliveryId::new(row.get::<String, _>(0)))
                            .await?
                            .ok_or(FactStoreError::CorruptData)?,
                    );
                }

                let work_rows = sqlx::query(
                    "SELECT work_id FROM agent_work_v3
                     WHERE state != 'completed' AND (?1 IS NULL OR session_id = ?1)
                     ORDER BY created_at, work_id",
                )
                .bind(session_filter)
                .fetch_all(&mut *connection)
                .await
                .map_err(storage_error)?;
                let mut pending_agent_work = Vec::with_capacity(work_rows.len());
                for row in work_rows {
                    pending_agent_work.push(
                        load_work(connection, &AgentWorkId::new(row.get::<String, _>(0)))
                            .await?
                            .ok_or(FactStoreError::CorruptData)?,
                    );
                }

                let link_rows = sqlx::query(
                    "SELECT link_id FROM acp_session_links_v3
                     WHERE is_current = 1 AND (?1 IS NULL OR session_id = ?1)
                     ORDER BY last_used_at DESC, link_id DESC",
                )
                .bind(session_filter)
                .fetch_all(&mut *connection)
                .await
                .map_err(storage_error)?;
                let mut current_acp_links = Vec::with_capacity(link_rows.len());
                for row in link_rows {
                    current_acp_links.push(
                        load_link(connection, &AcpSessionLinkId::new(row.get::<String, _>(0)))
                            .await?
                            .ok_or(FactStoreError::CorruptData)?,
                    );
                }

                Ok(FactQueryOutcome::Workbench(WorkbenchSnapshot {
                    sessions,
                    waiting_feedback,
                    drafts,
                    pending_deliveries,
                    pending_agent_work,
                    current_acp_links,
                }))
            }
            FactQuery::SessionRecovery(session_id) => Ok(FactQueryOutcome::SessionRecovery(
                load_session_recovery(connection, &session_id).await?,
            )),
        }?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(outcome)
    }
}

async fn load_session_recovery(
    connection: &mut sqlx::SqliteConnection,
    session_id: &SessionId,
) -> Result<SessionRecoverySnapshot, FactStoreError> {
    let session = load_session(connection, session_id)
        .await?
        .ok_or(FactStoreError::SessionNotFound)?;
    let link_id: Option<String> = sqlx::query_scalar(
        "SELECT link_id FROM acp_session_links_v3 WHERE session_id = ? AND is_current = 1",
    )
    .bind(session_id.as_str())
    .fetch_optional(&mut *connection)
    .await
    .map_err(storage_error)?;
    let current_acp_link = match link_id {
        Some(id) => Some(
            load_link(connection, &AcpSessionLinkId::new(id))
                .await?
                .ok_or(FactStoreError::CorruptData)?,
        ),
        None => None,
    };
    let launch_id: Option<String> = sqlx::query_scalar(
        "SELECT submission_id FROM ramble_submissions_v3
         WHERE session_id = ? AND intent = 'launch'",
    )
    .bind(session_id.as_str())
    .fetch_optional(&mut *connection)
    .await
    .map_err(storage_error)?;
    let launch_submission = match launch_id {
        Some(id) => Some(
            load_submission(connection, &SubmissionId::new(id))
                .await?
                .ok_or(FactStoreError::CorruptData)?,
        ),
        None => None,
    };
    if matches!(
        (session.kind, launch_submission.is_some()),
        (SessionKind::Managed, false) | (SessionKind::Connected, true)
    ) {
        return Err(FactStoreError::CorruptData);
    }
    let steering_rows = sqlx::query(
        "SELECT submission_id FROM ramble_submissions_v3
         WHERE session_id = ? AND intent = 'steering'
         ORDER BY created_at, submission_id",
    )
    .bind(session_id.as_str())
    .fetch_all(&mut *connection)
    .await
    .map_err(storage_error)?;
    let mut steering_submissions = Vec::with_capacity(steering_rows.len());
    for row in steering_rows {
        steering_submissions.push(
            load_submission(
                connection,
                &SubmissionId::new(row.get::<String, _>("submission_id")),
            )
            .await?
            .ok_or(FactStoreError::CorruptData)?,
        );
    }
    let delivery_rows = sqlx::query(
        "SELECT delivery_id FROM feedback_deliveries_v3
         WHERE session_id = ? AND state = 'pending' ORDER BY created_at, delivery_id",
    )
    .bind(session_id.as_str())
    .fetch_all(&mut *connection)
    .await
    .map_err(storage_error)?;
    let mut pending_feedback = Vec::with_capacity(delivery_rows.len());
    for row in delivery_rows {
        let delivery = load_delivery(
            connection,
            &DeliveryId::new(row.get::<String, _>("delivery_id")),
        )
        .await?
        .ok_or(FactStoreError::CorruptData)?;
        let request = load_feedback_request(connection, &delivery.request_id)
            .await?
            .ok_or(FactStoreError::CorruptData)?;
        if request.session_id != *session_id
            || request.status == FeedbackRequestStatus::Waiting
            || request.resolution != Some(delivery.resolution)
        {
            return Err(FactStoreError::CorruptData);
        }
        pending_feedback.push(PendingFeedbackRecovery { request, delivery });
    }
    let work_rows = sqlx::query(
        "SELECT work_id FROM agent_work_v3
         WHERE session_id = ? AND state != 'completed' ORDER BY created_at, work_id",
    )
    .bind(session_id.as_str())
    .fetch_all(&mut *connection)
    .await
    .map_err(storage_error)?;
    let mut pending_agent_work = Vec::with_capacity(work_rows.len());
    for row in work_rows {
        pending_agent_work.push(
            load_work(
                connection,
                &AgentWorkId::new(row.get::<String, _>("work_id")),
            )
            .await?
            .ok_or(FactStoreError::CorruptData)?,
        );
    }
    Ok(SessionRecoverySnapshot {
        session,
        current_acp_link,
        launch_submission,
        steering_submissions,
        pending_feedback,
        pending_agent_work,
    })
}
