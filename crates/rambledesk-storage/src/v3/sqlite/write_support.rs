use rambledesk_core::kernel::ports::FactStoreError;
use rambledesk_core::kernel::{
    AgentWorkKind, AgentWorkPayload, AgentWorkRecord, DeliveryState, FeedbackDeliveryRecord,
    FeedbackRequestSnapshot, PackageRecord, RambleSubmissionRecord, SessionKind, SessionLifecycle,
    SessionRecord, package_digests_match,
};
use sqlx::{Row, SqliteConnection};

use super::manifest::{build_manifest, purpose_label};
use super::read::{ramble_intent_label, resolution_label, work_kind_label};
use super::{storage_error, to_json};

pub(super) async fn insert_session(
    connection: &mut SqliteConnection,
    session: &SessionRecord,
) -> Result<(), FactStoreError> {
    let launch_json = session
        .launch_configuration
        .as_ref()
        .map(to_json)
        .transpose()?;
    sqlx::query(
        "INSERT INTO sessions_v3 (
            session_id, session_kind, title, lifecycle, launch_configuration_json,
            created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(session.session_id.as_str())
    .bind(match session.kind {
        SessionKind::Managed => "managed",
        SessionKind::Connected => "connected",
    })
    .bind(&session.title)
    .bind(match session.lifecycle {
        SessionLifecycle::Ready => "ready",
        SessionLifecycle::Stopped => "stopped",
        SessionLifecycle::Failed => "failed",
    })
    .bind(launch_json)
    .bind(&session.created_at)
    .bind(&session.updated_at)
    .execute(connection)
    .await
    .map_err(storage_error)?;
    Ok(())
}

pub(super) async fn insert_feedback_request(
    connection: &mut SqliteConnection,
    request: &FeedbackRequestSnapshot,
) -> Result<(), FactStoreError> {
    if request.status != rambledesk_core::kernel::FeedbackRequestStatus::Waiting
        || request.resolution.is_some()
        || request.response_package_id.is_some()
        || request.cancel_reason.is_some()
        || request.resolved_at.is_some()
    {
        return Err(FactStoreError::CorruptData);
    }
    for (position, action) in request.actions.iter().enumerate() {
        sqlx::query(
            "INSERT INTO feedback_request_actions_v3
             (request_id, action_id, position, instruction) VALUES (?, ?, ?, ?)",
        )
        .bind(request.request_id.as_str())
        .bind(&action.id)
        .bind(i64::try_from(position).map_err(|_| FactStoreError::Storage)?)
        .bind(&action.instruction)
        .execute(&mut *connection)
        .await
        .map_err(storage_error)?;
    }
    for (position, reference) in request.context_refs.iter().enumerate() {
        sqlx::query(
            "INSERT INTO feedback_request_context_refs_v3
             (request_id, position, label, uri) VALUES (?, ?, ?, ?)",
        )
        .bind(request.request_id.as_str())
        .bind(i64::try_from(position).map_err(|_| FactStoreError::Storage)?)
        .bind(&reference.label)
        .bind(&reference.uri)
        .execute(&mut *connection)
        .await
        .map_err(storage_error)?;
    }
    for artifact in &request.request_artifacts {
        register_artifact(
            connection,
            &artifact.storage_key,
            &artifact.sha256,
            artifact.size_bytes,
            &request.created_at,
        )
        .await?;
        sqlx::query(
            "INSERT INTO feedback_request_artifacts_v3 (
                request_id, artifact_id, position, display_name, media_type,
                size_bytes, sha256, storage_key
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(request.request_id.as_str())
        .bind(artifact.artifact_id.as_str())
        .bind(i64::from(artifact.position))
        .bind(&artifact.display_name)
        .bind(&artifact.media_type)
        .bind(i64::try_from(artifact.size_bytes).map_err(|_| FactStoreError::Storage)?)
        .bind(&artifact.sha256)
        .bind(&artifact.storage_key)
        .execute(&mut *connection)
        .await
        .map_err(storage_error)?;
    }
    sqlx::query(
        "INSERT INTO feedback_requests_v3 (
            request_id, session_id, source_link_id, title, instructions, input_digest,
            resolution, response_package_id, cancel_reason, created_at, resolved_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, NULL, ?, NULL, ?)",
    )
    .bind(request.request_id.as_str())
    .bind(request.session_id.as_str())
    .bind(request.source_link_id.as_ref().map(|id| id.as_str()))
    .bind(&request.title)
    .bind(&request.instructions)
    .bind(&request.input_digest)
    .bind(&request.created_at)
    .bind(&request.created_at)
    .execute(&mut *connection)
    .await
    .map_err(storage_error)?;
    Ok(())
}

pub(super) async fn insert_submission(
    connection: &mut SqliteConnection,
    submission: &RambleSubmissionRecord,
) -> Result<(), FactStoreError> {
    for artifact in &submission.artifacts {
        register_artifact(
            connection,
            &artifact.storage_key,
            &artifact.sha256,
            artifact.size_bytes,
            &submission.created_at,
        )
        .await?;
        sqlx::query(
            "INSERT INTO submission_artifacts_v3 (
                submission_id, artifact_id, position, display_name, media_type,
                size_bytes, sha256, storage_key
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(submission.submission_id.as_str())
        .bind(artifact.artifact_id.as_str())
        .bind(i64::from(artifact.position))
        .bind(&artifact.display_name)
        .bind(&artifact.media_type)
        .bind(i64::try_from(artifact.size_bytes).map_err(|_| FactStoreError::Storage)?)
        .bind(&artifact.sha256)
        .bind(&artifact.storage_key)
        .execute(&mut *connection)
        .await
        .map_err(storage_error)?;
    }
    sqlx::query(
        "INSERT INTO ramble_submissions_v3 (
            submission_id, session_id, intent, request_id, document_json,
            body_markdown, submission_digest, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(submission.submission_id.as_str())
    .bind(submission.session_id.as_str())
    .bind(ramble_intent_label(submission.intent))
    .bind(submission.request_id.as_ref().map(|id| id.as_str()))
    .bind(&submission.document_json)
    .bind(&submission.body_markdown)
    .bind(&submission.submission_digest)
    .bind(&submission.created_at)
    .execute(&mut *connection)
    .await
    .map_err(storage_error)?;
    Ok(())
}

pub(super) async fn insert_package(
    connection: &mut SqliteConnection,
    package: &PackageRecord,
) -> Result<(), FactStoreError> {
    if !package_digests_match(package) {
        return Err(FactStoreError::CorruptData);
    }
    let manifest_json = build_manifest(package)?;
    for artifact in &package.artifacts {
        register_artifact(
            connection,
            &artifact.storage_key,
            &artifact.sha256,
            artifact.size_bytes,
            &package.published_at,
        )
        .await?;
        sqlx::query(
            "INSERT INTO package_artifacts_v3 (
                package_id, artifact_id, position, role, display_name, media_type,
                size_bytes, sha256, storage_key
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(package.package_id.as_str())
        .bind(artifact.artifact_id.as_str())
        .bind(i64::from(artifact.position))
        .bind(artifact.role.digest_label())
        .bind(&artifact.display_name)
        .bind(&artifact.media_type)
        .bind(i64::try_from(artifact.size_bytes).map_err(|_| FactStoreError::Storage)?)
        .bind(&artifact.sha256)
        .bind(&artifact.storage_key)
        .execute(&mut *connection)
        .await
        .map_err(storage_error)?;
    }
    sqlx::query(
        "INSERT INTO packages_v3 (
            package_id, submission_id, package_purpose, request_id, schema_version,
            manifest_json, content_digest, manifest_digest, published_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(package.package_id.as_str())
    .bind(package.submission_id.as_str())
    .bind(purpose_label(package.purpose))
    .bind(package.request_id.as_ref().map(|id| id.as_str()))
    .bind(i64::from(package.schema_version))
    .bind(manifest_json)
    .bind(&package.content_digest)
    .bind(&package.manifest_digest)
    .bind(&package.published_at)
    .execute(&mut *connection)
    .await
    .map_err(storage_error)?;
    Ok(())
}

pub(super) async fn insert_delivery(
    connection: &mut SqliteConnection,
    delivery: &FeedbackDeliveryRecord,
) -> Result<(), FactStoreError> {
    let package_id = delivery
        .package
        .as_ref()
        .map(|value| value.package_id.as_str());
    sqlx::query(
        "INSERT INTO feedback_deliveries_v3 (
            delivery_id, request_id, session_id, resolution, package_id, state,
            attempt_count, last_error_code, last_error_at, created_at, delivered_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)",
    )
    .bind(delivery.delivery_id.as_str())
    .bind(delivery.request_id.as_str())
    .bind(delivery.session_id.as_str())
    .bind(resolution_label(delivery.resolution))
    .bind(package_id)
    .bind(match delivery.state {
        DeliveryState::Pending => "pending",
        DeliveryState::Delivered => "delivered",
    })
    .bind(i64::from(delivery.attempt_count))
    .bind(delivery.last_error_code.as_deref())
    .bind(&delivery.created_at)
    .bind(delivery.delivered_at.as_deref())
    .execute(connection)
    .await
    .map_err(storage_error)?;
    Ok(())
}

pub(super) async fn insert_work(
    connection: &mut SqliteConnection,
    work: &AgentWorkRecord,
) -> Result<(), FactStoreError> {
    if work.state != rambledesk_core::kernel::AgentWorkState::Pending
        || work.attempt_count != 0
        || work.last_error_code.is_some()
        || work.last_error_at.is_some()
        || work.completed_at.is_some()
    {
        return Err(FactStoreError::CorruptData);
    }
    let (submission_id, delivery_id) = match (&work.kind, &work.payload) {
        (AgentWorkKind::LaunchPrompt, AgentWorkPayload::Launch { submission_id, .. })
        | (AgentWorkKind::SteeringPrompt, AgentWorkPayload::Steering { submission_id, .. }) => {
            if work.source_id != submission_id.as_str() {
                return Err(FactStoreError::CorruptData);
            }
            (Some(submission_id.as_str()), None)
        }
        (AgentWorkKind::FeedbackResume, AgentWorkPayload::FeedbackResume { delivery_id, .. }) => {
            if work.source_id != delivery_id.as_str() {
                return Err(FactStoreError::CorruptData);
            }
            (None, Some(delivery_id.as_str()))
        }
        _ => return Err(FactStoreError::CorruptData),
    };
    sqlx::query(
        "INSERT INTO agent_work_v3 (
            work_id, session_id, kind, source_submission_id, source_delivery_id,
            payload_digest, state, lease_token, lease_until, attempt_count,
            last_error_code, last_error_at, created_at, completed_at
         ) VALUES (?, ?, ?, ?, ?, ?, 'pending', NULL, NULL, 0, NULL, NULL, ?, NULL)",
    )
    .bind(work.work_id.as_str())
    .bind(work.session_id.as_str())
    .bind(work_kind_label(work.kind))
    .bind(submission_id)
    .bind(delivery_id)
    .bind(&work.payload_digest)
    .bind(&work.created_at)
    .execute(connection)
    .await
    .map_err(storage_error)?;
    Ok(())
}

pub(super) async fn register_artifact(
    connection: &mut SqliteConnection,
    storage_key: &str,
    sha256: &str,
    size_bytes: u64,
    created_at: &str,
) -> Result<(), FactStoreError> {
    sqlx::query(
        "INSERT INTO artifact_objects_v3 (storage_key, sha256, size_bytes, created_at)
         VALUES (?, ?, ?, ?) ON CONFLICT(storage_key) DO NOTHING",
    )
    .bind(storage_key)
    .bind(sha256)
    .bind(i64::try_from(size_bytes).map_err(|_| FactStoreError::Storage)?)
    .bind(created_at)
    .execute(&mut *connection)
    .await
    .map_err(storage_error)?;
    let row =
        sqlx::query("SELECT sha256, size_bytes FROM artifact_objects_v3 WHERE storage_key = ?")
            .bind(storage_key)
            .fetch_one(connection)
            .await
            .map_err(storage_error)?;
    let stored_size: i64 = row.get("size_bytes");
    if row.get::<String, _>("sha256") != sha256
        || u64::try_from(stored_size).ok() != Some(size_bytes)
    {
        return Err(FactStoreError::IdempotencyConflict);
    }
    Ok(())
}
