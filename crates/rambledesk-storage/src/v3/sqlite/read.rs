use rambledesk_core::kernel::ports::FactStoreError;
use rambledesk_core::kernel::{
    AcpSessionLinkId, AcpSessionLinkSnapshot, AgentWorkId, AgentWorkKind, AgentWorkPayload,
    AgentWorkRecord, AgentWorkState, DeliveryId, DeliveryState, DraftArtifact, DraftId,
    DraftSnapshot, FeedbackDeliveryRecord, FeedbackRequestSnapshot, FeedbackRequestStatus,
    FeedbackResolution, FeedbackResolutionOutcome, LaunchOutcome, PackageArtifact, PackageId,
    PackagePurpose, PackageRecord, RambleIntent, RequestArtifact, RequestId, SessionId,
    SessionKind, SessionLifecycle, SessionRecord, SteeringOutcome, SubmissionArtifact,
    SubmissionId, package_digests_match,
};
use sqlx::{Row, SqliteConnection, sqlite::SqliteRow};

use super::manifest::{build_manifest, role_from_label};
use super::{checked_u32, checked_u64, parse_json, required, storage_error};

pub(super) async fn load_session(
    connection: &mut SqliteConnection,
    session_id: &SessionId,
) -> Result<Option<SessionRecord>, FactStoreError> {
    let row = sqlx::query(
        "SELECT session_kind, title, lifecycle, launch_configuration_json, created_at, updated_at
         FROM sessions_v3 WHERE session_id = ?",
    )
    .bind(session_id.as_str())
    .fetch_optional(connection)
    .await
    .map_err(storage_error)?;
    row.map(|row| session_from_row(session_id.clone(), &row))
        .transpose()
}

