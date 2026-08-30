use rambledesk_core::kernel::ports::FactStoreError;
use rambledesk_core::kernel::{
    AgentObservation, DraftCommit, FactMutation, FactMutationOutcome, FeedbackRequestStatus,
    FeedbackResolution, FeedbackResolutionCommit, StoredDraftMutation,
};
use sqlx::{Row, SqliteConnection};

use super::read::{
    load_draft, load_feedback_request, load_launch_outcome, load_link, load_resolution_outcome,
    load_steering_outcome, load_submission,
};
use super::write_support::{
    insert_delivery, insert_feedback_request, insert_package, insert_session, insert_submission,
    insert_work, register_artifact,
};
use super::{SqliteV3Store, storage_error, to_json};

impl SqliteV3Store {
    pub(super) async fn apply_mutation(
        &self,
        mutation: FactMutation,
    ) -> Result<FactMutationOutcome, FactStoreError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let connection = transaction.as_mut();
        let outcome = match mutation {
            FactMutation::Launch(commit) => {
                if let Some(stored_digest) = sqlx::query_scalar::<_, String>(
                    "SELECT submission_digest FROM ramble_submissions_v3 WHERE submission_id = ?",
                )
                .bind(commit.submission.submission_id.as_str())
                .fetch_optional(&mut *connection)
                .await
                .map_err(storage_error)?
                {
                    if stored_digest != commit.submission.submission_digest {
                        return Err(FactStoreError::IdempotencyConflict);
                    }
                    FactMutationOutcome::Launch(
                        load_launch_outcome(connection, &commit.submission.submission_id).await?,
                    )
                } else {
                    insert_session(connection, &commit.session).await?;
                    insert_submission(connection, &commit.submission).await?;
                    insert_package(connection, &commit.package).await?;
                    insert_work(connection, &commit.work).await?;
                    FactMutationOutcome::Launch(commit.outcome)
                }
            }
            FactMutation::Steering(commit) => {
                if let Some(stored_digest) = sqlx::query_scalar::<_, String>(
                    "SELECT submission_digest FROM ramble_submissions_v3 WHERE submission_id = ?",
                )
                .bind(commit.submission.submission_id.as_str())
                .fetch_optional(&mut *connection)
                .await
                .map_err(storage_error)?
                {
                    if stored_digest != commit.submission.submission_digest {
                        return Err(FactStoreError::IdempotencyConflict);
                    }
                    FactMutationOutcome::Steering(
                        load_steering_outcome(connection, &commit.submission.submission_id).await?,
                    )
                } else {
                    require_managed_session(connection, commit.submission.session_id.as_str())
                        .await?;
                    insert_submission(connection, &commit.submission).await?;
                    insert_work(connection, &commit.work).await?;
                    FactMutationOutcome::Steering(commit.outcome)
                }
            }
            FactMutation::FeedbackRequest(commit) => {
                if let Some(existing) =
                    load_feedback_request(connection, &commit.request.request_id).await?
                {
                    if existing.input_digest != commit.request.input_digest {
                        return Err(FactStoreError::IdempotencyConflict);
                    }
                    FactMutationOutcome::FeedbackRequest(existing)
                } else {
                    require_session(connection, commit.request.session_id.as_str()).await?;
                    if let Some(link_id) = &commit.request.source_link_id {
                        let exists: bool = sqlx::query_scalar(
                            "SELECT EXISTS(
                                SELECT 1 FROM acp_session_links_v3
                                WHERE session_id = ? AND link_id = ?
                            )",
                        )
                        .bind(commit.request.session_id.as_str())
                        .bind(link_id.as_str())
                        .fetch_one(&mut *connection)
                        .await
                        .map_err(storage_error)?;
                        if !exists {
                            return Err(FactStoreError::AcpSessionLinkNotFound);
                        }
                    }
                    insert_feedback_request(connection, &commit.request).await?;
                    FactMutationOutcome::FeedbackRequest(commit.request)
                }
            }
            FactMutation::FeedbackResolution(commit) => {
                apply_feedback_resolution(connection, *commit).await?
            }
            FactMutation::Draft(commit) => apply_draft(connection, *commit).await?,
            FactMutation::AgentObservation(commit) => {
                let AgentObservation::AcpSessionLinked(observation) = &commit.observation;
                require_managed_session(connection, observation.session_id.as_str()).await?;
                let existing_id: Option<String> = sqlx::query_scalar(
                    "SELECT link_id FROM acp_session_links_v3
                     WHERE session_id = ? AND agent_profile_id = ? AND launch_profile_id = ?
                       AND acp_session_id = ?",
                )
                .bind(observation.session_id.as_str())
                .bind(&observation.agent_profile_id)
                .bind(&observation.launch_profile_id)
                .bind(&observation.acp_session_id)
                .fetch_optional(&mut *connection)
                .await
                .map_err(storage_error)?;
                if let Some(link_id) = existing_id {
                    sqlx::query(
                        "UPDATE acp_session_links_v3 SET is_current = 0 WHERE session_id = ?",
                    )
                    .bind(observation.session_id.as_str())
                    .execute(&mut *connection)
                    .await
                    .map_err(storage_error)?;
                    sqlx::query(
                        "UPDATE acp_session_links_v3
                         SET capabilities_json = ?, session_toolset_digest = ?,
                             is_current = 1, last_used_at = ?
                         WHERE link_id = ?",
                    )
                    .bind(&observation.capabilities_json)
                    .bind(&observation.session_toolset_digest)
                    .bind(&commit.link.last_used_at)
                    .bind(&link_id)
                    .execute(&mut *connection)
                    .await
                    .map_err(storage_error)?;
                    FactMutationOutcome::AgentObservation(
                        load_link(
                            connection,
                            &rambledesk_core::kernel::AcpSessionLinkId::new(link_id),
                        )
                        .await?
                        .ok_or(FactStoreError::CorruptData)?,
                    )
                } else {
                    sqlx::query(
                        "UPDATE acp_session_links_v3 SET is_current = 0 WHERE session_id = ?",
                    )
                    .bind(observation.session_id.as_str())
                    .execute(&mut *connection)
                    .await
                    .map_err(storage_error)?;
                    sqlx::query(
                        "INSERT INTO acp_session_links_v3 (
                            link_id, session_id, agent_profile_id, launch_profile_id, acp_session_id,
                            capabilities_json, session_toolset_digest, is_current, created_at, last_used_at
                         ) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
                    )
                    .bind(commit.link.link_id.as_str())
                    .bind(observation.session_id.as_str())
                    .bind(&observation.agent_profile_id)
                    .bind(&observation.launch_profile_id)
                    .bind(&observation.acp_session_id)
                    .bind(&observation.capabilities_json)
                    .bind(&observation.session_toolset_digest)
                    .bind(&commit.link.created_at)
                    .bind(&commit.link.last_used_at)
                    .execute(&mut *connection)
                    .await
                    .map_err(storage_error)?;
                    FactMutationOutcome::AgentObservation(commit.link)
                }
            }
        };
        transaction.commit().await.map_err(storage_error)?;
        Ok(outcome)
    }
}

