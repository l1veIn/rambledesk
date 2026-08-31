use std::{collections::HashMap, path::Path, time::Duration};

use rambledesk_core::kernel::{
    ArtifactId, ArtifactInput, ArtifactRole, ContextReference, CreateFeedbackRequest,
    FeedbackAction, FeedbackSubmission, PackageArtifact, PackageDigestInput, PackageId,
    PackagePurpose, RequestId, SessionId, SubmissionId, calculate_feedback_request_digest,
    calculate_feedback_submission_digest, calculate_package_digests,
    validate_feedback_request_input, validate_feedback_submission_input,
};
use serde::Serialize;
use sqlx::sqlite::SqliteConnectOptions;

use crate::{
    digest::deterministic_id,
    legacy_v2::{LegacyDataset, LegacyFile, LegacyPackage, LegacyPackageArtifact, LegacyRequest},
    migration::MigrationError,
};

use super::ArtifactIndex;

static V3_MIGRATOR: sqlx::migrate::Migrator =
    sqlx::migrate!("../../crates/rambledesk-storage/migrations_v3");

const IMPORTED_AT: &str = "1970-01-01T00:00:00Z";

#[derive(Debug, Clone)]
struct TargetArtifact {
    artifact_id: String,
    role: &'static str,
    position: u32,
    display_name: String,
    media_type: String,
    size_bytes: u64,
    sha256: String,
    storage_key: String,
}

#[derive(Serialize)]
struct Manifest<'a> {
    schema_version: u32,
    package_id: &'a str,
    submission_id: &'a str,
    package_purpose: &'static str,
    request_id: &'a str,
    content_digest: &'a str,
    artifacts: Vec<ManifestArtifact<'a>>,
    published_at: &'a str,
}

#[derive(Serialize)]
struct ManifestArtifact<'a> {
    artifact_id: &'a str,
    role: &'a str,
    position: u32,
    display_name: &'a str,
    media_type: &'a str,
    size_bytes: u64,
    sha256: &'a str,
}