fn session_from_row(
    session_id: SessionId,
    row: &SqliteRow,
) -> Result<SessionRecord, FactStoreError> {
    let kind = match row.get::<String, _>("session_kind").as_str() {
        "managed" => SessionKind::Managed,
        "connected" => SessionKind::Connected,
        _ => return Err(FactStoreError::CorruptData),
    };
    let lifecycle = match row.get::<String, _>("lifecycle").as_str() {
        "ready" => SessionLifecycle::Ready,
        "stopped" => SessionLifecycle::Stopped,
        "failed" => SessionLifecycle::Failed,
        _ => return Err(FactStoreError::CorruptData),
    };
    let launch_json: Option<String> = row.get("launch_configuration_json");
    Ok(SessionRecord {
        session_id,
        kind,
        title: row.get("title"),
        lifecycle,
        launch_configuration: launch_json.as_deref().map(parse_json).transpose()?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) async fn load_feedback_request(
    connection: &mut SqliteConnection,
    request_id: &RequestId,
) -> Result<Option<FeedbackRequestSnapshot>, FactStoreError> {
    let Some(row) = sqlx::query(
        "SELECT session_id, source_link_id, title, instructions, input_digest, resolution,
                response_package_id, cancel_reason, created_at, resolved_at
         FROM feedback_requests_v3 WHERE request_id = ?",
    )
    .bind(request_id.as_str())
    .fetch_optional(&mut *connection)
    .await
    .map_err(storage_error)?
    else {
        return Ok(None);
    };
    let resolution_text: Option<String> = row.get("resolution");
    let (status, resolution) = match resolution_text.as_deref() {
        None => (FeedbackRequestStatus::Waiting, None),
        Some("submitted") => (
            FeedbackRequestStatus::Submitted,
            Some(FeedbackResolution::Submitted),
        ),
        Some("cancelled") => (
            FeedbackRequestStatus::Cancelled,
            Some(FeedbackResolution::Cancelled),
        ),
        _ => return Err(FactStoreError::CorruptData),
    };
    let action_rows = sqlx::query(
        "SELECT action_id, instruction FROM feedback_request_actions_v3
         WHERE request_id = ? ORDER BY position",
    )
    .bind(request_id.as_str())
    .fetch_all(&mut *connection)
    .await
    .map_err(storage_error)?;
    let context_rows = sqlx::query(
        "SELECT label, uri FROM feedback_request_context_refs_v3
         WHERE request_id = ? ORDER BY position",
    )
    .bind(request_id.as_str())
    .fetch_all(&mut *connection)
    .await
    .map_err(storage_error)?;
    let request_artifacts = load_request_artifacts(connection, request_id).await?;
    Ok(Some(FeedbackRequestSnapshot {
        request_id: request_id.clone(),
        session_id: SessionId::new(row.get::<String, _>("session_id")),
        source_link_id: row
            .get::<Option<String>, _>("source_link_id")
            .map(AcpSessionLinkId::new),
        title: row.get("title"),
        instructions: row.get("instructions"),
        actions: action_rows
            .into_iter()
            .map(|row| rambledesk_core::kernel::FeedbackAction {
                id: row.get("action_id"),
                instruction: row.get("instruction"),
            })
            .collect(),
        context_refs: context_rows
            .into_iter()
            .map(|row| rambledesk_core::kernel::ContextReference {
                label: row.get("label"),
                uri: row.get("uri"),
            })
            .collect(),
        input_digest: row.get("input_digest"),
        status,
        resolution,
        response_package_id: row
            .get::<Option<String>, _>("response_package_id")
            .map(PackageId::new),
        cancel_reason: row.get("cancel_reason"),
        request_artifacts,
        created_at: row.get("created_at"),
        resolved_at: row.get("resolved_at"),
    }))
}

async fn load_request_artifacts(
    connection: &mut SqliteConnection,
    request_id: &RequestId,
) -> Result<Vec<RequestArtifact>, FactStoreError> {
    let rows = sqlx::query(
        "SELECT artifact_id, position, display_name, media_type, size_bytes, sha256, storage_key
         FROM feedback_request_artifacts_v3 WHERE request_id = ? ORDER BY position",
    )
    .bind(request_id.as_str())
    .fetch_all(connection)
    .await
    .map_err(storage_error)?;
    rows.into_iter().map(request_artifact_from_row).collect()
}

fn request_artifact_from_row(row: SqliteRow) -> Result<RequestArtifact, FactStoreError> {
    Ok(RequestArtifact {
        artifact_id: rambledesk_core::kernel::ArtifactId::new(row.get::<String, _>("artifact_id")),
        position: checked_u32(row.get("position"))?,
        display_name: row.get("display_name"),
        media_type: row.get("media_type"),
        size_bytes: checked_u64(row.get("size_bytes"))?,
        sha256: row.get("sha256"),
        storage_key: row.get("storage_key"),
    })
}

pub(super) async fn load_submission(
    connection: &mut SqliteConnection,
    submission_id: &SubmissionId,
) -> Result<Option<rambledesk_core::kernel::RambleSubmissionRecord>, FactStoreError> {
    let Some(row) = sqlx::query(
        "SELECT session_id, intent, request_id, document_json, body_markdown,
                submission_digest, created_at
         FROM ramble_submissions_v3 WHERE submission_id = ?",
    )
    .bind(submission_id.as_str())
    .fetch_optional(&mut *connection)
    .await
    .map_err(storage_error)?
    else {
        return Ok(None);
    };
    let intent = ramble_intent_from_label(&row.get::<String, _>("intent"))?;
    let artifact_rows = sqlx::query(
        "SELECT artifact_id, position, display_name, media_type, size_bytes, sha256, storage_key
         FROM submission_artifacts_v3 WHERE submission_id = ? ORDER BY position",
    )
    .bind(submission_id.as_str())
    .fetch_all(&mut *connection)
    .await
    .map_err(storage_error)?;
    let artifacts = artifact_rows
        .into_iter()
        .map(|row| {
            Ok(SubmissionArtifact {
                artifact_id: rambledesk_core::kernel::ArtifactId::new(
                    row.get::<String, _>("artifact_id"),
                ),
                position: checked_u32(row.get("position"))?,
                display_name: row.get("display_name"),
                media_type: row.get("media_type"),
                size_bytes: checked_u64(row.get("size_bytes"))?,
                sha256: row.get("sha256"),
                storage_key: row.get("storage_key"),
            })
        })
        .collect::<Result<Vec<_>, FactStoreError>>()?;
    Ok(Some(rambledesk_core::kernel::RambleSubmissionRecord {
        submission_id: submission_id.clone(),
        session_id: SessionId::new(row.get::<String, _>("session_id")),
        intent,
        request_id: row
            .get::<Option<String>, _>("request_id")
            .map(RequestId::new),
        document_json: row.get("document_json"),
        body_markdown: row.get("body_markdown"),
        submission_digest: row.get("submission_digest"),
        artifacts,
        created_at: row.get("created_at"),
    }))
}

pub(super) async fn load_package(
    connection: &mut SqliteConnection,
    package_id: &PackageId,
) -> Result<Option<PackageRecord>, FactStoreError> {
    let Some(row) = sqlx::query(
        "SELECT submission_id, package_purpose, request_id, schema_version, manifest_json,
                content_digest, manifest_digest, published_at
         FROM packages_v3 WHERE package_id = ?",
    )
    .bind(package_id.as_str())
    .fetch_optional(&mut *connection)
    .await
    .map_err(storage_error)?
    else {
        return Ok(None);
    };
    let purpose = match row.get::<String, _>("package_purpose").as_str() {
        "launch" => PackagePurpose::Launch,
        "response" => PackagePurpose::Response,
        _ => return Err(FactStoreError::CorruptData),
    };
    let artifact_rows = sqlx::query(
        "SELECT artifact_id, role, position, display_name, media_type, size_bytes, sha256, storage_key
         FROM package_artifacts_v3 WHERE package_id = ? ORDER BY position",
    )
    .bind(package_id.as_str())
    .fetch_all(&mut *connection)
    .await
    .map_err(storage_error)?;
    let artifacts = artifact_rows
        .into_iter()
        .map(|row| {
            Ok(PackageArtifact {
                artifact_id: rambledesk_core::kernel::ArtifactId::new(
                    row.get::<String, _>("artifact_id"),
                ),
                role: role_from_label(row.get("role")),
                position: checked_u32(row.get("position"))?,
                display_name: row.get("display_name"),
                media_type: row.get("media_type"),
                size_bytes: checked_u64(row.get("size_bytes"))?,
                sha256: row.get("sha256"),
                storage_key: row.get("storage_key"),
            })
        })
        .collect::<Result<Vec<_>, FactStoreError>>()?;
    let package = PackageRecord {
        package_id: package_id.clone(),
        submission_id: SubmissionId::new(row.get::<String, _>("submission_id")),
        purpose,
        request_id: row
            .get::<Option<String>, _>("request_id")
            .map(RequestId::new),
        content_digest: row.get("content_digest"),
        manifest_digest: row.get("manifest_digest"),
        schema_version: checked_u32(row.get("schema_version"))?,
        artifacts,
        published_at: row.get("published_at"),
    };
    let stored_manifest: String = row.get("manifest_json");
    if build_manifest(&package)? != stored_manifest {
        return Err(FactStoreError::CorruptData);
    }
    if !package_digests_match(&package) {
        return Err(FactStoreError::CorruptData);
    }
    Ok(Some(package))
}

pub(super) async fn load_delivery(
    connection: &mut SqliteConnection,
    delivery_id: &DeliveryId,
) -> Result<Option<FeedbackDeliveryRecord>, FactStoreError> {
    let row = sqlx::query(
        "SELECT request_id, session_id, resolution, package_id, state, attempt_count,
                last_error_code, last_error_at, created_at, delivered_at
         FROM feedback_deliveries_v3 WHERE delivery_id = ?",
    )
    .bind(delivery_id.as_str())
    .fetch_optional(&mut *connection)
    .await
    .map_err(storage_error)?;
    delivery_from_optional_row(connection, delivery_id.clone(), row).await
}

pub(super) async fn load_delivery_for_request(
    connection: &mut SqliteConnection,
    request_id: &RequestId,
) -> Result<Option<FeedbackDeliveryRecord>, FactStoreError> {
    let row = sqlx::query(
        "SELECT delivery_id, request_id, session_id, resolution, package_id, state, attempt_count,
                last_error_code, last_error_at, created_at, delivered_at
         FROM feedback_deliveries_v3 WHERE request_id = ?",
    )
    .bind(request_id.as_str())
    .fetch_optional(&mut *connection)
    .await
    .map_err(storage_error)?;
    let delivery_id = row
        .as_ref()
        .map(|value| DeliveryId::new(value.get::<String, _>("delivery_id")))
        .unwrap_or_else(|| DeliveryId::new(""));
    delivery_from_optional_row(connection, delivery_id, row).await
}

async fn delivery_from_optional_row(
    connection: &mut SqliteConnection,
    delivery_id: DeliveryId,
    row: Option<SqliteRow>,
) -> Result<Option<FeedbackDeliveryRecord>, FactStoreError> {
    let Some(row) = row else { return Ok(None) };
    let resolution = resolution_from_label(&row.get::<String, _>("resolution"))?;
    let state = match row.get::<String, _>("state").as_str() {
        "pending" => DeliveryState::Pending,
        "delivered" => DeliveryState::Delivered,
        _ => return Err(FactStoreError::CorruptData),
    };
    let package_id = row
        .get::<Option<String>, _>("package_id")
        .map(PackageId::new);
    let package = match package_id.as_ref() {
        Some(package_id) => Some(
            load_package(connection, package_id)
                .await?
                .ok_or(FactStoreError::CorruptData)?,
        ),
        None => None,
    };
    let request_id = RequestId::new(row.get::<String, _>("request_id"));
    let cancel_reason = if resolution == FeedbackResolution::Cancelled {
        Some(
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT cancel_reason FROM feedback_requests_v3 WHERE request_id = ?",
            )
            .bind(request_id.as_str())
            .fetch_one(connection)
            .await
            .map_err(storage_error)?
            .ok_or(FactStoreError::CorruptData)?,
        )
    } else {
        None
    };
    Ok(Some(FeedbackDeliveryRecord {
        delivery_id,
        request_id,
        session_id: SessionId::new(row.get::<String, _>("session_id")),
        resolution,
        package,
        cancel_reason,
        state,
        attempt_count: checked_u32(row.get("attempt_count"))?,
        last_error_code: row.get("last_error_code"),
        last_error_at: row.get("last_error_at"),
        created_at: row.get("created_at"),
        delivered_at: row.get("delivered_at"),
    }))
}

pub(super) async fn load_draft(
    connection: &mut SqliteConnection,
    draft_id: &DraftId,
) -> Result<Option<DraftSnapshot>, FactStoreError> {
    let Some(row) = sqlx::query(
        "SELECT intent, session_id, request_id, launch_configuration_json, document_json,
                body_markdown, revision, created_at, updated_at
         FROM ramble_drafts_v3 WHERE draft_id = ?",
    )
    .bind(draft_id.as_str())
    .fetch_optional(&mut *connection)
    .await
    .map_err(storage_error)?
    else {
        return Ok(None);
    };
    let artifact_rows = sqlx::query(
        "SELECT artifact_id, position, display_name, media_type, size_bytes, sha256, storage_key
         FROM draft_artifacts_v3 WHERE draft_id = ? ORDER BY position",
    )
    .bind(draft_id.as_str())
    .fetch_all(&mut *connection)
    .await
    .map_err(storage_error)?;
    let artifacts = artifact_rows
        .into_iter()
        .map(|row| {
            Ok(DraftArtifact {
                artifact_id: rambledesk_core::kernel::ArtifactId::new(
                    row.get::<String, _>("artifact_id"),
                ),
                position: checked_u32(row.get("position"))?,
                display_name: row.get("display_name"),
                media_type: row.get("media_type"),
                size_bytes: checked_u64(row.get("size_bytes"))?,
                sha256: row.get("sha256"),
                storage_key: row.get("storage_key"),
            })
        })
        .collect::<Result<Vec<_>, FactStoreError>>()?;
    let launch_json: Option<String> = row.get("launch_configuration_json");
    Ok(Some(DraftSnapshot {
        draft_id: draft_id.clone(),
        intent: ramble_intent_from_label(&row.get::<String, _>("intent"))?,
        session_id: row
            .get::<Option<String>, _>("session_id")
            .map(SessionId::new),
        request_id: row
            .get::<Option<String>, _>("request_id")
            .map(RequestId::new),
        launch_configuration: launch_json.as_deref().map(parse_json).transpose()?,
        document_json: row.get("document_json"),
        body_markdown: row.get("body_markdown"),
        revision: checked_u64(row.get("revision"))?,
        artifacts,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }))
}

pub(super) async fn load_work(
    connection: &mut SqliteConnection,
    work_id: &AgentWorkId,
) -> Result<Option<AgentWorkRecord>, FactStoreError> {
    let Some(row) = sqlx::query(
        "SELECT session_id, kind, source_submission_id, source_delivery_id, payload_digest,
                state, attempt_count, last_error_code, last_error_at, created_at, completed_at
         FROM agent_work_v3 WHERE work_id = ?",
    )
    .bind(work_id.as_str())
    .fetch_optional(&mut *connection)
    .await
    .map_err(storage_error)?
    else {
        return Ok(None);
    };
    let kind = work_kind_from_label(&row.get::<String, _>("kind"))?;
    let state = work_state_from_label(&row.get::<String, _>("state"))?;
    let (source_id, payload) = match kind {
        AgentWorkKind::LaunchPrompt => {
            let submission_id = SubmissionId::new(required(
                row.get::<Option<String>, _>("source_submission_id"),
            )?);
            let package_id: String =
                sqlx::query_scalar("SELECT package_id FROM packages_v3 WHERE submission_id = ?")
                    .bind(submission_id.as_str())
                    .fetch_one(&mut *connection)
                    .await
                    .map_err(storage_error)?;
            let prompt_markdown: String = sqlx::query_scalar(
                "SELECT body_markdown FROM ramble_submissions_v3 WHERE submission_id = ?",
            )
            .bind(submission_id.as_str())
            .fetch_one(&mut *connection)
            .await
            .map_err(storage_error)?;
            (
                submission_id.as_str().to_owned(),
                AgentWorkPayload::Launch {
                    submission_id,
                    package_id: PackageId::new(package_id),
                    prompt_markdown,
                },
            )
        }
        AgentWorkKind::SteeringPrompt => {
            let submission_id = SubmissionId::new(required(
                row.get::<Option<String>, _>("source_submission_id"),
            )?);
            let prompt_markdown: String = sqlx::query_scalar(
                "SELECT body_markdown FROM ramble_submissions_v3 WHERE submission_id = ?",
            )
            .bind(submission_id.as_str())
            .fetch_one(&mut *connection)
            .await
            .map_err(storage_error)?;
            (
                submission_id.as_str().to_owned(),
                AgentWorkPayload::Steering {
                    submission_id,
                    prompt_markdown,
                },
            )
        }
        AgentWorkKind::FeedbackResume => {
            let delivery_id = DeliveryId::new(required(
                row.get::<Option<String>, _>("source_delivery_id"),
            )?);
            let request_id: String = sqlx::query_scalar(
                "SELECT request_id FROM feedback_deliveries_v3 WHERE delivery_id = ?",
            )
            .bind(delivery_id.as_str())
            .fetch_one(&mut *connection)
            .await
            .map_err(storage_error)?;
            (
                delivery_id.as_str().to_owned(),
                AgentWorkPayload::FeedbackResume {
                    delivery_id,
                    request_id: RequestId::new(request_id),
                },
            )
        }
    };
    Ok(Some(AgentWorkRecord {
        work_id: work_id.clone(),
        session_id: SessionId::new(row.get::<String, _>("session_id")),
        kind,
        source_id,
        payload_digest: row.get("payload_digest"),
        payload,
        state,
        attempt_count: checked_u32(row.get("attempt_count"))?,
        last_error_code: row.get("last_error_code"),
        last_error_at: row.get("last_error_at"),
        created_at: row.get("created_at"),
        completed_at: row.get("completed_at"),
    }))
}

pub(super) async fn load_link(
    connection: &mut SqliteConnection,
    link_id: &AcpSessionLinkId,
) -> Result<Option<AcpSessionLinkSnapshot>, FactStoreError> {
    let row = sqlx::query(
        "SELECT session_id, agent_profile_id, launch_profile_id, acp_session_id,
                capabilities_json, session_toolset_digest, is_current, created_at, last_used_at
         FROM acp_session_links_v3 WHERE link_id = ?",
    )
    .bind(link_id.as_str())
    .fetch_optional(connection)
    .await
    .map_err(storage_error)?;
    row.map(|row| {
        Ok(AcpSessionLinkSnapshot {
            link_id: link_id.clone(),
            session_id: SessionId::new(row.get::<String, _>("session_id")),
            agent_profile_id: row.get("agent_profile_id"),
            launch_profile_id: row.get("launch_profile_id"),
            acp_session_id: row.get("acp_session_id"),
            capabilities_json: row.get("capabilities_json"),
            session_toolset_digest: row.get("session_toolset_digest"),
            is_current: row.get::<i64, _>("is_current") == 1,
            created_at: row.get("created_at"),
            last_used_at: row.get("last_used_at"),
        })
    })
    .transpose()
}

pub(super) async fn load_launch_outcome(
    connection: &mut SqliteConnection,
    submission_id: &SubmissionId,
) -> Result<LaunchOutcome, FactStoreError> {
    let row = sqlx::query(
        "SELECT s.session_id, s.submission_digest, p.package_id, p.content_digest,
                p.manifest_digest, w.work_id, w.state
         FROM ramble_submissions_v3 s
         JOIN packages_v3 p ON p.submission_id = s.submission_id
         JOIN agent_work_v3 w ON w.source_submission_id = s.submission_id
         WHERE s.submission_id = ? AND s.intent = 'launch' AND w.kind = 'launch_prompt'",
    )
    .bind(submission_id.as_str())
    .fetch_optional(connection)
    .await
    .map_err(storage_error)?
    .ok_or(FactStoreError::CorruptData)?;
    Ok(LaunchOutcome {
        session_id: SessionId::new(row.get::<String, _>("session_id")),
        submission_id: submission_id.clone(),
        submission_digest: row.get("submission_digest"),
        package_id: PackageId::new(row.get::<String, _>("package_id")),
        package_content_digest: row.get("content_digest"),
        package_manifest_digest: row.get("manifest_digest"),
        agent_work_id: AgentWorkId::new(row.get::<String, _>("work_id")),
        agent_work_state: work_state_from_label(&row.get::<String, _>("state"))?,
    })
}

pub(super) async fn load_steering_outcome(
    connection: &mut SqliteConnection,
    submission_id: &SubmissionId,
) -> Result<SteeringOutcome, FactStoreError> {
    let row = sqlx::query(
        "SELECT s.session_id, s.submission_digest, w.work_id, w.state
         FROM ramble_submissions_v3 s
         JOIN agent_work_v3 w ON w.source_submission_id = s.submission_id
         WHERE s.submission_id = ? AND s.intent = 'steering' AND w.kind = 'steering_prompt'",
    )
    .bind(submission_id.as_str())
    .fetch_optional(connection)
    .await
    .map_err(storage_error)?
    .ok_or(FactStoreError::CorruptData)?;
    Ok(SteeringOutcome {
        session_id: SessionId::new(row.get::<String, _>("session_id")),
        submission_id: submission_id.clone(),
        submission_digest: row.get("submission_digest"),
        agent_work_id: AgentWorkId::new(row.get::<String, _>("work_id")),
        agent_work_state: work_state_from_label(&row.get::<String, _>("state"))?,
    })
}

pub(super) async fn load_resolution_outcome(
    connection: &mut SqliteConnection,
    request_id: &RequestId,
) -> Result<FeedbackResolutionOutcome, FactStoreError> {
    let request = load_feedback_request(connection, request_id)
        .await?
        .ok_or(FactStoreError::CorruptData)?;
    let delivery = load_delivery_for_request(connection, request_id)
        .await?
        .ok_or(FactStoreError::CorruptData)?;
    let row = sqlx::query("SELECT work_id FROM agent_work_v3 WHERE source_delivery_id = ?")
        .bind(delivery.delivery_id.as_str())
        .fetch_optional(connection)
        .await
        .map_err(storage_error)?
        .ok_or(FactStoreError::CorruptData)?;
    let package = delivery.package.as_ref();
    Ok(FeedbackResolutionOutcome {
        request,
        package_id: package.map(|value| value.package_id.clone()),
        package_content_digest: package.map(|value| value.content_digest.clone()),
        package_manifest_digest: package.map(|value| value.manifest_digest.clone()),
        delivery_id: delivery.delivery_id,
        delivery_state: delivery.state,
        agent_work_id: AgentWorkId::new(row.get::<String, _>("work_id")),
    })
}

pub(super) fn ramble_intent_label(value: RambleIntent) -> &'static str {
    match value {
        RambleIntent::Launch => "launch",
        RambleIntent::Steering => "steering",
        RambleIntent::Feedback => "feedback",
    }
}

