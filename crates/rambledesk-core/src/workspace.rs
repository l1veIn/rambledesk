use sha2::{Digest, Sha256};

use crate::{
    ApplicationError, FeedbackApplication, FeedbackStatus, RepositoryError, StoredFeedbackRequest,
};

mod model;

pub use model::*;

impl FeedbackApplication {
    pub async fn read_feedback_package(
        &self,
        request: &crate::FeedbackRequestView,
    ) -> Result<Option<FeedbackPackageContent>, ApplicationError> {
        let Some(result) = request.feedback.as_ref() else {
            return Ok(None);
        };
        self.package_reader
            .read(&request.request_id, result)
            .await
            .map(Some)
            .map_err(ApplicationError::from)
    }

    pub async fn list_host_sessions(&self) -> Result<Vec<HostSessionSummary>, ApplicationError> {
        self.repository
            .list_host_sessions()
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn list_open_feedback_requests(
        &self,
    ) -> Result<Vec<FeedbackRequestSummary>, ApplicationError> {
        self.repository
            .list_open_requests()
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn list_feedback_requests(
        &self,
        input: ListFeedbackRequestsInput,
    ) -> Result<ListFeedbackRequestsOutput, ApplicationError> {
        if let Some(host_id) = input.host_id.as_deref() {
            crate::feedback::validate_text("host_id", host_id, 1, 200)?;
        }
        if let Some(host_session_id) = input.host_session_id.as_deref() {
            crate::feedback::validate_text("host_session_id", host_session_id, 1, 200)?;
        }
        let statuses = input
            .status
            .unwrap_or_else(|| vec![FeedbackStatus::Waiting, FeedbackStatus::InProgress]);
        if statuses.is_empty() {
            return Err(ApplicationError::invalid_argument(
                "status must contain at least one value",
            ));
        }
        let limit = input.limit.unwrap_or(50);
        if !(1..=100).contains(&limit) {
            return Err(ApplicationError::invalid_argument(
                "limit must be between 1 and 100",
            ));
        }
        let cursor = input
            .cursor
            .as_deref()
            .map(decode_list_cursor)
            .transpose()?;
        let mut requests = self
            .repository
            .list_requests(FeedbackRequestQuery {
                host_id: input.host_id,
                host_session_id: input.host_session_id,
                statuses,
                limit,
                before_updated_at: cursor.as_ref().map(|value| value.updated_at.clone()),
                before_request_id: cursor.as_ref().map(|value| value.request_id.clone()),
            })
            .await
            .map_err(ApplicationError::from)?;
        let has_more = requests.len() > limit as usize;
        requests.truncate(limit as usize);
        let next_cursor = if has_more {
            requests.last().map(encode_list_cursor).transpose()?
        } else {
            None
        };
        Ok(ListFeedbackRequestsOutput {
            requests,
            next_cursor,
        })
    }

    pub async fn get_feedback_workspace(
        &self,
        request_id: String,
    ) -> Result<FeedbackWorkspaceView, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&request_id, "request_id")?;
        self.repository
            .get_workspace(&request_id)
            .await
            .map(Into::into)
            .map_err(ApplicationError::from)
    }

