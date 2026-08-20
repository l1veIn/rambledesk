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
            .list_host_sessions(HostSessionQuery {
                archived: false,
                search: None,
            })
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn list_archived_host_sessions(
        &self,
        input: ListHostSessionsInput,
    ) -> Result<Vec<HostSessionSummary>, ApplicationError> {
        let search = validate_optional_search(input.search)?;
        self.repository
            .list_host_sessions(HostSessionQuery {
                archived: true,
                search,
            })
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn rename_host_session(
        &self,
        input: RenameHostSessionInput,
    ) -> Result<HostSessionSummary, ApplicationError> {
        let (host_id, host_session_id) =
            validate_host_session_identity(input.host_id, input.host_session_id)?;
        let title = input.title.trim().to_owned();
        crate::feedback::validate_text("title", &title, 1, 160)?;
        self.repository
            .rename_host_session(
                &host_id,
                &host_session_id,
                &title,
                &self.clock.now_rfc3339(),
            )
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn set_host_session_pinned(
        &self,
        input: SetHostSessionPinnedInput,
    ) -> Result<HostSessionSummary, ApplicationError> {
        let (host_id, host_session_id) =
            validate_host_session_identity(input.host_id, input.host_session_id)?;
        let pinned_at = input.pinned.then(|| self.clock.now_rfc3339());
        self.repository
            .set_host_session_pinned(&host_id, &host_session_id, pinned_at.as_deref())
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn archive_host_session(
        &self,
        input: HostSessionInput,
    ) -> Result<HostSessionSummary, ApplicationError> {
        let (host_id, host_session_id) =
            validate_host_session_identity(input.host_id, input.host_session_id)?;
        self.repository
            .archive_host_session(&host_id, &host_session_id, &self.clock.now_rfc3339())
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn unarchive_host_session(
        &self,
        input: HostSessionInput,
    ) -> Result<HostSessionSummary, ApplicationError> {
        let (host_id, host_session_id) =
            validate_host_session_identity(input.host_id, input.host_session_id)?;
        self.repository
            .unarchive_host_session(&host_id, &host_session_id, &self.clock.now_rfc3339())
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn delete_host_session(
        &self,
        input: HostSessionInput,
    ) -> Result<(), ApplicationError> {
        let (host_id, host_session_id) =
            validate_host_session_identity(input.host_id, input.host_session_id)?;
        self.repository
            .delete_host_session(&host_id, &host_session_id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn delete_feedback_request(
        &self,
        input: DeleteFeedbackRequestInput,
    ) -> Result<(), ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&input.request_id, "request_id")?;
        self.repository
            .delete_feedback_request(&request_id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn set_host_pinned(
        &self,
        input: SetHostPinnedInput,
    ) -> Result<Vec<HostSessionSummary>, ApplicationError> {
        let host_id = validate_host_id(input.host_id)?;
        let pinned_at = input.pinned.then(|| self.clock.now_rfc3339());
        let now = self.clock.now_rfc3339();
        self.repository
            .set_host_pinned(&host_id, pinned_at.as_deref(), &now)
            .await
            .map_err(ApplicationError::from)?;
        self.list_host_sessions().await
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
        let search = validate_optional_search(input.search)?;
        let mut requests = self
            .repository
            .list_requests(FeedbackRequestQuery {
                host_id: input.host_id,
                host_session_id: input.host_session_id,
                statuses,
                archived: input.archived.unwrap_or(false),
                search,
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
        let media_type = detect_media_type(&input.contents, &file_name);
        if media_type.starts_with("image/") {
            file_name = normalize_image_file_name(&file_name, media_type);
        }
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

    pub async fn read_request_attachment(
        &self,
        request_id: String,
        attachment_id: String,
    ) -> Result<Vec<u8>, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&request_id, "request_id")?;
        let attachment_id = crate::feedback::canonical_uuid(&attachment_id, "attachment_id")?;
        self.repository
            .read_request_attachment(&request_id, &attachment_id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn resolve_feedback_attachment_path(
        &self,
        request_id: String,
        attachment_id: String,
    ) -> Result<String, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&request_id, "request_id")?;
        let attachment_id = crate::feedback::canonical_uuid(&attachment_id, "attachment_id")?;
        self.repository
            .resolve_attachment_path(&request_id, &attachment_id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn resolve_request_attachment_path(
        &self,
        request_id: String,
        attachment_id: String,
    ) -> Result<String, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&request_id, "request_id")?;
        let attachment_id = crate::feedback::canonical_uuid(&attachment_id, "attachment_id")?;
        self.repository
            .resolve_request_attachment_path(&request_id, &attachment_id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn submit_feedback(
        &self,
        input: SubmitFeedbackInput,
    ) -> Result<crate::FeedbackRequestView, ApplicationError> {
        let request_id = crate::feedback::canonical_uuid(&input.request_id, "request_id")?;
        match (&input.cooked_markdown, &input.cooking_model) {
            (Some(markdown), Some(model)) => {
                crate::feedback::validate_text("cooked_markdown", markdown, 1, 100_000)?;
                crate::feedback::validate_text("cooking_model", model, 1, 500)?;
            }
            (None, None) => {}
            _ => {
                return Err(ApplicationError::invalid_argument(
                    "cooked_markdown and cooking_model must be provided together",
                ));
            }
        }
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
                input.cooked_markdown.as_deref(),
                input.cooking_model.as_deref(),
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

fn validate_host_session_identity(
    host_id: String,
    host_session_id: String,
) -> Result<(String, String), ApplicationError> {
    let host_id = validate_host_id(host_id)?;
    let host_session_id = host_session_id.trim().to_owned();
    crate::feedback::validate_text("host_session_id", &host_session_id, 1, 200)?;
    Ok((host_id, host_session_id))
}

fn validate_host_id(host_id: String) -> Result<String, ApplicationError> {
    let host_id = host_id.trim().to_owned();
    crate::feedback::validate_text("host_id", &host_id, 1, 200)?;
    Ok(host_id)
}

fn validate_optional_search(search: Option<String>) -> Result<Option<String>, ApplicationError> {
    let Some(search) = search else {
        return Ok(None);
    };
    let search = search.trim().to_owned();
    if search.is_empty() {
        return Ok(None);
    }
    crate::feedback::validate_text("search", &search, 1, 200)?;
    Ok(Some(search))
}

pub(crate) mod validation;

use validation::{
    decode_list_cursor, detect_media_type, encode_list_cursor, normalize_image_file_name,
    validate_file_name,
};