async fn apply_feedback_resolution(
    connection: &mut SqliteConnection,
    commit: FeedbackResolutionCommit,
) -> Result<FactMutationOutcome, FactStoreError> {
    let existing = load_feedback_request(connection, &commit.request_id)
        .await?
        .ok_or(FactStoreError::RequestNotFound)?;
    if existing.status != FeedbackRequestStatus::Waiting {
        let same = match commit.resolution {
            FeedbackResolution::Submitted => {
                if let Some(incoming) = commit.submission.as_ref() {
                    let stored_id: Option<String> = sqlx::query_scalar(
                        "SELECT submission_id FROM ramble_submissions_v3
                         WHERE request_id = ? AND intent = 'feedback'",
                    )
                    .bind(commit.request_id.as_str())
                    .fetch_optional(&mut *connection)
                    .await
                    .map_err(storage_error)?;
                    match stored_id {
                        Some(id) => load_submission(
                            connection,
                            &rambledesk_core::kernel::SubmissionId::new(id),
                        )
                        .await?
                        .is_some_and(|stored| {
                            stored.submission_id == incoming.submission_id
                                && stored.submission_digest == incoming.submission_digest
                        }),
                        None => false,
                    }
                } else {
                    false
                }
            }
            FeedbackResolution::Cancelled => {
                existing.resolution == Some(FeedbackResolution::Cancelled)
                    && existing.cancel_reason == commit.cancel_reason
            }
        };
        if !same {
            return Err(
                if commit.resolution == FeedbackResolution::Submitted
                    && existing.resolution == Some(FeedbackResolution::Submitted)
                {
                    FactStoreError::IdempotencyConflict
                } else {
                    FactStoreError::RequestTerminal
                },
            );
        }
        return Ok(FactMutationOutcome::FeedbackResolution(
            load_resolution_outcome(connection, &commit.request_id).await?,
        ));
    }

    require_managed_session(connection, existing.session_id.as_str()).await?;

    if let Some(expected) = commit.expected_draft_revision {
        let stored_revision: Option<i64> =
            sqlx::query_scalar("SELECT revision FROM ramble_drafts_v3 WHERE request_id = ?")
                .bind(commit.request_id.as_str())
                .fetch_optional(&mut *connection)
                .await
                .map_err(storage_error)?;
        let stored = stored_revision.unwrap_or(0);
        if u64::try_from(stored).ok() != Some(expected) {
            return Err(FactStoreError::DraftConflict);
        }
    }

    match commit.resolution {
        FeedbackResolution::Submitted => {
            let submission = commit
                .submission
                .as_ref()
                .ok_or(FactStoreError::CorruptData)?;
            let package = commit.package.as_ref().ok_or(FactStoreError::CorruptData)?;
            if commit.cancel_reason.is_some() {
                return Err(FactStoreError::CorruptData);
            }
            insert_submission(connection, submission).await?;
            insert_package(connection, package).await?;
        }
        FeedbackResolution::Cancelled => {
            if commit.submission.is_some()
                || commit.package.is_some()
                || commit.cancel_reason.is_none()
            {
                return Err(FactStoreError::CorruptData);
            }
        }
    }
    let package_id = commit
        .package
        .as_ref()
        .map(|value| value.package_id.as_str());
    sqlx::query(
        "UPDATE feedback_requests_v3
         SET resolution = ?, response_package_id = ?, cancel_reason = ?, resolved_at = ?, updated_at = ?
         WHERE request_id = ? AND resolution IS NULL",
    )
    .bind(super::read::resolution_label(commit.resolution))
    .bind(package_id)
    .bind(commit.cancel_reason.as_deref())
    .bind(commit.outcome.request.resolved_at.as_deref())
    .bind(
        commit
            .outcome
            .request
            .resolved_at
            .as_deref()
            .ok_or(FactStoreError::CorruptData)?,
    )
    .bind(commit.request_id.as_str())
    .execute(&mut *connection)
    .await
    .map_err(storage_error)?;
    insert_delivery(connection, &commit.delivery).await?;
    insert_work(connection, &commit.work).await?;
    sqlx::query("DELETE FROM ramble_drafts_v3 WHERE request_id = ?")
        .bind(commit.request_id.as_str())
        .execute(&mut *connection)
        .await
        .map_err(storage_error)?;
    Ok(FactMutationOutcome::FeedbackResolution(commit.outcome))
}

