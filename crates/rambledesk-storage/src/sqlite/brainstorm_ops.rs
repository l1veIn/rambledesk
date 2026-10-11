//! Resolve a new round from immutable published feedback, within request creation's transaction.
use super::*;
use rambledesk_core::{
    BrainstormData, BrainstormWireData, FeedbackPackageReader, WorkbenchData, WorkbenchResult,
    WorkbenchState, initial_brainstorm_state, resolve_brainstorm_request,
    workbench_result_complete,
};

impl SqliteFeedbackStore {
    pub(super) async fn resolve_brainstorm_input(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        request: &NewFeedbackRequest,
    ) -> Result<Option<BrainstormData>, RepositoryError> {
        let Some(spec) = &request.workbench else {
            return Ok(None);
        };
        if spec.kind != "brainstorm" {
            return Ok(None);
        }
        let WorkbenchData::Brainstorm(BrainstormWireData::Request(input)) = &spec.data else {
            return Err(RepositoryError::WorkbenchIncomplete);
        };
        if spec.version != 1 {
            return Err(RepositoryError::WorkbenchUnsupported);
        }
        let parent = if let Some(parent_id) = &input.inherit_from_request_id {
            if parent_id == &request.request_id {
                return Err(RepositoryError::RequestConflict);
            }
            let row = load_request_row(transaction, parent_id)
                .await?
                .ok_or(RepositoryError::RequestNotFound)?;
            let stored = stored_request_from_row(&row)?;
            // External credentials already read arbitrary requests. The host pair is a
            // correlation guard; managed ownership comes only from the trusted binding.
            if stored.host_id != request.host_id
                || stored.host_session_id != request.host_session_id
                || stored.managed_session_id != request.managed_session_id
            {
                return Err(RepositoryError::RequestNotFound);
            }
            if matches!(
                stored.status,
                FeedbackStatus::Waiting | FeedbackStatus::InProgress
            ) {
                return Err(RepositoryError::RequestNotTerminal);
            }
            if stored.status != FeedbackStatus::Completed
                || stored.resolution != Some(FeedbackResolution::FeedbackSubmitted)
            {
                return Err(RepositoryError::WorkbenchIncomplete);
            }
            let published = stored.feedback.ok_or(RepositoryError::CorruptData)?;
            let package = FeedbackPackageReader::read(self, parent_id, &published).await?;
            let workbench = package
                .manifest
                .workbench
                .ok_or(RepositoryError::WorkbenchIncomplete)?;
            if workbench.input.kind != "brainstorm"
                || workbench.input.version != 1
                || !workbench_result_complete(&workbench)
            {
                return Err(RepositoryError::WorkbenchIncomplete);
            }
            let Some(WorkbenchResult::Brainstorm(result)) = workbench.result else {
                return Err(RepositoryError::WorkbenchIncomplete);
            };
            if result.source_version != *parent_id {
                return Err(RepositoryError::CorruptData);
            }
            Some(result)
        } else {
            None
        };
        resolve_brainstorm_request(input, &request.request_id, parent.as_ref())
            .map(Some)
            .map_err(|_| RepositoryError::WorkbenchIncomplete)
    }

    /// Called only after inserting the new request; retries return before this stage.
    pub(super) async fn seed_brainstorm_draft(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        request: &mut NewFeedbackRequest,
        mut data: BrainstormData,
    ) -> Result<Vec<PathBuf>, RepositoryError> {
        let copied = if let Some(parent_id) = data.inherit_from_request_id.clone() {
            self.inherit_brainstorm_attachments(
                transaction,
                &parent_id,
                &request.request_id,
                &request.created_at,
                &mut data,
            )
            .await?
        } else {
            vec![]
        };
        let save = async {
            let state = WorkbenchState::Brainstorm(initial_brainstorm_state(&data));
            let spec = request.workbench.as_mut().ok_or(RepositoryError::CorruptData)?;
            spec.data = WorkbenchData::Brainstorm(BrainstormWireData::Resolved(data));
            let document = serde_json::json!({"schemaVersion":2,
                "doc":{"type":"doc","content":[]},"workbenchState":state}).to_string();
            // Keep the exact hydrated input and seed consistent, including rewritten references.
            sqlx::query("UPDATE feedback_requests SET workbench_json = ?2 WHERE id = ?1")
                .bind(&request.request_id)
                .bind(serde_json::to_string(spec).map_err(storage_error)?)
                .execute(&mut **transaction).await.map_err(storage_error)?;
            sqlx::query("INSERT INTO drafts (request_id, document_json, body_markdown, revision, updated_at) VALUES (?1, ?2, '', 0, ?3)")
                .bind(&request.request_id).bind(document).bind(&request.created_at)
                .execute(&mut **transaction).await.map_err(storage_error)?;
            Ok::<(), RepositoryError>(())
        }.await;
        if let Err(error) = save {
            for path in &copied {
                let _ = tokio::fs::remove_file(path).await;
            }
            return Err(error);
        }
        Ok(copied)
    }
}
