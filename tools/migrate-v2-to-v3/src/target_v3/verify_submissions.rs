use std::path::Path;

use rambledesk_core::kernel::{
    ArtifactInput, FeedbackSubmission, RequestId, SubmissionId,
    calculate_feedback_submission_digest, validate_feedback_submission_input,
};
use sqlx::{Row, SqlitePool};

use crate::{migration::MigrationError, model::VerifyCheck};

use super::verify_paths::read_real_file;

pub(super) async fn check_submissions(
    pool: &SqlitePool,
    target_root: &Path,
) -> Result<VerifyCheck, MigrationError> {
    let rows = sqlx::query(
        "SELECT submission_id, request_id, document_json, body_markdown, submission_digest \
         FROM ramble_submissions_v3 WHERE intent = 'feedback' ORDER BY submission_id",
    )
    .fetch_all(pool)
    .await
    .map_err(MigrationError::TargetDatabase)?;
    let mut errors = Vec::new();
    for row in &rows {
        let submission_id: String = row
            .try_get("submission_id")
            .map_err(MigrationError::TargetDatabase)?;
        let request_id: String = row
            .try_get("request_id")
            .map_err(MigrationError::TargetDatabase)?;
        let uncooked_rows = sqlx::query(
            "SELECT a.storage_key FROM packages_v3 p \
             JOIN package_artifacts_v3 a ON a.package_id = p.package_id \
             WHERE p.submission_id = ?1 AND a.role = 'uncooked' ORDER BY a.position",
        )
        .bind(&submission_id)
        .fetch_all(pool)
        .await
        .map_err(MigrationError::TargetDatabase)?;
        if uncooked_rows.len() != 1 {
            errors.push(format!(
                "submission {submission_id} must have exactly one uncooked Package Artifact"
            ));
            continue;
        }
        let uncooked_key: String = uncooked_rows[0]
            .try_get("storage_key")
            .map_err(MigrationError::TargetDatabase)?;
        let uncooked = match read_artifact_text(target_root, &uncooked_key).await {
            Ok(value) => value,
            Err(_) => {
                errors.push(format!(
                    "submission {submission_id} uncooked text is invalid"
                ));
                continue;
            }
        };
        let artifact_rows = sqlx::query(
            "SELECT position, display_name, media_type, storage_key \
             FROM submission_artifacts_v3 WHERE submission_id = ?1 ORDER BY position",
        )
        .bind(&submission_id)
        .fetch_all(pool)
        .await
        .map_err(MigrationError::TargetDatabase)?;
        let mut artifacts = Vec::with_capacity(artifact_rows.len());
        let mut artifact_error = false;
        for (expected_position, artifact) in artifact_rows.into_iter().enumerate() {
            let position: i64 = artifact
                .try_get("position")
                .map_err(MigrationError::TargetDatabase)?;
            let storage_key: String = artifact
                .try_get("storage_key")
                .map_err(MigrationError::TargetDatabase)?;
            let contents = match read_artifact_bytes(target_root, &storage_key).await {
                Ok(value) => value,
                Err(_) => {
                    artifact_error = true;
                    break;
                }
            };
            if position != expected_position as i64 {
                artifact_error = true;
                break;
            }
            artifacts.push(ArtifactInput {
                display_name: artifact
                    .try_get("display_name")
                    .map_err(MigrationError::TargetDatabase)?,
                media_type: artifact
                    .try_get("media_type")
                    .map_err(MigrationError::TargetDatabase)?,
                contents,
            });
        }
        if artifact_error {
            errors.push(format!("submission {submission_id} Artifacts are invalid"));
            continue;
        }
        let input = FeedbackSubmission {
            submission_id: SubmissionId::new(submission_id.clone()),
            request_id: RequestId::new(request_id),
            expected_draft_revision: 0,
            submission_digest_assertion: None,
            document_json: row
                .try_get("document_json")
                .map_err(MigrationError::TargetDatabase)?,
            uncooked_markdown: uncooked,
            feedback_markdown: row
                .try_get("body_markdown")
                .map_err(MigrationError::TargetDatabase)?,
            cooking_model: None,
            artifacts,
        };
        let stored_digest: String = row
            .try_get("submission_digest")
            .map_err(MigrationError::TargetDatabase)?;
        if validate_feedback_submission_input(&input).is_err()
            || calculate_feedback_submission_digest(&input) != stored_digest
        {
            errors.push(format!(
                "submission {submission_id} does not match the Core contract"
            ));
        }
    }
    Ok(VerifyCheck {
        name: "feedback_submissions".to_owned(),
        passed: errors.is_empty(),
        detail: if errors.is_empty() {
            format!("verified={}", rows.len())
        } else {
            errors.join("; ")
        },
    })
}

async fn read_artifact_text(root: &Path, storage_key: &str) -> Result<String, ()> {
    String::from_utf8(read_artifact_bytes(root, storage_key).await?).map_err(|_| ())
}

async fn read_artifact_bytes(root: &Path, storage_key: &str) -> Result<Vec<u8>, ()> {
    if !storage_key.starts_with("sha256/") {
        return Err(());
    }
    read_real_file(root, &root.join("library/artifacts").join(storage_key))
        .await
        .map_err(|_| ())
}
