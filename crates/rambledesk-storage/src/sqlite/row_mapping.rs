use super::*;

pub(super) fn summary_from_row(row: &SqliteRow) -> Result<FeedbackRequestSummary, RepositoryError> {
    let status: String = row.try_get("status").map_err(storage_error)?;
    Ok(FeedbackRequestSummary {
        request_id: row.try_get("id").map_err(storage_error)?,
        host_id: row.try_get("host_id").map_err(storage_error)?,
        host_session_id: row.try_get("host_session_id").map_err(storage_error)?,
        source_hint: row.try_get("source_hint").map_err(storage_error)?,
        title: row.try_get("title").map_err(storage_error)?,
        what_happened: row.try_get("what_happened").map_err(storage_error)?,
        status: FeedbackStatus::try_from(status.as_str())?,
        resolution: row
            .try_get::<Option<String>, _>("resolution")
            .map_err(storage_error)?
            .map(|value| FeedbackResolution::try_from(value.as_str()))
            .transpose()?,
        allow_finish: row.try_get("allow_finish").map_err(storage_error)?,
        final_summary: row.try_get("final_summary").map_err(storage_error)?,
        revision: row.try_get::<i64, _>("revision").map_err(storage_error)? as u64,
        created_at: row.try_get("created_at").map_err(storage_error)?,
        updated_at: row.try_get("updated_at").map_err(storage_error)?,
    })
}