    pub async fn save_feedback_draft(
        &self,
        input: SaveDraftInput,
    ) -> Result<DraftView, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&input.request_id, "request_id")?;
        crate::feedback::validate_text("body_markdown", &input.body_markdown, 0, 100_000)?;
        self.repository
            .save_draft(
                &request_id,
                &input.body_markdown,
                input.expected_revision,
                &self.clock.now_rfc3339(),
            )
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn add_feedback_attachment(
        &self,
        input: AddAttachmentInput,
    ) -> Result<FeedbackWorkspaceView, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&input.request_id, "request_id")?;
        let mut file_name = validate_file_name(&input.file_name)?;
        if input.contents.is_empty() {
            return Err(ApplicationError::invalid_argument(
                "attachment contents cannot be empty",
            ));
        }
        if input.contents.len() > MAX_ATTACHMENT_BYTES {
            return Err(ApplicationError::invalid_argument(format!(
                "attachment exceeds the {} MiB limit",
                MAX_ATTACHMENT_BYTES / 1024 / 1024
            )));
        }
        let media_type = detect_image_media_type(&input.contents).ok_or_else(|| {
            ApplicationError::invalid_argument("attachment must be a PNG, JPEG, GIF, or WebP image")
        })?;
        file_name = normalize_image_file_name(&file_name, media_type);
        let sha256 = hex::encode(Sha256::digest(&input.contents));
        self.repository
            .add_attachment(
                &request_id,
                NewAttachment {
                    attachment_id: self.ids.new_id(),
                    file_name,
                    media_type: media_type.to_owned(),
                    contents: input.contents,
                    sha256,
                },
                input.expected_revision,
                &self.clock.now_rfc3339(),
            )
            .await
            .map(Into::into)
            .map_err(ApplicationError::from)
    }

    pub async fn remove_feedback_attachment(
        &self,
        input: RemoveAttachmentInput,
    ) -> Result<FeedbackWorkspaceView, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&input.request_id, "request_id")?;
        let attachment_id = crate::feedback::canonical_uuid(&input.attachment_id, "attachment_id")?;
        self.repository
            .remove_attachment(
                &request_id,
                &attachment_id,
                input.expected_revision,
                &self.clock.now_rfc3339(),
            )
            .await
            .map(Into::into)
            .map_err(ApplicationError::from)
    }

    pub async fn reorder_feedback_attachments(
        &self,
        input: ReorderAttachmentsInput,
    ) -> Result<FeedbackWorkspaceView, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&input.request_id, "request_id")?;
        let attachment_ids = input
            .attachment_ids
            .iter()
            .map(|id| crate::feedback::canonical_uuid(id, "attachment_ids"))
            .collect::<Result<Vec<_>, _>>()?;
        self.repository
            .reorder_attachments(
                &request_id,
                &attachment_ids,
                input.expected_revision,
                &self.clock.now_rfc3339(),
            )
            .await
            .map(Into::into)
            .map_err(ApplicationError::from)
    }

    pub async fn read_feedback_attachment(
        &self,
        request_id: String,
        attachment_id: String,
    ) -> Result<Vec<u8>, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&request_id, "request_id")?;
        let attachment_id = crate::feedback::canonical_uuid(&attachment_id, "attachment_id")?;
        self.repository
            .read_attachment(&request_id, &attachment_id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn submit_feedback(
        &self,
        input: SubmitFeedbackInput,
    ) -> Result<crate::FeedbackRequestView, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&input.request_id, "request_id")?;
        let existing = self
            .repository
            .get_request(&request_id)
            .await
            .map_err(ApplicationError::from)?;
        if existing.status == FeedbackStatus::Completed {
            self.notify_feedback_terminal(&request_id);
            return Ok(existing.into());
        }
        let now = self.clock.now_rfc3339();
        let plan_result = self
            .repository
            .plan_submission(
                &request_id,
                input.expected_revision,
                &self.ids.new_id(),
                &now,
            )
            .await;
        let plan = match plan_result {
            Ok(plan) => plan,
            Err(RepositoryError::RequestTerminal) => {
                let raced = self
                    .repository
                    .get_request(&request_id)
                    .await
                    .map_err(ApplicationError::from)?;
                if raced.status == FeedbackStatus::Completed {
                    self.notify_feedback_terminal(&request_id);
                    return Ok(raced.into());
                }
                return Err(ApplicationError::from(RepositoryError::RequestTerminal));
            }
            Err(error) => return Err(ApplicationError::from(error)),
        };
        let published = self
            .publisher
            .publish(&plan)
            .await
            .map_err(ApplicationError::from)?;
        let stored: StoredFeedbackRequest = self
            .repository
            .complete_submission(&plan, &published)
            .await
            .map_err(ApplicationError::from)?;
        self.notify_feedback_terminal(&request_id);
        Ok(stored.into())
    }
}

mod validation;

use validation::{
    decode_list_cursor, detect_image_media_type, encode_list_cursor, normalize_image_file_name,
    validate_file_name,
};