pub(crate) async fn write_database(
    path: &Path,
    dataset: &LegacyDataset,
    artifacts: &ArtifactIndex,
    _source_database_sha256: &str,
) -> Result<(), MigrationError> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    V3_MIGRATOR
        .run(&pool)
        .await
        .map_err(MigrationError::TargetMigration)?;
    let mut transaction = pool.begin().await.map_err(MigrationError::TargetDatabase)?;
    sqlx::query("PRAGMA defer_foreign_keys = ON")
        .execute(&mut *transaction)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    for artifact in artifacts.values() {
        sqlx::query(
            "INSERT INTO artifact_objects_v3 \
             (storage_key, sha256, size_bytes, created_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&artifact.storage_key)
        .bind(&artifact.sha256)
        .bind(artifact.size_bytes as i64)
        .bind(IMPORTED_AT)
        .execute(&mut *transaction)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    }
    let mut session_ids = HashMap::new();
    for session in &dataset.sessions {
        let session_id = deterministic_id("session", &session.id);
        session_ids.insert(session.id.clone(), session_id.clone());
        sqlx::query(
            "INSERT INTO sessions_v3 \
             (session_id, session_kind_v3001, session_kind, title, lifecycle, \
              launch_configuration_json, created_at, updated_at) \
             VALUES (?1, 'connected', 'imported', ?2, 'stopped', NULL, ?3, ?4)",
        )
        .bind(&session_id)
        .bind(nonblank(&session.title, "Migrated Session"))
        .bind(&session.created_at)
        .bind(&session.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    }
    for request in &dataset.requests {
        let session_id = session_ids.get(&request.session_id).ok_or_else(|| {
            MigrationError::Invariant(format!(
                "request {} references an unplanned Session",
                request.id
            ))
        })?;
        write_request(&mut transaction, request, session_id, artifacts).await?;
    }
    transaction
        .commit()
        .await
        .map_err(MigrationError::TargetDatabase)?;
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    if !violations.is_empty() {
        return Err(MigrationError::Invariant(
            "target database has foreign-key violations".to_owned(),
        ));
    }
    pool.close().await;
    Ok(())
}

async fn write_request(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request: &LegacyRequest,
    session_id: &str,
    artifacts: &ArtifactIndex,
) -> Result<(), MigrationError> {
    let (request_artifacts, request_artifact_inputs) =
        target_request_artifacts(request, artifacts)?;
    for (position, action) in request.actions.iter().take(20).enumerate() {
        sqlx::query(
            "INSERT INTO feedback_request_actions_v3 \
             (request_id, action_id, position, instruction) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&request.id)
        .bind(normalize_action_id(&action.id, position))
        .bind(position as i64)
        .bind(nonblank(
            &action.instruction,
            "Review the migrated feedback request.",
        ))
        .execute(&mut **transaction)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    }
    for (position, context) in request.context_refs.iter().take(20).enumerate() {
        sqlx::query(
            "INSERT INTO feedback_request_context_refs_v3 \
             (request_id, position, label, uri) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&request.id)
        .bind(position as i64)
        .bind(nonblank(&context.label, "Migrated context"))
        .bind(nonblank(&context.uri, "about:blank"))
        .execute(&mut **transaction)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    }
    for artifact in &request_artifacts {
        sqlx::query(
            "INSERT INTO feedback_request_artifacts_v3 \
             (request_id, artifact_id, position, display_name, media_type, size_bytes, sha256, storage_key) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&request.id)
        .bind(&artifact.artifact_id)
        .bind(artifact.position as i64)
        .bind(&artifact.display_name)
        .bind(&artifact.media_type)
        .bind(artifact.size_bytes as i64)
        .bind(&artifact.sha256)
        .bind(&artifact.storage_key)
        .execute(&mut **transaction)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    }
    let response_package_id = request
        .package
        .as_ref()
        .map(|_| deterministic_id("package", &request.id));
    let digest_input = CreateFeedbackRequest {
        request_id: Some(RequestId::new(request.id.clone())),
        session_id: SessionId::new(session_id),
        source_link_id: None,
        title: request.title.clone(),
        instructions: request.instructions.clone(),
        actions: request
            .actions
            .iter()
            .enumerate()
            .map(|(position, action)| FeedbackAction {
                id: normalize_action_id(&action.id, position),
                instruction: nonblank(&action.instruction, "Review the migrated feedback request.")
                    .to_owned(),
            })
            .collect(),
        context_refs: request
            .context_refs
            .iter()
            .map(|context| ContextReference {
                label: context.label.clone(),
                uri: context.uri.clone(),
            })
            .collect(),
        artifacts: request_artifact_inputs,
    };
    validate_feedback_request_input(&digest_input).map_err(|error| {
        MigrationError::InvalidLegacyRequest {
            legacy_id: request.id.clone(),
            reason: error.to_string(),
        }
    })?;
    let input_digest = calculate_feedback_request_digest(&digest_input);
    sqlx::query(
        "INSERT INTO feedback_requests_v3 \
         (request_id, session_id, source_link_id, title, instructions, input_digest, resolution, \
          response_package_id, cancel_reason, created_at, resolved_at, updated_at) \
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9, ?10)",
    )
    .bind(&request.id)
    .bind(session_id)
    .bind(&request.title)
    .bind(&request.instructions)
    .bind(input_digest)
    .bind(response_package_id.as_ref().map(|_| "submitted"))
    .bind(response_package_id.as_deref())
    .bind(&request.created_at)
    .bind(if response_package_id.is_some() {
        request.resolved_at.as_deref().or(Some(&request.updated_at))
    } else {
        None
    })
    .bind(&request.updated_at)
    .execute(&mut **transaction)
    .await
    .map_err(MigrationError::TargetDatabase)?;
    if request.waiting {
        if let Some(draft) = &request.draft {
            write_draft(transaction, request, session_id, draft, artifacts).await?;
        }
    } else if let (Some(package), Some(package_id)) = (&request.package, response_package_id) {
        write_submitted(
            transaction,
            request,
            session_id,
            package,
            &package_id,
            artifacts,
        )
        .await?;
    }
    Ok(())
}

async fn write_draft(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request: &LegacyRequest,
    session_id: &str,
    draft: &crate::legacy_v2::LegacyDraft,
    artifacts: &ArtifactIndex,
) -> Result<(), MigrationError> {
    let draft_id = deterministic_id("draft", &request.id);
    sqlx::query(
        "INSERT INTO ramble_drafts_v3 \
         (draft_id, intent, session_id, request_id, launch_configuration_json, document_json, \
          body_markdown, revision, created_at, updated_at) \
         VALUES (?1, 'feedback', ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8)",
    )
    .bind(&draft_id)
    .bind(session_id)
    .bind(&request.id)
    .bind(&draft.document_json)
    .bind(&draft.body_markdown)
    .bind(draft.revision as i64)
    .bind(&request.created_at)
    .bind(&draft.updated_at)
    .execute(&mut **transaction)
    .await
    .map_err(MigrationError::TargetDatabase)?;
    for file in &draft.artifacts {
        let artifact = target_file_artifact(
            file,
            &format!("{}:draft:{}", request.id, file.id),
            "attachment",
            file.position,
            artifacts,
        )?;
        sqlx::query(
            "INSERT INTO draft_artifacts_v3 \
             (draft_id, artifact_id, position, display_name, media_type, size_bytes, sha256, storage_key) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&draft_id)
        .bind(&artifact.artifact_id)
        .bind(artifact.position as i64)
        .bind(&artifact.display_name)
        .bind(&artifact.media_type)
        .bind(artifact.size_bytes as i64)
        .bind(&artifact.sha256)
        .bind(&artifact.storage_key)
        .execute(&mut **transaction)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    }
    Ok(())
}

async fn write_submitted(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request: &LegacyRequest,
    session_id: &str,
    package: &LegacyPackage,
    package_id: &str,
    artifacts: &ArtifactIndex,
) -> Result<(), MigrationError> {
    let submission_id = deterministic_id("submission", &request.id);
    let normalized = request.submission.as_ref().ok_or_else(|| {
        MigrationError::Invariant(format!(
            "submitted legacy request {} has no validated Submission",
            request.id
        ))
    })?;
    let submission_artifacts = package
        .attachments
        .iter()
        .enumerate()
        .map(|(position, file)| {
            target_package_artifact(
                file,
                &format!("{}:submission:{position}", request.id),
                "attachment",
                position as u32,
                artifacts,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    for artifact in &submission_artifacts {
        sqlx::query(
            "INSERT INTO submission_artifacts_v3 \
             (submission_id, artifact_id, position, display_name, media_type, size_bytes, sha256, storage_key) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&submission_id)
        .bind(&artifact.artifact_id)
        .bind(artifact.position as i64)
        .bind(&artifact.display_name)
        .bind(&artifact.media_type)
        .bind(artifact.size_bytes as i64)
        .bind(&artifact.sha256)
        .bind(&artifact.storage_key)
        .execute(&mut **transaction)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    }
    let submission_input = FeedbackSubmission {
        submission_id: SubmissionId::new(submission_id.clone()),
        request_id: RequestId::new(request.id.clone()),
        expected_draft_revision: 0,
        submission_digest_assertion: None,
        document_json: normalized.document_json.clone(),
        uncooked_markdown: normalized.uncooked_markdown.clone(),
        feedback_markdown: normalized.feedback_markdown.clone(),
        cooking_model: None,
        artifacts: package
            .attachments
            .iter()
            .map(|artifact| ArtifactInput {
                display_name: artifact.display_name.clone(),
                media_type: artifact.media_type.clone(),
                contents: artifact.bytes.clone(),
            })
            .collect(),
    };
    validate_feedback_submission_input(&submission_input).map_err(|error| {
        MigrationError::InvalidLegacyRequest {
            legacy_id: request.id.clone(),
            reason: error.to_string(),
        }
    })?;
    let submission_digest = calculate_feedback_submission_digest(&submission_input);
    if submission_digest != normalized.submission_digest {
        return Err(MigrationError::Invariant(format!(
            "legacy request {} Submission changed after normalization",
            request.id
        )));
    }
    sqlx::query(
        "INSERT INTO ramble_submissions_v3 \
         (submission_id, session_id, intent, request_id, document_json, body_markdown, submission_digest, created_at) \
         VALUES (?1, ?2, 'feedback', ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(&submission_id)
    .bind(session_id)
    .bind(&request.id)
    .bind(&normalized.document_json)
    .bind(&normalized.feedback_markdown)
    .bind(submission_digest)
    .bind(request.resolved_at.as_deref().unwrap_or(&request.updated_at))
    .execute(&mut **transaction)
    .await
    .map_err(MigrationError::TargetDatabase)?;

    let mut package_artifacts = vec![target_package_artifact(
        &package.feedback,
        &format!("{}:package:feedback", request.id),
        "feedback",
        0,
        artifacts,
    )?];
    if let Some(uncooked) = &package.uncooked {
        package_artifacts.push(target_package_artifact(
            uncooked,
            &format!("{}:package:uncooked", request.id),
            "uncooked",
            1,
            artifacts,
        )?);
    }
    let start = package_artifacts.len() as u32;
    for (position, file) in package.attachments.iter().enumerate() {
        package_artifacts.push(target_package_artifact(
            file,
            &format!("{}:package:attachment:{position}", request.id),
            "attachment",
            start + position as u32,
            artifacts,
        )?);
    }
    for artifact in &package_artifacts {
        sqlx::query(
            "INSERT INTO package_artifacts_v3 \
             (package_id, artifact_id, position, role, display_name, media_type, size_bytes, sha256, storage_key) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(package_id)
        .bind(&artifact.artifact_id)
        .bind(artifact.position as i64)
        .bind(artifact.role)
        .bind(&artifact.display_name)
        .bind(&artifact.media_type)
        .bind(artifact.size_bytes as i64)
        .bind(&artifact.sha256)
        .bind(&artifact.storage_key)
        .execute(&mut **transaction)
        .await
        .map_err(MigrationError::TargetDatabase)?;
    }
    let published_at = request
        .published_at
        .as_deref()
        .or(request.resolved_at.as_deref())
        .unwrap_or(&request.updated_at);
    let core_artifacts = package_artifacts
        .iter()
        .map(core_package_artifact)
        .collect::<Vec<_>>();
    let core_package_id = PackageId::new(package_id);
    let core_submission_id = SubmissionId::new(submission_id.clone());
    let core_request_id = RequestId::new(request.id.clone());
    let package_digests = calculate_package_digests(PackageDigestInput {
        package_id: &core_package_id,
        submission_id: &core_submission_id,
        purpose: PackagePurpose::Response,
        request_id: Some(&core_request_id),
        schema_version: 3,
        artifacts: &core_artifacts,
        published_at,
    });
    let content_digest = package_digests.content_digest;
    let manifest_json = serde_json::to_string(&Manifest {
        schema_version: 3,
        package_id,
        submission_id: &submission_id,
        package_purpose: "response",
        request_id: &request.id,
        content_digest: &content_digest,
        artifacts: package_artifacts
            .iter()
            .map(|artifact| ManifestArtifact {
                artifact_id: &artifact.artifact_id,
                role: artifact.role,
                position: artifact.position,
                display_name: &artifact.display_name,
                media_type: &artifact.media_type,
                size_bytes: artifact.size_bytes,
                sha256: &artifact.sha256,
            })
            .collect(),
        published_at,
    })
    .map_err(MigrationError::Serialize)?;
    let manifest_digest = package_digests.manifest_digest;
    sqlx::query(
        "INSERT INTO packages_v3 \
         (package_id, submission_id, package_purpose, request_id, schema_version, manifest_json, \
          content_digest, manifest_digest, published_at) \
         VALUES (?1, ?2, 'response', ?3, 3, ?4, ?5, ?6, ?7)",
    )
    .bind(package_id)
    .bind(&submission_id)
    .bind(&request.id)
    .bind(&manifest_json)
    .bind(&content_digest)
    .bind(&manifest_digest)
    .bind(published_at)
    .execute(&mut **transaction)
    .await
    .map_err(MigrationError::TargetDatabase)?;
    sqlx::query(
        "INSERT INTO feedback_deliveries_v3 \
         (delivery_id, request_id, session_id, resolution, package_id, state, attempt_count, \
          last_error_code, last_error_at, created_at, delivered_at) \
         VALUES (?1, ?2, ?3, 'submitted', ?4, 'delivered', 0, NULL, NULL, ?5, ?5)",
    )
    .bind(deterministic_id("delivery", &request.id))
    .bind(&request.id)
    .bind(session_id)
    .bind(package_id)
    .bind(published_at)
    .execute(&mut **transaction)
    .await
    .map_err(MigrationError::TargetDatabase)?;
    Ok(())
}

fn target_request_artifacts(
    request: &LegacyRequest,
    artifacts: &ArtifactIndex,
) -> Result<(Vec<TargetArtifact>, Vec<ArtifactInput>), MigrationError> {
    let mut files = request.request_artifacts.iter().collect::<Vec<_>>();
    let package_fallback = request
        .package
        .as_ref()
        .map(|package| package.request_attachments.as_slice())
        .unwrap_or_default();
    let mut result = Vec::new();
    let mut inputs = Vec::new();
    for (position, file) in files.drain(..).enumerate() {
        let target = target_file_artifact(
            file,
            &format!("{}:request:{position}", request.id),
            "attachment",
            position as u32,
            artifacts,
        )?;
        inputs.push(ArtifactInput {
            display_name: target.display_name.clone(),
            media_type: target.media_type.clone(),
            contents: file.bytes.clone(),
        });
        result.push(target);
    }
    let database_count = result.len();
    let mut matched_database_entries = vec![false; database_count];
    for file in package_fallback {
        let digest = &file.sha256;
        let match_index = result[..database_count]
            .iter()
            .enumerate()
            .find(|(index, artifact)| {
                !matched_database_entries[*index]
                    && &artifact.sha256 == digest
                    && artifact.display_name == safe_display_name(&file.display_name)
                    && artifact.media_type == nonblank(&file.media_type, "application/octet-stream")
            })
            .map(|(index, _)| index);
        if let Some(index) = match_index {
            matched_database_entries[index] = true;
            continue;
        }
        let position = result.len() as u32;
        let target = target_package_artifact(
            file,
            &format!("{}:request-package:{position}", request.id),
            "attachment",
            position,
            artifacts,
        )?;
        inputs.push(ArtifactInput {
            display_name: target.display_name.clone(),
            media_type: target.media_type.clone(),
            contents: file.bytes.clone(),
        });
        result.push(target);
    }
    Ok((result, inputs))
}

fn target_file_artifact(
    file: &LegacyFile,
    identity: &str,
    role: &'static str,
    position: u32,
    artifacts: &ArtifactIndex,
) -> Result<TargetArtifact, MigrationError> {
    target_artifact(
        &file.display_name,
        &file.media_type,
        &file.sha256,
        identity,
        role,
        position,
        artifacts,
    )
}

fn target_package_artifact(
    file: &LegacyPackageArtifact,
    identity: &str,
    role: &'static str,
    position: u32,
    artifacts: &ArtifactIndex,
) -> Result<TargetArtifact, MigrationError> {
    target_artifact(
        &file.display_name,
        &file.media_type,
        &file.sha256,
        identity,
        role,
        position,
        artifacts,
    )
}

fn target_artifact(
    display_name: &str,
    media_type: &str,
    digest: &str,
    identity: &str,
    role: &'static str,
    position: u32,
    artifacts: &ArtifactIndex,
) -> Result<TargetArtifact, MigrationError> {
    let stored = artifacts.get(digest)?;
    Ok(TargetArtifact {
        artifact_id: deterministic_id("artifact", identity),
        role,
        position,
        display_name: safe_display_name(display_name),
        media_type: nonblank(media_type, "application/octet-stream").to_owned(),
        size_bytes: stored.size_bytes,
        sha256: stored.sha256.clone(),
        storage_key: stored.storage_key.clone(),
    })
}

fn core_package_artifact(artifact: &TargetArtifact) -> PackageArtifact {
    PackageArtifact {
        artifact_id: ArtifactId::new(artifact.artifact_id.clone()),
        role: match artifact.role {
            "feedback" => ArtifactRole::Feedback,
            "uncooked" => ArtifactRole::Uncooked,
            "attachment" => ArtifactRole::Attachment,
            other => ArtifactRole::Other(other.to_owned()),
        },
        position: artifact.position,
        display_name: artifact.display_name.clone(),
        media_type: artifact.media_type.clone(),
        size_bytes: artifact.size_bytes,
        sha256: artifact.sha256.clone(),
        storage_key: artifact.storage_key.clone(),
    }
}

fn normalize_action_id(value: &str, position: usize) -> String {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'_' | b'-'))
        });
    if valid {
        value.to_owned()
    } else {
        format!("legacy-action-{position}")
    }
}

fn safe_display_name(value: &str) -> String {
    let name = Path::new(value)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("attachment.bin");
    nonblank(name, "attachment.bin").to_owned()
}

fn nonblank<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}