fn ramble_intent_from_label(value: &str) -> Result<RambleIntent, FactStoreError> {
    match value {
        "launch" => Ok(RambleIntent::Launch),
        "steering" => Ok(RambleIntent::Steering),
        "feedback" => Ok(RambleIntent::Feedback),
        _ => Err(FactStoreError::CorruptData),
    }
}

pub(super) fn resolution_label(value: FeedbackResolution) -> &'static str {
    match value {
        FeedbackResolution::Submitted => "submitted",
        FeedbackResolution::Cancelled => "cancelled",
    }
}

fn resolution_from_label(value: &str) -> Result<FeedbackResolution, FactStoreError> {
    match value {
        "submitted" => Ok(FeedbackResolution::Submitted),
        "cancelled" => Ok(FeedbackResolution::Cancelled),
        _ => Err(FactStoreError::CorruptData),
    }
}

pub(super) fn work_kind_label(value: AgentWorkKind) -> &'static str {
    match value {
        AgentWorkKind::LaunchPrompt => "launch_prompt",
        AgentWorkKind::SteeringPrompt => "steering_prompt",
        AgentWorkKind::FeedbackResume => "feedback_resume",
    }
}

pub(super) fn work_kind_from_label(value: &str) -> Result<AgentWorkKind, FactStoreError> {
    match value {
        "launch_prompt" => Ok(AgentWorkKind::LaunchPrompt),
        "steering_prompt" => Ok(AgentWorkKind::SteeringPrompt),
        "feedback_resume" => Ok(AgentWorkKind::FeedbackResume),
        _ => Err(FactStoreError::CorruptData),
    }
}

pub(super) fn work_state_from_label(value: &str) -> Result<AgentWorkState, FactStoreError> {
    match value {
        "pending" => Ok(AgentWorkState::Pending),
        "claimed" => Ok(AgentWorkState::Claimed),
        "completed" => Ok(AgentWorkState::Completed),
        _ => Err(FactStoreError::CorruptData),
    }
}