async fn apply_draft(
    connection: &mut SqliteConnection,
    commit: DraftCommit,
) -> Result<FactMutationOutcome, FactStoreError> {
    let draft_id = match &commit.mutation {
        StoredDraftMutation::Save(value) => value.draft_id.clone(),
        StoredDraftMutation::AddArtifact { draft_id, .. } => draft_id.clone(),
        StoredDraftMutation::RemoveArtifact(value) => value.draft_id.clone(),
        StoredDraftMutation::ReorderArtifacts(value) => value.draft_id.clone(),
    };
    let existing = load_draft(connection, &draft_id).await?;
    match commit.mutation {
        StoredDraftMutation::Save(value) => {
            let revision = existing.as_ref().map_or(0, |value| value.revision);
            if revision != value.expected_revision {
                return Err(FactStoreError::DraftConflict);
            }
            if existing.as_ref().is_some_and(|stored| {
                stored.intent != value.intent
                    || stored.session_id != value.session_id
                    || stored.request_id != value.request_id
            }) {
                return Err(FactStoreError::DraftConflict);
            }
            let launch_json = value
                .launch_configuration
                .as_ref()
                .map(to_json)
                .transpose()?;
            if existing.is_some() {
                sqlx::query(
                    "UPDATE ramble_drafts_v3 SET
                        launch_configuration_json = ?, document_json = ?, body_markdown = ?,
                        revision = revision + 1, updated_at = ? WHERE draft_id = ? AND revision = ?",
                )
                .bind(launch_json)
                .bind(&value.document_json)
                .bind(&value.body_markdown)
                .bind(&commit.now)
                .bind(value.draft_id.as_str())
                .bind(i64::try_from(value.expected_revision).map_err(|_| FactStoreError::Storage)?)
                .execute(&mut *connection)
                .await
                .map_err(storage_error)?;
            } else {
                sqlx::query(
                    "INSERT INTO ramble_drafts_v3 (
                        draft_id, intent, session_id, request_id, launch_configuration_json,
                        document_json, body_markdown, revision, created_at, updated_at
                     ) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
                )
                .bind(value.draft_id.as_str())
                .bind(super::read::ramble_intent_label(value.intent))
                .bind(value.session_id.as_ref().map(|id| id.as_str()))
                .bind(value.request_id.as_ref().map(|id| id.as_str()))
                .bind(launch_json)
                .bind(&value.document_json)
                .bind(&value.body_markdown)
                .bind(&commit.now)
                .bind(&commit.now)
                .execute(&mut *connection)
                .await
                .map_err(storage_error)?;
            }
        }
        StoredDraftMutation::AddArtifact {
            draft_id,
            expected_revision,
            artifact,
        } => {
            require_draft_revision(existing.as_ref(), expected_revision)?;
            register_artifact(
                connection,
                &artifact.storage_key,
                &artifact.sha256,
                artifact.size_bytes,
                &commit.now,
            )
            .await?;
            let position: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM draft_artifacts_v3 WHERE draft_id = ?")
                    .bind(draft_id.as_str())
                    .fetch_one(&mut *connection)
                    .await
                    .map_err(storage_error)?;
            sqlx::query(
                "INSERT INTO draft_artifacts_v3 (
                    draft_id, artifact_id, position, display_name, media_type,
                    size_bytes, sha256, storage_key
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(draft_id.as_str())
            .bind(artifact.artifact_id.as_str())
            .bind(position)
            .bind(&artifact.display_name)
            .bind(&artifact.media_type)
            .bind(i64::try_from(artifact.size_bytes).map_err(|_| FactStoreError::Storage)?)
            .bind(&artifact.sha256)
            .bind(&artifact.storage_key)
            .execute(&mut *connection)
            .await
            .map_err(storage_error)?;
            bump_draft(connection, &draft_id, expected_revision, &commit.now).await?;
        }
        StoredDraftMutation::RemoveArtifact(value) => {
            require_draft_revision(existing.as_ref(), value.expected_revision)?;
            let result = sqlx::query(
                "DELETE FROM draft_artifacts_v3 WHERE draft_id = ? AND artifact_id = ?",
            )
            .bind(value.draft_id.as_str())
            .bind(value.artifact_id.as_str())
            .execute(&mut *connection)
            .await
            .map_err(storage_error)?;
            if result.rows_affected() != 1 {
                return Err(FactStoreError::DraftConflict);
            }
            compact_draft_positions(connection, &value.draft_id).await?;
            bump_draft(
                connection,
                &value.draft_id,
                value.expected_revision,
                &commit.now,
            )
            .await?;
        }
        StoredDraftMutation::ReorderArtifacts(value) => {
            let draft = existing.as_ref().ok_or(FactStoreError::DraftConflict)?;
            require_draft_revision(Some(draft), value.expected_revision)?;
            if draft.artifacts.len() != value.artifact_ids.len()
                || value.artifact_ids.iter().any(|id| {
                    !draft
                        .artifacts
                        .iter()
                        .any(|artifact| &artifact.artifact_id == id)
                })
            {
                return Err(FactStoreError::DraftConflict);
            }
            sqlx::query(
                "UPDATE draft_artifacts_v3 SET position = position + 100000 WHERE draft_id = ?",
            )
            .bind(value.draft_id.as_str())
            .execute(&mut *connection)
            .await
            .map_err(storage_error)?;
            for (position, artifact_id) in value.artifact_ids.iter().enumerate() {
                sqlx::query(
                    "UPDATE draft_artifacts_v3 SET position = ? WHERE draft_id = ? AND artifact_id = ?",
                )
                .bind(i64::try_from(position).map_err(|_| FactStoreError::Storage)?)
                .bind(value.draft_id.as_str())
                .bind(artifact_id.as_str())
                .execute(&mut *connection)
                .await
                .map_err(storage_error)?;
            }
            bump_draft(
                connection,
                &value.draft_id,
                value.expected_revision,
                &commit.now,
            )
            .await?;
        }
    }
    Ok(FactMutationOutcome::Draft(
        load_draft(connection, &draft_id)
            .await?
            .ok_or(FactStoreError::CorruptData)?,
    ))
}

