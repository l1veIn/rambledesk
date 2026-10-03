use super::*;
use rambledesk_core::{
    VisualFeedbackData, WorkbenchData, WorkbenchPackage, WorkbenchResult, WorkbenchSpec,
    WorkbenchState,
};
use tokio::io::AsyncReadExt;

pub(super) async fn validate_saved_attachment(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
    spec: Option<&WorkbenchSpec>,
    document: &str,
) -> Result<(), RepositoryError> {
    let Some(WorkbenchSpec {
        data: WorkbenchData::VisualFeedback(data),
        ..
    }) = spec
    else {
        return Ok(());
    };
    let draft = crate::workbench_result::parse_feedback_draft(Some(document), true)
        .ok_or(RepositoryError::WorkbenchIncomplete)?;
    if let Some(WorkbenchState::VisualFeedback(state)) = draft.workbench_state
        && let Some(id) = state.composite_attachment_id
    {
        validate_attachment(transaction, request_id, data, &id).await?;
    }
    Ok(())
}

pub(super) async fn validate_package_attachment(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
    package: Option<&WorkbenchPackage>,
) -> Result<(), RepositoryError> {
    if let Some(WorkbenchPackage {
        input:
            WorkbenchSpec {
                data: WorkbenchData::VisualFeedback(data),
                ..
            },
        result: Some(WorkbenchResult::VisualFeedback(result)),
    }) = package
    {
        validate_attachment(
            transaction,
            request_id,
            data,
            &result.composite_attachment_id,
        )
        .await?;
    }
    Ok(())
}

/// This runs inside the same transaction that saves the draft or freezes publication.
/// Only a current feedback attachment can serve as the composite, never a foreign or source image.
async fn validate_attachment(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
    data: &VisualFeedbackData,
    id: &str,
) -> Result<(), RepositoryError> {
    let row = sqlx::query("SELECT media_type, draft_path, byte_size, sha256 FROM attachments WHERE request_id = ?1 AND id = ?2")
        .bind(request_id).bind(id).fetch_optional(&mut **transaction).await.map_err(storage_error)?
        .ok_or(RepositoryError::WorkbenchImageInvalid)?;
    let media_type: String = row.try_get("media_type").map_err(storage_error)?;
    let path: String = row.try_get("draft_path").map_err(storage_error)?;
    let byte_size: i64 = row.try_get("byte_size").map_err(storage_error)?;
    let sha256: String = row.try_get("sha256").map_err(storage_error)?;
    if media_type != "image/png"
        || byte_size <= 0
        || byte_size as u64 > rambledesk_core::MAX_ATTACHMENT_BYTES as u64
    {
        return Err(RepositoryError::WorkbenchImageInvalid);
    }
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| RepositoryError::WorkbenchImageInvalid)?;
    let metadata = file
        .metadata()
        .await
        .map_err(|_| RepositoryError::WorkbenchImageInvalid)?;
    if !metadata.is_file() || metadata.len() != byte_size as u64 {
        return Err(RepositoryError::WorkbenchImageInvalid);
    }
    let mut contents = Vec::new();
    file.take(rambledesk_core::MAX_ATTACHMENT_BYTES as u64 + 1)
        .read_to_end(&mut contents)
        .await
        .map_err(|_| RepositoryError::WorkbenchImageInvalid)?;
    if contents.len() as i64 != byte_size || hex::encode(Sha256::digest(&contents)) != sha256 {
        return Err(RepositoryError::WorkbenchImageInvalid);
    }
    let dimensions = tokio::task::spawn_blocking(move || {
        rambledesk_core::visual_feedback_image_dimensions(&contents, "image/png")
    })
    .await
    .map_err(|_| RepositoryError::WorkbenchImageInvalid)?
    .map_err(|_| RepositoryError::WorkbenchImageInvalid)?;
    if dimensions != (data.width, data.height) {
        return Err(RepositoryError::WorkbenchImageInvalid);
    }
    Ok(())
}
