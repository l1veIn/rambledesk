use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use rambledesk_core::kernel::{
    ArtifactInput, FeedbackSubmission, MAX_ARTIFACT_BYTES, MAX_ARTIFACT_TOTAL_BYTES, RequestId,
    SubmissionId, calculate_feedback_submission_digest, validate_feedback_submission_input,
    validate_ramble_draft_content,
};
use sha2::Digest;
use sqlx::{Row, SqlitePool};

use crate::{
    digest::deterministic_id,
    inspect::{InspectError, InspectReport, RecordDisposition},
    model::MigrationLoss,
};

use super::{
    LegacyPackage, LegacyPackageArtifact, LegacyPackageIssue, LegacyPackagePaths,
    package_directory_contains, read_package,
    read_support::{
        MAX_MIGRATION_ARTIFACT_BYTES, charge_artifact_bytes, load_actions, load_context_refs,
        nonblank, valid_action_id,
    },
};

#[derive(Debug, Clone)]
pub(crate) struct LegacySession {
    pub id: String,
    pub host_id: String,
    pub host_session_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyAction {
    pub id: String,
    pub instruction: String,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyContextRef {
    pub label: String,
    pub uri: String,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyFile {
    pub id: String,
    pub display_name: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub position: u32,
    pub legacy_path: String,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyBackupFile {
    pub legacy_id: String,
    pub legacy_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyDraft {
    pub document_json: String,
    pub body_markdown: String,
    pub revision: u64,
    pub updated_at: String,
    pub artifacts: Vec<LegacyFile>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacySubmission {
    pub document_json: String,
    pub feedback_markdown: String,
    pub uncooked_markdown: String,
    pub submission_digest: String,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyRequest {
    pub id: String,
    pub session_id: String,
    pub title: String,
    pub instructions: String,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
    pub published_at: Option<String>,
    pub waiting: bool,
    pub actions: Vec<LegacyAction>,
    pub context_refs: Vec<LegacyContextRef>,
    pub request_artifacts: Vec<LegacyFile>,
    pub draft: Option<LegacyDraft>,
    pub package: Option<LegacyPackage>,
    pub submission: Option<LegacySubmission>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyDataset {
    pub sessions: Vec<LegacySession>,
    pub requests: Vec<LegacyRequest>,
    pub losses: Vec<MigrationLoss>,
    pub backup_files: Vec<LegacyBackupFile>,
    pub records_dropped_during_load: u64,
}

pub(crate) async fn load_dataset(
    pool: &SqlitePool,
    inspected: &InspectReport,
    source_db: &Path,
) -> Result<LegacyDataset, InspectError> {
    let session_title = if column_exists(pool, "host_sessions", "display_title").await? {
        "COALESCE(display_title, host_session_id)"
    } else {
        "host_session_id"
    };
    let session_query = format!(
        "SELECT id, host_id, host_session_id, {session_title} AS title, \
         created_at, updated_at FROM host_sessions ORDER BY id"
    );
    let session_rows = sqlx::query(&session_query)
        .fetch_all(pool)
        .await
        .map_err(InspectError::SourceSchema)?;
    let mut sessions = session_rows
        .into_iter()
        .map(|row| {
            Ok(LegacySession {
                id: row.try_get("id").map_err(InspectError::SourceSchema)?,
                host_id: row.try_get("host_id").map_err(InspectError::SourceSchema)?,
                host_session_id: row
                    .try_get("host_session_id")
                    .map_err(InspectError::SourceSchema)?,
                title: row.try_get("title").map_err(InspectError::SourceSchema)?,
                created_at: row
                    .try_get("created_at")
                    .map_err(InspectError::SourceSchema)?,
                updated_at: row
                    .try_get("updated_at")
                    .map_err(InspectError::SourceSchema)?,
            })
        })
        .collect::<Result<Vec<_>, InspectError>>()?;
    let request_rows = sqlx::query(
        "SELECT r.id, r.host_session_record_id, r.title, r.what_happened, r.status, \
                r.created_at, r.updated_at, r.completed_at, \
                fr.directory_path, fr.markdown_path, fr.manifest_path, fr.manifest_sha256, fr.published_at \
         FROM feedback_requests r \
         LEFT JOIN feedback_results fr ON fr.request_id = r.id ORDER BY r.id",
    )
    .fetch_all(pool)
    .await
    .map_err(InspectError::SourceSchema)?;
    let mut requests = Vec::new();
    let mut records_dropped_during_load = 0u64;
    let mut migration_artifact_bytes = 0usize;
    let mut losses = inspected
        .records
        .iter()
        .filter_map(|record| {
            record.loss_reason.map(|reason| MigrationLoss {
                legacy_id: record.legacy_id.clone(),
                reason: if record.detail == Some(LegacyPackageIssue::UnsafePackageDirectory) {
                    "completed_package_unsafe_directory".to_owned()
                } else {
                    serde_json::to_value(reason)
                        .expect("loss reason serializes")
                        .as_str()
                        .expect("loss reason is a string")
                        .to_owned()
                },
            })
        })
        .collect::<Vec<_>>();

    for row in request_rows {
        let id: String = row.try_get("id").map_err(InspectError::SourceSchema)?;
        let Some(record) = inspected
            .records
            .iter()
            .find(|record| record.legacy_id == id)
        else {
            continue;
        };
        if record.disposition == RecordDisposition::Drop {
            continue;
        }
        let mut actions = load_actions(pool, &id).await?;
        let before_actions = actions.len();
        actions
            .retain(|action| !action.id.trim().is_empty() && !action.instruction.trim().is_empty());
        if actions.len() != before_actions {
            losses.push(MigrationLoss {
                legacy_id: id.clone(),
                reason: "blank_action_dropped".to_owned(),
            });
        }
        if actions.len() > 20 {
            actions.truncate(20);
            losses.push(MigrationLoss {
                legacy_id: id.clone(),
                reason: "actions_truncated".to_owned(),
            });
        }
        let mut action_ids = HashSet::new();
        let mut action_ids_normalized = false;
        for (position, action) in actions.iter_mut().enumerate() {
            if !valid_action_id(&action.id) || !action_ids.insert(action.id.clone()) {
                let mut replacement = format!("legacy-action-{position}");
                while !action_ids.insert(replacement.clone()) {
                    replacement.push('x');
                }
                action.id = replacement;
                action_ids_normalized = true;
            }
        }
        if action_ids_normalized {
            losses.push(MigrationLoss {
                legacy_id: id.clone(),
                reason: "action_ids_normalized".to_owned(),
            });
        }
        if actions.is_empty() {
            actions.push(LegacyAction {
                id: "review".to_owned(),
                instruction: "Review the migrated feedback request.".to_owned(),
            });
            losses.push(MigrationLoss {
                legacy_id: id.clone(),
                reason: "missing_actions_synthesized".to_owned(),
            });
        }
        let mut context_refs = load_context_refs(pool, &id).await?;
        let before_context = context_refs.len();
        context_refs
            .retain(|context| !context.label.trim().is_empty() && !context.uri.trim().is_empty());
        if context_refs.len() != before_context {
            losses.push(MigrationLoss {
                legacy_id: id.clone(),
                reason: "blank_context_ref_dropped".to_owned(),
            });
        }
        if context_refs.len() > 20 {
            context_refs.truncate(20);
            losses.push(MigrationLoss {
                legacy_id: id.clone(),
                reason: "context_refs_truncated".to_owned(),
            });
        }
        let request_artifacts = load_request_artifacts(pool, &id, &mut losses).await?;
        let request_artifact_bytes = request_artifacts
            .iter()
            .map(|file| file.bytes.len())
            .try_fold(0usize, usize::checked_add)
            .ok_or_else(|| {
                InspectError::ResourceLimit(format!(
                    "legacy request {id} Artifact byte count overflowed"
                ))
            })?;
        charge_artifact_bytes(&mut migration_artifact_bytes, request_artifact_bytes, &id)?;
        let draft = load_draft(pool, &id, &mut migration_artifact_bytes, &mut losses).await?;
        let mut package = if record.disposition == RecordDisposition::MigrateSubmitted {
            let paths = LegacyPackagePaths {
                directory_path: row
                    .try_get("directory_path")
                    .map_err(InspectError::SourceSchema)?,
                markdown_path: row
                    .try_get("markdown_path")
                    .map_err(InspectError::SourceSchema)?,
                manifest_path: row
                    .try_get("manifest_path")
                    .map_err(InspectError::SourceSchema)?,
                manifest_sha256: row
                    .try_get("manifest_sha256")
                    .map_err(InspectError::SourceSchema)?,
            };
            Some(read_package(&id, &paths).await.map_err(|_| {
                InspectError::SourceSchema(sqlx::Error::Protocol(
                    "legacy Package changed after inspection".to_owned(),
                ))
            })?)
        } else {
            None
        };
        if let Some(package) = &package {
            if draft.is_none() {
                losses.push(MigrationLoss {
                    legacy_id: id.clone(),
                    reason: "submitted_document_synthesized".to_owned(),
                });
            }
            if row
                .try_get::<Option<String>, _>("published_at")
                .map_err(InspectError::SourceSchema)?
                .is_none()
            {
                losses.push(MigrationLoss {
                    legacy_id: id.clone(),
                    reason: "published_at_synthesized".to_owned(),
                });
            }
            for artifact in package
                .attachments
                .iter()
                .chain(&package.request_attachments)
                .filter(|artifact| artifact.metadata_synthesized)
            {
                losses.push(MigrationLoss {
                    legacy_id: format!("{}:{}", id, artifact.id),
                    reason: "attachment_metadata_synthesized".to_owned(),
                });
            }
        }
        let submission = if let Some(package) = &mut package {
            let feedback_markdown = match std::str::from_utf8(&package.feedback.bytes) {
                Ok(value) => value.to_owned(),
                Err(_) => {
                    losses.push(MigrationLoss {
                        legacy_id: id.clone(),
                        reason: "submitted_feedback_invalid_utf8".to_owned(),
                    });
                    records_dropped_during_load += 1;
                    continue;
                }
            };
            let uncooked_markdown = if let Some(uncooked) = &package.uncooked {
                match std::str::from_utf8(&uncooked.bytes) {
                    Ok(value) => value.to_owned(),
                    Err(_) => {
                        losses.push(MigrationLoss {
                            legacy_id: id.clone(),
                            reason: "submitted_uncooked_invalid_utf8".to_owned(),
                        });
                        records_dropped_during_load += 1;
                        continue;
                    }
                }
            } else {
                losses.push(MigrationLoss {
                    legacy_id: id.clone(),
                    reason: "submitted_uncooked_synthesized".to_owned(),
                });
                package.uncooked = Some(LegacyPackageArtifact {
                    id: format!("legacy-{id}-uncooked-synthesized"),
                    display_name: "uncooked.md".to_owned(),
                    media_type: "text/markdown; charset=utf-8".to_owned(),
                    bytes: package.feedback.bytes.clone(),
                    sha256: package.feedback.sha256.clone(),
                    legacy_path: format!("synthetic:{id}/uncooked.md"),
                    metadata_synthesized: false,
                });
                feedback_markdown.clone()
            };
            let document_json = draft
                .as_ref()
                .map(|draft| draft.document_json.clone())
                .unwrap_or_else(|| document_from_markdown(&feedback_markdown));
            let input = FeedbackSubmission {
                submission_id: SubmissionId::new(deterministic_id("submission", &id)),
                request_id: RequestId::new(id.clone()),
                expected_draft_revision: 0,
                submission_digest_assertion: None,
                document_json: document_json.clone(),
                uncooked_markdown: uncooked_markdown.clone(),
                feedback_markdown: feedback_markdown.clone(),
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
            if validate_feedback_submission_input(&input).is_err() {
                losses.push(MigrationLoss {
                    legacy_id: id.clone(),
                    reason: "submitted_request_invalid".to_owned(),
                });
                records_dropped_during_load += 1;
                continue;
            }
            Some(LegacySubmission {
                document_json,
                feedback_markdown,
                uncooked_markdown,
                submission_digest: calculate_feedback_submission_digest(&input),
            })
        } else {
            None
        };
        let stored_title: String = row.try_get("title").map_err(InspectError::SourceSchema)?;
        let stored_instructions: String = row
            .try_get("what_happened")
            .map_err(InspectError::SourceSchema)?;
        if stored_title.trim().is_empty() {
            losses.push(MigrationLoss {
                legacy_id: id.clone(),
                reason: "request_title_synthesized".to_owned(),
            });
        }
        if stored_instructions.trim().is_empty() {
            losses.push(MigrationLoss {
                legacy_id: id.clone(),
                reason: "request_instructions_synthesized".to_owned(),
            });
        }
        let request_bytes = package
            .iter()
            .flat_map(|package| {
                std::iter::once(package.manifest.bytes.len())
                    .chain(std::iter::once(package.feedback.bytes.len()))
                    .chain(package.uncooked.iter().map(|file| file.bytes.len()))
                    .chain(package.attachments.iter().map(|file| file.bytes.len()))
                    .chain(
                        package
                            .request_attachments
                            .iter()
                            .map(|file| file.bytes.len()),
                    )
            })
            .try_fold(0usize, usize::checked_add)
            .ok_or_else(|| {
                InspectError::ResourceLimit(format!("legacy request {id} byte count overflowed"))
            })?;
        charge_artifact_bytes(&mut migration_artifact_bytes, request_bytes, &id)?;
        requests.push(LegacyRequest {
            id,
            session_id: row
                .try_get("host_session_record_id")
                .map_err(InspectError::SourceSchema)?,
            title: nonblank(stored_title, "Migrated Feedback Request"),
            instructions: nonblank(stored_instructions, "Review the migrated feedback request."),
            created_at: row
                .try_get("created_at")
                .map_err(InspectError::SourceSchema)?,
            updated_at: row
                .try_get("updated_at")
                .map_err(InspectError::SourceSchema)?,
            resolved_at: row
                .try_get("completed_at")
                .map_err(InspectError::SourceSchema)?,
            published_at: row
                .try_get("published_at")
                .map_err(InspectError::SourceSchema)?,
            waiting: record.disposition == RecordDisposition::MigrateWaiting,
            actions,
            context_refs,
            request_artifacts,
            draft,
            package,
            submission,
        });
    }
    sessions.retain(|session| {
        requests
            .iter()
            .any(|request| request.session_id == session.id)
    });
    let backup_files =
        load_backup_files(pool, source_db, &mut migration_artifact_bytes, &mut losses).await?;
    losses.sort_by(|left, right| {
        left.legacy_id
            .cmp(&right.legacy_id)
            .then_with(|| left.reason.cmp(&right.reason))
    });
    Ok(LegacyDataset {
        sessions,
        requests,
        losses,
        backup_files,
        records_dropped_during_load,
    })
}

async fn load_backup_files(
    pool: &SqlitePool,
    source_db: &Path,
    migration_artifact_bytes: &mut usize,
    losses: &mut Vec<MigrationLoss>,
) -> Result<Vec<LegacyBackupFile>, InspectError> {
    let mut files = Vec::new();
    let package_rows = sqlx::query(
        "SELECT request_id, directory_path, markdown_path, manifest_path, manifest_sha256 \
         FROM feedback_results ORDER BY request_id",
    )
    .fetch_all(pool)
    .await
    .map_err(InspectError::SourceSchema)?;
    for row in package_rows {
        let request_id: String = row
            .try_get("request_id")
            .map_err(InspectError::SourceSchema)?;
        let paths = LegacyPackagePaths {
            directory_path: row
                .try_get("directory_path")
                .map_err(InspectError::SourceSchema)?,
            markdown_path: row
                .try_get("markdown_path")
                .map_err(InspectError::SourceSchema)?,
            manifest_path: row
                .try_get("manifest_path")
                .map_err(InspectError::SourceSchema)?,
            manifest_sha256: row
                .try_get("manifest_sha256")
                .map_err(InspectError::SourceSchema)?,
        };
        if package_directory_contains(&paths, source_db).await {
            continue;
        }
        if let Ok(package) = read_package(&request_id, &paths).await {
            for artifact in std::iter::once(package.manifest)
                .chain(std::iter::once(package.feedback))
                .chain(package.uncooked)
                .chain(package.attachments)
                .chain(package.request_attachments)
            {
                charge_artifact_bytes(migration_artifact_bytes, artifact.bytes.len(), &request_id)?;
                files.push(LegacyBackupFile {
                    legacy_id: format!("{request_id}:{}", artifact.id),
                    legacy_path: artifact.legacy_path,
                    bytes: artifact.bytes,
                });
            }
        }
    }
    for table in ["attachments", "request_attachments"] {
        if !table_exists(pool, table).await? {
            continue;
        }
        let has_published = column_exists(pool, table, "published_path").await?;
        let has_draft = column_exists(pool, table, "draft_path").await?;
        let has_contents = column_exists(pool, table, "contents").await?;
        if has_contents {
            let inline_bytes: i64 = sqlx::query_scalar(&format!(
                "SELECT COALESCE(SUM(CASE WHEN length(contents) <= {MAX_ARTIFACT_BYTES} \
                 THEN length(contents) ELSE 0 END), 0) FROM {table}"
            ))
            .fetch_one(pool)
            .await
            .map_err(InspectError::SourceSchema)?;
            let remaining = MAX_MIGRATION_ARTIFACT_BYTES.saturating_sub(*migration_artifact_bytes);
            if inline_bytes.max(0) as usize > remaining {
                return Err(InspectError::ResourceLimit(format!(
                    "inline {table} bytes exceed the remaining migration budget"
                )));
            }
        }
        let query = format!(
            "SELECT id, request_id, {}, {}, {} FROM {table} ORDER BY request_id, id",
            if has_published {
                "published_path"
            } else {
                "NULL AS published_path"
            },
            if has_draft {
                "draft_path"
            } else {
                "NULL AS draft_path"
            },
            if has_contents {
                "CASE WHEN length(contents) <= 20971520 THEN contents ELSE NULL END AS contents"
            } else {
                "NULL AS contents"
            },
        );
        for row in sqlx::query(&query)
            .fetch_all(pool)
            .await
            .map_err(InspectError::SourceSchema)?
        {
            let id: String = row.try_get("id").map_err(InspectError::SourceSchema)?;
            let request_id: String = row
                .try_get("request_id")
                .map_err(InspectError::SourceSchema)?;
            let contents: Option<Vec<u8>> = row
                .try_get("contents")
                .map_err(InspectError::SourceSchema)?;
            if let Some(contents) = contents.filter(|value| !value.is_empty()) {
                charge_artifact_bytes(
                    migration_artifact_bytes,
                    contents.len(),
                    &format!("{request_id}:{id}"),
                )?;
                files.push(LegacyBackupFile {
                    legacy_id: format!("{request_id}:{id}"),
                    legacy_path: format!("database:{table}/{id}"),
                    bytes: contents,
                });
            }
            for column in ["published_path", "draft_path"] {
                let path: Option<String> =
                    row.try_get(column).map_err(InspectError::SourceSchema)?;
                let Some(path) = path else { continue };
                match read_regular_file(Path::new(&path)).await {
                    Some(bytes) => {
                        charge_artifact_bytes(
                            migration_artifact_bytes,
                            bytes.len(),
                            &format!("{request_id}:{id}"),
                        )?;
                        files.push(LegacyBackupFile {
                            legacy_id: format!("{request_id}:{id}"),
                            legacy_path: path,
                            bytes,
                        });
                    }
                    None => losses.push(MigrationLoss {
                        legacy_id: format!("{request_id}:{id}"),
                        reason: "backup_source_unreadable".to_owned(),
                    }),
                }
            }
        }
    }
    files.sort_by(|left, right| {
        left.legacy_id
            .cmp(&right.legacy_id)
            .then_with(|| left.legacy_path.cmp(&right.legacy_path))
    });
    files.dedup_by(|left, right| {
        left.legacy_id == right.legacy_id
            && left.legacy_path == right.legacy_path
            && left.bytes == right.bytes
    });
    Ok(files)
}

async fn load_draft(
    pool: &SqlitePool,
    request_id: &str,
    migration_artifact_bytes: &mut usize,
    losses: &mut Vec<MigrationLoss>,
) -> Result<Option<LegacyDraft>, InspectError> {
    let has_document = column_exists(pool, "drafts", "document_json").await?;
    let query = if has_document {
        "SELECT document_json, body_markdown, revision, updated_at FROM drafts WHERE request_id = ?1"
    } else {
        "SELECT NULL AS document_json, body_markdown, revision, updated_at FROM drafts WHERE request_id = ?1"
    };
    let Some(row) = sqlx::query(query)
        .bind(request_id)
        .fetch_optional(pool)
        .await
        .map_err(InspectError::SourceSchema)?
    else {
        return Ok(None);
    };
    let body_markdown: String = row
        .try_get("body_markdown")
        .map_err(InspectError::SourceSchema)?;
    let stored_document: Option<String> = row
        .try_get("document_json")
        .map_err(InspectError::SourceSchema)?;
    let document_json = match stored_document {
        Some(value) if !value.trim().is_empty() => value,
        _ => {
            losses.push(MigrationLoss {
                legacy_id: request_id.to_owned(),
                reason: "markdown_draft_structure_synthesized".to_owned(),
            });
            serde_json::json!({
                "schemaVersion": 2,
                "doc": {"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": body_markdown}]}]}
            })
            .to_string()
        }
    };
    let mut artifacts = load_path_artifacts(
        pool,
        "attachments",
        request_id,
        migration_artifact_bytes,
        losses,
    )
    .await?;
    let stored_revision: i64 = row
        .try_get("revision")
        .map_err(InspectError::SourceSchema)?;
    if stored_revision < 0 {
        losses.push(MigrationLoss {
            legacy_id: request_id.to_owned(),
            reason: "draft_revision_clamped".to_owned(),
        });
    }
    let artifact_inputs = artifacts
        .iter()
        .map(|artifact| ArtifactInput {
            display_name: artifact.display_name.clone(),
            media_type: artifact.media_type.clone(),
            contents: artifact.bytes.clone(),
        })
        .collect::<Vec<_>>();
    if validate_ramble_draft_content(&document_json, &body_markdown, &artifact_inputs).is_err() {
        let artifact_bytes = artifacts.iter().map(|file| file.bytes.len()).sum::<usize>();
        *migration_artifact_bytes = migration_artifact_bytes.saturating_sub(artifact_bytes);
        if validate_ramble_draft_content(&document_json, &body_markdown, &[]).is_ok() {
            artifacts.clear();
            losses.push(MigrationLoss {
                legacy_id: request_id.to_owned(),
                reason: "invalid_draft_artifacts_dropped".to_owned(),
            });
        } else {
            losses.push(MigrationLoss {
                legacy_id: request_id.to_owned(),
                reason: "invalid_draft_dropped".to_owned(),
            });
            return Ok(None);
        }
    }
    Ok(Some(LegacyDraft {
        document_json,
        body_markdown,
        revision: stored_revision.max(0) as u64,
        updated_at: row
            .try_get("updated_at")
            .map_err(InspectError::SourceSchema)?,
        artifacts,
    }))
}

async fn load_request_artifacts(
    pool: &SqlitePool,
    request_id: &str,
    losses: &mut Vec<MigrationLoss>,
) -> Result<Vec<LegacyFile>, InspectError> {
    if !table_exists(pool, "request_attachments").await? {
        return Ok(Vec::new());
    }
    let has_draft_path = column_exists(pool, "request_attachments", "draft_path").await?;
    let has_published_path = column_exists(pool, "request_attachments", "published_path").await?;
    let inline_total: i64 = sqlx::query_scalar(&format!(
        "SELECT COALESCE(SUM(CASE WHEN length(contents) <= {MAX_ARTIFACT_BYTES} \
         THEN length(contents) ELSE 0 END), 0) FROM request_attachments WHERE request_id = ?1"
    ))
    .bind(request_id)
    .fetch_one(pool)
    .await
    .map_err(InspectError::SourceSchema)?;
    if inline_total.max(0) as usize > MAX_ARTIFACT_TOTAL_BYTES {
        losses.push(MigrationLoss {
            legacy_id: request_id.to_owned(),
            reason: "request_artifacts_total_exceeded".to_owned(),
        });
        return Ok(Vec::new());
    }
    let draft_projection = if has_draft_path {
        "draft_path"
    } else {
        "NULL AS draft_path"
    };
    let published_projection = if has_published_path {
        "published_path"
    } else {
        "NULL AS published_path"
    };
    let query = format!(
        "SELECT id, file_name, media_type, byte_size, sha256, position, \
         CASE WHEN length(contents) <= {MAX_ARTIFACT_BYTES} THEN contents ELSE NULL END AS contents, \
         length(contents) AS contents_length, \
         {draft_projection}, {published_projection} FROM request_attachments \
         WHERE request_id = ?1 ORDER BY position, id"
    );
    let rows = sqlx::query(&query)
        .bind(request_id)
        .fetch_all(pool)
        .await
        .map_err(InspectError::SourceSchema)?;
    let mut result = Vec::new();
    let mut total_bytes = 0usize;
    let mut total_exceeded = false;
    for row in rows {
        let contents_length: Option<i64> = row
            .try_get("contents_length")
            .map_err(InspectError::SourceSchema)?;
        let bytes: Option<Vec<u8>> = row
            .try_get("contents")
            .map_err(InspectError::SourceSchema)?;
        let path: Option<String> = row
            .try_get("draft_path")
            .map_err(InspectError::SourceSchema)?;
        let published_path: Option<String> = row
            .try_get("published_path")
            .map_err(InspectError::SourceSchema)?;
        let oversized_inline =
            contents_length.is_some_and(|length| length > MAX_ARTIFACT_BYTES as i64);
        let (bytes, legacy_path) = if oversized_inline {
            (None, format!("database:request_attachments/{request_id}"))
        } else if let Some(bytes) = bytes.filter(|value| !value.is_empty()) {
            (
                Some(bytes),
                format!("database:request_attachments/{request_id}"),
            )
        } else if let Some(path) = published_path {
            (read_regular_file(Path::new(&path)).await, path)
        } else if let Some(path) = path {
            (read_regular_file(Path::new(&path)).await, path)
        } else {
            (None, format!("database:request_attachments/{request_id}"))
        };
        if let Some(file) = file_from_row(
            row,
            bytes,
            request_id,
            "request-attachments",
            legacy_path,
            if oversized_inline {
                "oversized_attachment_dropped"
            } else {
                "unreadable_attachment"
            },
            losses,
        )? {
            total_bytes = total_bytes.saturating_add(file.bytes.len());
            if total_bytes > MAX_ARTIFACT_TOTAL_BYTES {
                total_exceeded = true;
                break;
            }
            result.push(file);
        }
    }
    if total_exceeded {
        losses.push(MigrationLoss {
            legacy_id: request_id.to_owned(),
            reason: "request_artifacts_total_exceeded".to_owned(),
        });
        result.clear();
    }
    Ok(result)
}

async fn load_path_artifacts(
    pool: &SqlitePool,
    table: &str,
    request_id: &str,
    migration_artifact_bytes: &mut usize,
    losses: &mut Vec<MigrationLoss>,
) -> Result<Vec<LegacyFile>, InspectError> {
    if !table_exists(pool, table).await? {
        return Ok(Vec::new());
    }
    let query = format!(
        "SELECT id, file_name, media_type, byte_size, sha256, position, draft_path \
         FROM {table} WHERE request_id = ?1 ORDER BY position, id"
    );
    let mut rows = sqlx::query(&query)
        .bind(request_id)
        .fetch_all(pool)
        .await
        .map_err(InspectError::SourceSchema)?;
    if rows.len() > 20 {
        rows.truncate(20);
        losses.push(MigrationLoss {
            legacy_id: request_id.to_owned(),
            reason: "draft_artifacts_truncated".to_owned(),
        });
    }
    let mut declared_bytes = 0usize;
    for row in &rows {
        let path: String = row
            .try_get("draft_path")
            .map_err(InspectError::SourceSchema)?;
        if let Some(size) = regular_file_size(Path::new(&path)).await
            && size <= MAX_ARTIFACT_BYTES
        {
            declared_bytes = declared_bytes.saturating_add(size);
        }
    }
    if declared_bytes > MAX_ARTIFACT_TOTAL_BYTES {
        losses.push(MigrationLoss {
            legacy_id: request_id.to_owned(),
            reason: "draft_artifacts_total_exceeded".to_owned(),
        });
        return Ok(Vec::new());
    }
    if declared_bytes > MAX_MIGRATION_ARTIFACT_BYTES.saturating_sub(*migration_artifact_bytes) {
        losses.push(MigrationLoss {
            legacy_id: request_id.to_owned(),
            reason: "migration_artifact_budget_exceeded".to_owned(),
        });
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for row in rows {
        let path: String = row
            .try_get("draft_path")
            .map_err(InspectError::SourceSchema)?;
        let bytes = read_regular_file(Path::new(&path)).await;
        if let Some(file) = file_from_row(
            row,
            bytes,
            request_id,
            "draft-attachments",
            path,
            "unreadable_attachment",
            losses,
        )? {
            charge_artifact_bytes(migration_artifact_bytes, file.bytes.len(), request_id)?;
            result.push(file);
        }
    }
    let positions_normalized = result
        .iter()
        .enumerate()
        .any(|(position, file)| file.position != position as u32);
    if positions_normalized {
        for (position, file) in result.iter_mut().enumerate() {
            file.position = position as u32;
        }
        losses.push(MigrationLoss {
            legacy_id: request_id.to_owned(),
            reason: "draft_artifact_positions_normalized".to_owned(),
        });
    }
    Ok(result)
}

fn file_from_row(
    row: sqlx::sqlite::SqliteRow,
    bytes: Option<Vec<u8>>,
    request_id: &str,
    _backup_group: &str,
    legacy_path: String,
    unreadable_reason: &str,
    losses: &mut Vec<MigrationLoss>,
) -> Result<Option<LegacyFile>, InspectError> {
    let id: String = row.try_get("id").map_err(InspectError::SourceSchema)?;
    let Some(bytes) = bytes else {
        losses.push(MigrationLoss {
            legacy_id: format!("{request_id}:{id}"),
            reason: unreadable_reason.to_owned(),
        });
        return Ok(None);
    };
    let stored_digest: String = row.try_get("sha256").map_err(InspectError::SourceSchema)?;
    let actual = hex::encode(sha2::Sha256::digest(&bytes));
    let expected = stored_digest
        .strip_prefix("sha256:")
        .unwrap_or(&stored_digest);
    let stored_size: i64 = row
        .try_get("byte_size")
        .map_err(InspectError::SourceSchema)?;
    if actual != expected || stored_size.max(0) as usize != bytes.len() {
        losses.push(MigrationLoss {
            legacy_id: format!("{request_id}:{id}"),
            reason: "attachment_digest_mismatch".to_owned(),
        });
        return Ok(None);
    }
    let stored_name: String = row
        .try_get("file_name")
        .map_err(InspectError::SourceSchema)?;
    let stored_media_type: String = row
        .try_get("media_type")
        .map_err(InspectError::SourceSchema)?;
    let safe_name = Path::new(&stored_name)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("attachment.bin")
        .to_owned();
    let metadata_synthesized = safe_name != stored_name || stored_media_type.trim().is_empty();
    if metadata_synthesized {
        losses.push(MigrationLoss {
            legacy_id: format!("{request_id}:{id}"),
            reason: "attachment_metadata_synthesized".to_owned(),
        });
    }
    Ok(Some(LegacyFile {
        id,
        display_name: safe_name,
        media_type: if stored_media_type.trim().is_empty() {
            "application/octet-stream".to_owned()
        } else {
            stored_media_type
        },
        bytes,
        sha256: format!("sha256:{actual}"),
        position: row
            .try_get::<i64, _>("position")
            .map_err(InspectError::SourceSchema)?
            .max(0) as u32,
        legacy_path,
    }))
}

async fn read_regular_file(path: &Path) -> Option<Vec<u8>> {
    let metadata = tokio::fs::symlink_metadata(path).await.ok()?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_ARTIFACT_BYTES as u64
    {
        return None;
    }
    tokio::fs::read(PathBuf::from(path)).await.ok()
}

async fn regular_file_size(path: &Path) -> Option<usize> {
    let metadata = tokio::fs::symlink_metadata(path).await.ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return None;
    }
    usize::try_from(metadata.len()).ok()
}

fn document_from_markdown(markdown: &str) -> String {
    serde_json::json!({
        "schemaVersion": 2,
        "doc": {"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": markdown}]}]}
    })
    .to_string()
}

pub(super) async fn table_exists(pool: &SqlitePool, table: &str) -> Result<bool, InspectError> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .map_err(InspectError::SourceSchema)
}

async fn column_exists(pool: &SqlitePool, table: &str, column: &str) -> Result<bool, InspectError> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await
        .map_err(InspectError::SourceSchema)?;
    Ok(rows.iter().any(|row| {
        row.try_get::<String, _>("name")
            .map(|name| name == column)
            .unwrap_or(false)
    }))
}