fn require_draft_revision(
    draft: Option<&rambledesk_core::kernel::DraftSnapshot>,
    expected: u64,
) -> Result<(), FactStoreError> {
    if draft.is_none_or(|value| value.revision != expected) {
        return Err(FactStoreError::DraftConflict);
    }
    Ok(())
}

async fn bump_draft(
    connection: &mut SqliteConnection,
    draft_id: &rambledesk_core::kernel::DraftId,
    expected: u64,
    now: &str,
) -> Result<(), FactStoreError> {
    let result = sqlx::query(
        "UPDATE ramble_drafts_v3 SET revision = revision + 1, updated_at = ?
         WHERE draft_id = ? AND revision = ?",
    )
    .bind(now)
    .bind(draft_id.as_str())
    .bind(i64::try_from(expected).map_err(|_| FactStoreError::Storage)?)
    .execute(connection)
    .await
    .map_err(storage_error)?;
    if result.rows_affected() != 1 {
        return Err(FactStoreError::DraftConflict);
    }
    Ok(())
}

async fn compact_draft_positions(
    connection: &mut SqliteConnection,
    draft_id: &rambledesk_core::kernel::DraftId,
) -> Result<(), FactStoreError> {
    let rows = sqlx::query(
        "SELECT artifact_id FROM draft_artifacts_v3 WHERE draft_id = ? ORDER BY position",
    )
    .bind(draft_id.as_str())
    .fetch_all(&mut *connection)
    .await
    .map_err(storage_error)?;
    sqlx::query("UPDATE draft_artifacts_v3 SET position = position + 100000 WHERE draft_id = ?")
        .bind(draft_id.as_str())
        .execute(&mut *connection)
        .await
        .map_err(storage_error)?;
    for (position, row) in rows.into_iter().enumerate() {
        sqlx::query(
            "UPDATE draft_artifacts_v3 SET position = ? WHERE draft_id = ? AND artifact_id = ?",
        )
        .bind(i64::try_from(position).map_err(|_| FactStoreError::Storage)?)
        .bind(draft_id.as_str())
        .bind(row.get::<String, _>("artifact_id"))
        .execute(&mut *connection)
        .await
        .map_err(storage_error)?;
    }
    Ok(())
}

async fn require_session(
    connection: &mut SqliteConnection,
    session_id: &str,
) -> Result<(), FactStoreError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sessions_v3 WHERE session_id = ?)")
            .bind(session_id)
            .fetch_one(connection)
            .await
            .map_err(storage_error)?;
    if !exists {
        return Err(FactStoreError::SessionNotFound);
    }
    Ok(())
}

async fn require_managed_session(
    connection: &mut SqliteConnection,
    session_id: &str,
) -> Result<(), FactStoreError> {
    let kind: Option<String> =
        sqlx::query_scalar("SELECT session_kind FROM sessions_v3 WHERE session_id = ?")
            .bind(session_id)
            .fetch_optional(connection)
            .await
            .map_err(storage_error)?;
    match kind.as_deref() {
        Some("managed") => Ok(()),
        Some("connected") => Err(FactStoreError::SessionNotManaged),
        Some(_) => Err(FactStoreError::CorruptData),
        None => Err(FactStoreError::SessionNotFound),
    }
}
