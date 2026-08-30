use rambledesk_core::kernel::ports::FactStoreError;
use rambledesk_core::kernel::{
    AgentWorkBatch, AgentWorkDisposition, AgentWorkEvidence, AgentWorkId, AgentWorkKind,
    AgentWorkRecordOutcome, AgentWorkState, ClaimedAgentWork, DeliveryId, StoredWorkResult,
    WorkClaim,
};
use sqlx::Row;

use super::read::{load_work, work_kind_from_label, work_state_from_label};
use super::{SqliteV3Store, storage_error};

impl SqliteV3Store {
    pub(super) async fn claim_work_impl(
        &self,
        claim: WorkClaim,
    ) -> Result<AgentWorkBatch, FactStoreError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let connection = transaction.as_mut();
        let rows = sqlx::query(
            "SELECT w.work_id FROM agent_work_v3 w
             JOIN sessions_v3 s ON s.session_id = w.session_id
             WHERE s.session_kind = 'managed'
               AND (?1 IS NULL OR w.session_id = ?1)
               AND (w.state = 'pending' OR (w.state = 'claimed' AND w.lease_until <= ?2))
             ORDER BY w.created_at, w.work_id LIMIT ?3",
        )
        .bind(claim.scope.session_id.as_ref().map(|id| id.as_str()))
        .bind(&claim.claimed_at)
        .bind(i64::from(claim.scope.limit))
        .fetch_all(&mut *connection)
        .await
        .map_err(storage_error)?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let work_id = AgentWorkId::new(row.get::<String, _>("work_id"));
            let result = sqlx::query(
                "UPDATE agent_work_v3
                 SET state = 'claimed', lease_token = ?, lease_until = ?,
                     attempt_count = attempt_count + 1
                 WHERE work_id = ?
                   AND (state = 'pending' OR (state = 'claimed' AND lease_until <= ?))",
            )
            .bind(claim.claim_token.as_str())
            .bind(&claim.lease_until)
            .bind(work_id.as_str())
            .bind(&claim.claimed_at)
            .execute(&mut *connection)
            .await
            .map_err(storage_error)?;
            if result.rows_affected() != 1 {
                return Err(FactStoreError::WorkClaimConflict);
            }
            let work = load_work(connection, &work_id)
                .await?
                .ok_or(FactStoreError::CorruptData)?;
            if let rambledesk_core::kernel::AgentWorkPayload::FeedbackResume {
                delivery_id, ..
            } = &work.payload
            {
                let delivery_result = sqlx::query(
                    "UPDATE feedback_deliveries_v3
                     SET attempt_count = attempt_count + 1
                     WHERE delivery_id = ? AND state = 'pending'",
                )
                .bind(delivery_id.as_str())
                .execute(&mut *connection)
                .await
                .map_err(storage_error)?;
                if delivery_result.rows_affected() != 1 {
                    return Err(FactStoreError::CorruptData);
                }
            }
            items.push(ClaimedAgentWork {
                work,
                claim_token: claim.claim_token.clone(),
                lease_until: claim.lease_until.clone(),
            });
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(AgentWorkBatch { items })
    }

    pub(super) async fn record_work_impl(
        &self,
        result: StoredWorkResult,
    ) -> Result<AgentWorkRecordOutcome, FactStoreError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let connection = transaction.as_mut();
        let row = sqlx::query(
            "SELECT kind, source_delivery_id, state, lease_token, lease_until
             FROM agent_work_v3 WHERE work_id = ?",
        )
        .bind(result.result.work_id.as_str())
        .fetch_optional(&mut *connection)
        .await
        .map_err(storage_error)?
        .ok_or(FactStoreError::WorkNotFound)?;
        let state = work_state_from_label(&row.get::<String, _>("state"))?;
        let stored_token: Option<String> = row.get("lease_token");
        if stored_token.as_deref() != Some(result.result.claim_token.as_str()) {
            return Err(FactStoreError::WorkClaimConflict);
        }
        let kind = work_kind_from_label(&row.get::<String, _>("kind"))?;
        let source_delivery = row
            .get::<Option<String>, _>("source_delivery_id")
            .map(DeliveryId::new);

        if state == AgentWorkState::Completed {
            let AgentWorkDisposition::Completed { evidence } = &result.result.disposition else {
                return Err(FactStoreError::WorkClaimConflict);
            };
            return Ok(AgentWorkRecordOutcome {
                work_id: result.result.work_id,
                state,
                delivered: validate_evidence(kind, source_delivery.as_ref(), evidence)?,
            });
        }
        if state != AgentWorkState::Claimed {
            return Err(FactStoreError::WorkClaimConflict);
        }
        let lease_until: Option<String> = row.get("lease_until");
        if lease_until
            .as_deref()
            .is_none_or(|lease_until| lease_until <= result.recorded_at.as_str())
        {
            return Err(FactStoreError::WorkClaimConflict);
        }

        let delivered = match &result.result.disposition {
            AgentWorkDisposition::Retry { error_code } => {
                record_retry(
                    connection,
                    &result.result.work_id,
                    result.result.claim_token.as_str(),
                    source_delivery.as_ref(),
                    error_code,
                    &result.recorded_at,
                )
                .await?;
                None
            }
            AgentWorkDisposition::Completed { evidence } => {
                let delivered = validate_evidence(kind, source_delivery.as_ref(), evidence)?;
                record_completion(
                    connection,
                    &result.result.work_id,
                    result.result.claim_token.as_str(),
                    delivered.as_ref(),
                    &result.recorded_at,
                )
                .await?;
                delivered
            }
        };
        transaction.commit().await.map_err(storage_error)?;
        Ok(AgentWorkRecordOutcome {
            work_id: result.result.work_id,
            state: if matches!(
                result.result.disposition,
                AgentWorkDisposition::Retry { .. }
            ) {
                AgentWorkState::Pending
            } else {
                AgentWorkState::Completed
            },
            delivered,
        })
    }
}