pub(super) async fn ensure_attachment_mutable(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
    expected_revision: u64,
) -> Result<i64, RepositoryError> {
    let row = sqlx::query(
        "SELECT status, revision, \
                EXISTS(SELECT 1 FROM submission_plans WHERE request_id = ?1) AS planned \
         FROM feedback_requests WHERE id = ?1",
    )
    .bind(request_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(storage_error)?
    .ok_or(RepositoryError::RequestNotFound)?;
    let status: String = row.try_get("status").map_err(storage_error)?;
    if matches!(
        FeedbackStatus::try_from(status.as_str())?,
        FeedbackStatus::Completed | FeedbackStatus::Cancelled
    ) {
        return Err(RepositoryError::RequestTerminal);
    }
    let planned: bool = row.try_get("planned").map_err(storage_error)?;
    let current_revision: i64 = row.try_get("revision").map_err(storage_error)?;
    if planned || current_revision != expected_revision as i64 {
        return Err(RepositoryError::DraftConflict);
    }
    Ok(current_revision)
}

pub(super) async fn advance_attachment_revision(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
    current_revision: i64,
    now: &str,
) -> Result<(), RepositoryError> {
    let next_revision = current_revision + 1;
    let updated = sqlx::query(
        "UPDATE feedback_requests SET \
             status = 'in_progress', started_at = COALESCE(started_at, ?3), \
             updated_at = ?3, revision = ?2 \
         WHERE id = ?1 AND revision = ?4 AND status IN ('waiting', 'in_progress')",
    )
    .bind(request_id)
    .bind(next_revision)
    .bind(now)
    .bind(current_revision)
    .execute(&mut **transaction)
    .await
    .map_err(storage_error)?;
    if updated.rows_affected() != 1 {
        return Err(RepositoryError::DraftConflict);
    }
    sqlx::query(
        "INSERT INTO drafts (request_id, body_markdown, revision, updated_at) \
         VALUES (?1, '', ?2, ?3) \
         ON CONFLICT(request_id) DO UPDATE SET \
             revision = excluded.revision, updated_at = excluded.updated_at",
    )
    .bind(request_id)
    .bind(next_revision)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(storage_error)?;
    Ok(())
}

pub(super) async fn load_workspace_from_pool(
    pool: &SqlitePool,
    request_id: &str,
) -> Result<StoredFeedbackWorkspace, RepositoryError> {
    let row = sqlx::query(
        "SELECT r.id, hs.host_id, hs.host_session_id, r.source_hint, \
                r.title, r.what_happened, r.status, r.resolution, r.allow_finish, r.final_summary, \
                r.revision, r.created_at, r.updated_at, \
                fr.package_uri, fr.directory_path, fr.markdown_path, fr.manifest_path \
         FROM feedback_requests r \
         JOIN host_sessions hs ON hs.id = r.host_session_record_id \
         LEFT JOIN feedback_results fr ON fr.request_id = r.id \
         WHERE r.id = ?1",
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await
    .map_err(storage_error)?
    .ok_or(RepositoryError::RequestNotFound)?;
    let action_rows = sqlx::query(
        "SELECT action_id, instruction FROM request_actions \
         WHERE request_id = ?1 ORDER BY position",
    )
    .bind(request_id)
    .fetch_all(pool)
    .await
    .map_err(storage_error)?;
    let actions = action_rows
        .iter()
        .map(|row| {
            Ok(ActionInput {
                id: row.try_get("action_id").map_err(storage_error)?,
                instruction: row.try_get("instruction").map_err(storage_error)?,
            })
        })
        .collect::<Result<Vec<_>, RepositoryError>>()?;
    let context_rows = sqlx::query(
        "SELECT label, uri FROM request_context_refs \
         WHERE request_id = ?1 ORDER BY position",
    )
    .bind(request_id)
    .fetch_all(pool)
    .await
    .map_err(storage_error)?;
    let context_refs = context_rows
        .iter()
        .map(|row| {
            Ok(ContextRef {
                label: row.try_get("label").map_err(storage_error)?,
                uri: row.try_get("uri").map_err(storage_error)?,
            })
        })
        .collect::<Result<Vec<_>, RepositoryError>>()?;
    let request_attachment_rows = sqlx::query(
        "SELECT id, file_name, media_type, byte_size, sha256, position \
         FROM request_attachments WHERE request_id = ?1 ORDER BY position, id",
    )
    .bind(request_id)
    .fetch_all(pool)
    .await
    .map_err(storage_error)?;
    let request_attachments = request_attachment_rows
        .iter()
        .map(request_attachment_view_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let draft_row = sqlx::query(
        "SELECT document_json, body_markdown, revision, updated_at FROM drafts WHERE request_id = ?1",
    )
            .bind(request_id)
            .fetch_optional(pool)
            .await
            .map_err(storage_error)?;
    let draft = match draft_row {
        Some(row) => DraftView {
            document_json: row.try_get("document_json").map_err(storage_error)?,
            body_markdown: row.try_get("body_markdown").map_err(storage_error)?,
            saved_revision: row.try_get::<i64, _>("revision").map_err(storage_error)? as u64,
            updated_at: Some(row.try_get("updated_at").map_err(storage_error)?),
        },
        None => DraftView {
            document_json: None,
            body_markdown: String::new(),
            saved_revision: 0,
            updated_at: None,
        },
    };
    let attachment_rows = sqlx::query(
        "SELECT id, file_name, media_type, byte_size, sha256, position \
         FROM attachments WHERE request_id = ?1 ORDER BY position, id",
    )
    .bind(request_id)
    .fetch_all(pool)
    .await
    .map_err(storage_error)?;
    let attachments = attachment_rows
        .iter()
        .map(attachment_view_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StoredFeedbackWorkspace {
        request: summary_from_row(&row)?,
        actions,
        context_refs,
        request_attachments,
        draft,
        attachments,
        feedback: feedback_result_from_row(&row)?,
    })
}

fn request_attachment_view_from_row(
    row: &SqliteRow,
) -> Result<RequestAttachmentView, RepositoryError> {
    Ok(RequestAttachmentView {
        attachment_id: row.try_get("id").map_err(storage_error)?,
        file_name: row.try_get("file_name").map_err(storage_error)?,
        media_type: row.try_get("media_type").map_err(storage_error)?,
        byte_size: row.try_get::<i64, _>("byte_size").map_err(storage_error)? as u64,
        sha256: row.try_get("sha256").map_err(storage_error)?,
        position: row.try_get::<i64, _>("position").map_err(storage_error)? as u32,
    })
}

pub(super) fn attachment_view_from_row(row: &SqliteRow) -> Result<AttachmentView, RepositoryError> {
    Ok(AttachmentView {
        attachment_id: row.try_get("id").map_err(storage_error)?,
        file_name: row.try_get("file_name").map_err(storage_error)?,
        media_type: row.try_get("media_type").map_err(storage_error)?,
        byte_size: row.try_get::<i64, _>("byte_size").map_err(storage_error)? as u64,
        sha256: row.try_get("sha256").map_err(storage_error)?,
        position: row.try_get::<i64, _>("position").map_err(storage_error)? as u32,
    })
}

pub(super) async fn load_submission_row(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
) -> Result<Option<SqliteRow>, RepositoryError> {
    sqlx::query(
        "SELECT r.id, r.status, r.revision AS request_revision, r.title, r.what_happened, \
                r.cancel_reason AS request_cancel_reason, \
                r.source_hint, hs.host_id, hs.host_session_id, \
                d.body_markdown, d.revision AS draft_revision, \
                sp.publication_id, sp.source_revision, sp.body_sha256, sp.cooked_markdown, \
                sp.cooking_model, sp.uncooked_markdown AS plan_uncooked_markdown, \
                sp.terminal_resolution, sp.cancel_reason AS plan_cancel_reason, \
                sp.submitted_at, sp.package_uri, sp.directory_path, \
                sp.temp_directory_path, sp.markdown_path, sp.manifest_path \
         FROM feedback_requests r \
         JOIN host_sessions hs ON hs.id = r.host_session_record_id \
         LEFT JOIN drafts d ON d.request_id = r.id \
         LEFT JOIN submission_plans sp ON sp.request_id = r.id \
         WHERE r.id = ?1",
    )
    .bind(request_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(storage_error)
}

pub(super) async fn load_actions(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
) -> Result<Vec<ActionInput>, RepositoryError> {
    let rows = sqlx::query(
        "SELECT action_id, instruction FROM request_actions \
         WHERE request_id = ?1 ORDER BY position",
    )
    .bind(request_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(storage_error)?;
    rows.iter()
        .map(|row| {
            Ok(ActionInput {
                id: row.try_get("action_id").map_err(storage_error)?,
                instruction: row.try_get("instruction").map_err(storage_error)?,
            })
        })
        .collect()
}

pub(super) async fn load_submission_attachments(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
) -> Result<Vec<SubmissionAttachment>, RepositoryError> {
    let rows = sqlx::query(
        "SELECT id, draft_path, file_name, media_type, byte_size, sha256 \
         FROM attachments WHERE request_id = ?1 ORDER BY position, id",
    )
    .bind(request_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(storage_error)?;
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let file_name: String = row.try_get("file_name").map_err(storage_error)?;
            Ok(SubmissionAttachment {
                attachment_id: row.try_get("id").map_err(storage_error)?,
                relative_path: format!(
                    "attachments/{:03}-{}",
                    index + 1,
                    portable_file_name(&file_name)
                ),
                file_name,
                media_type: row.try_get("media_type").map_err(storage_error)?,
                byte_size: row.try_get::<i64, _>("byte_size").map_err(storage_error)? as u64,
                sha256: row.try_get("sha256").map_err(storage_error)?,
                draft_path: row.try_get("draft_path").map_err(storage_error)?,
            })
        })
        .collect()
}

pub(super) async fn load_submission_request_attachments(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request_id: &str,
) -> Result<Vec<SubmissionRequestAttachment>, RepositoryError> {
    let rows = sqlx::query(
        "SELECT id, file_name, media_type, byte_size, sha256, draft_path \
         FROM request_attachments WHERE request_id = ?1 ORDER BY position, id",
    )
    .bind(request_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(storage_error)?;
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let file_name: String = row.try_get("file_name").map_err(storage_error)?;
            Ok(SubmissionRequestAttachment {
                attachment_id: row.try_get("id").map_err(storage_error)?,
                relative_path: format!(
                    "request-attachments/{:03}-{}",
                    index + 1,
                    portable_file_name(&file_name)
                ),
                file_name,
                media_type: row.try_get("media_type").map_err(storage_error)?,
                byte_size: row.try_get::<i64, _>("byte_size").map_err(storage_error)? as u64,
                sha256: row.try_get("sha256").map_err(storage_error)?,
                draft_path: row.try_get("draft_path").map_err(storage_error)?,
            })
        })
        .collect()
}

pub(super) fn submission_plan_from_row(
    row: &SqliteRow,
    actions: Vec<ActionInput>,
    attachments: Vec<SubmissionAttachment>,
    request_attachments: Vec<SubmissionRequestAttachment>,
    body_markdown: String,
) -> Result<SubmissionPlan, RepositoryError> {
    let resolution_value: String = row.try_get("terminal_resolution").map_err(storage_error)?;
    let resolution = FeedbackResolution::try_from(resolution_value.as_str())?;
    Ok(SubmissionPlan {
        request_id: row.try_get("id").map_err(storage_error)?,
        host_id: row.try_get("host_id").map_err(storage_error)?,
        host_session_id: row.try_get("host_session_id").map_err(storage_error)?,
        source_hint: row.try_get("source_hint").map_err(storage_error)?,
        title: row.try_get("title").map_err(storage_error)?,
        what_happened: row.try_get("what_happened").map_err(storage_error)?,
        actions,
        attachments,
        request_attachments,
        body_markdown: row
            .try_get::<Option<String>, _>("cooked_markdown")
            .map_err(storage_error)?
            .unwrap_or_else(|| body_markdown.clone()),
        uncooked_markdown: row
            .try_get::<Option<String>, _>("plan_uncooked_markdown")
            .map_err(storage_error)?
            .unwrap_or(body_markdown),
        cooking_model: row.try_get("cooking_model").map_err(storage_error)?,
        resolution,
        cancel_reason: row.try_get("plan_cancel_reason").map_err(storage_error)?,
        source_revision: row
            .try_get::<i64, _>("source_revision")
            .map_err(storage_error)? as u64,
        publication_id: row.try_get("publication_id").map_err(storage_error)?,
        body_sha256: row.try_get("body_sha256").map_err(storage_error)?,
        submitted_at: row.try_get("submitted_at").map_err(storage_error)?,
        package_uri: row.try_get("package_uri").map_err(storage_error)?,
        directory_path: row.try_get("directory_path").map_err(storage_error)?,
        temp_directory_path: row.try_get("temp_directory_path").map_err(storage_error)?,
        markdown_path: row.try_get("markdown_path").map_err(storage_error)?,
        manifest_path: row.try_get("manifest_path").map_err(storage_error)?,
    })
}