async fn record_retry(
    connection: &mut sqlx::SqliteConnection,
    work_id: &AgentWorkId,
    claim_token: &str,
    delivery_id: Option<&DeliveryId>,
    error_code: &str,
    recorded_at: &str,
) -> Result<(), FactStoreError> {
    let result = sqlx::query(
        "UPDATE agent_work_v3
         SET state = 'pending', lease_token = NULL, lease_until = NULL,
             last_error_code = ?, last_error_at = ?
         WHERE work_id = ? AND state = 'claimed' AND lease_token = ?",
    )
    .bind(error_code)
    .bind(recorded_at)
    .bind(work_id.as_str())
    .bind(claim_token)
    .execute(&mut *connection)
    .await
    .map_err(storage_error)?;
    if result.rows_affected() != 1 {
        return Err(FactStoreError::WorkClaimConflict);
    }
    if let Some(delivery_id) = delivery_id {
        let result = sqlx::query(
            "UPDATE feedback_deliveries_v3
             SET last_error_code = ?, last_error_at = ?
             WHERE delivery_id = ? AND state = 'pending'",
        )
        .bind(error_code)
        .bind(recorded_at)
        .bind(delivery_id.as_str())
        .execute(connection)
        .await
        .map_err(storage_error)?;
        if result.rows_affected() != 1 {
            return Err(FactStoreError::CorruptData);
        }
    }
    Ok(())
}

async fn record_completion(
    connection: &mut sqlx::SqliteConnection,
    work_id: &AgentWorkId,
    claim_token: &str,
    delivery_id: Option<&DeliveryId>,
    recorded_at: &str,
) -> Result<(), FactStoreError> {
    if let Some(delivery_id) = delivery_id {
        let result = sqlx::query(
            "UPDATE feedback_deliveries_v3
             SET state = 'delivered', last_error_code = NULL,
                 last_error_at = NULL, delivered_at = ?
             WHERE delivery_id = ? AND state = 'pending'",
        )
        .bind(recorded_at)
        .bind(delivery_id.as_str())
        .execute(&mut *connection)
        .await
        .map_err(storage_error)?;
        if result.rows_affected() != 1 {
            return Err(FactStoreError::WorkClaimConflict);
        }
    }
    let result = sqlx::query(
        "UPDATE agent_work_v3
         SET state = 'completed', lease_until = NULL, last_error_code = NULL,
             last_error_at = NULL, completed_at = ?
         WHERE work_id = ? AND state = 'claimed' AND lease_token = ?",
    )
    .bind(recorded_at)
    .bind(work_id.as_str())
    .bind(claim_token)
    .execute(connection)
    .await
    .map_err(storage_error)?;
    if result.rows_affected() != 1 {
        return Err(FactStoreError::WorkClaimConflict);
    }
    Ok(())
}

fn validate_evidence(
    kind: AgentWorkKind,
    source_delivery: Option<&DeliveryId>,
    evidence: &AgentWorkEvidence,
) -> Result<Option<DeliveryId>, FactStoreError> {
    match (kind, evidence) {
        (
            AgentWorkKind::LaunchPrompt | AgentWorkKind::SteeringPrompt,
            AgentWorkEvidence::PromptTurnCompleted,
        ) => Ok(None),
        (
            AgentWorkKind::FeedbackResume,
            AgentWorkEvidence::FeedbackConsumedAndTurnCompleted { delivery_id },
        ) if source_delivery == Some(delivery_id) => Ok(Some(delivery_id.clone())),
        _ => Err(FactStoreError::WorkClaimConflict),
    }
}
