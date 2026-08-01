use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::{
    ActionInput, ApplicationError, ContextRef, FeedbackApplication, FeedbackResultView,
    FeedbackStatus, RepositoryError, StoredFeedbackRequest,
};

pub const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_ATTACHMENT_COUNT: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FeedbackRequestSummary {
    pub request_id: String,
    pub project_id: String,
    pub project_name: String,
    pub agent: String,
    pub session_id: String,
    pub what_happened: String,
    pub status: FeedbackStatus,
    #[ts(type = "number")]
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS, Default)]
pub struct ListFeedbackRequestsInput {
    pub project_id: Option<String>,
    pub agent: Option<String>,
    pub session_id: Option<String>,
    pub status: Option<Vec<FeedbackStatus>>,
    #[ts(type = "number | null")]
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ListFeedbackRequestsOutput {
    pub requests: Vec<FeedbackRequestSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackRequestQuery {
    pub project_id: Option<String>,
    pub agent: Option<String>,
    pub session_id: Option<String>,
    pub statuses: Vec<FeedbackStatus>,
    pub limit: u32,
    pub before_updated_at: Option<String>,
    pub before_request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct DraftView {
    pub body_markdown: String,
    #[ts(type = "number")]
    pub saved_revision: u64,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FeedbackWorkspaceView {
    pub request: FeedbackRequestSummary,
    pub actions: Vec<ActionInput>,
    pub context_refs: Vec<ContextRef>,
    pub draft: DraftView,
    pub attachments: Vec<AttachmentView>,
    pub feedback: Option<FeedbackResultView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AttachmentView {
    pub attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    #[ts(type = "number")]
    pub byte_size: u64,
    pub sha256: String,
    #[ts(type = "number")]
    pub position: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AddAttachmentInput {
    pub request_id: String,
    pub file_name: String,
    #[ts(type = "number[]")]
    pub contents: Vec<u8>,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct RemoveAttachmentInput {
    pub request_id: String,
    pub attachment_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ReorderAttachmentsInput {
    pub request_id: String,
    pub attachment_ids: Vec<String>,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SaveDraftInput {
    pub request_id: String,
    pub body_markdown: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SubmitFeedbackInput {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFeedbackWorkspace {
    pub request: FeedbackRequestSummary,
    pub actions: Vec<ActionInput>,
    pub context_refs: Vec<ContextRef>,
    pub draft: DraftView,
    pub attachments: Vec<AttachmentView>,
    pub feedback: Option<FeedbackResultView>,
}

impl From<StoredFeedbackWorkspace> for FeedbackWorkspaceView {
    fn from(value: StoredFeedbackWorkspace) -> Self {
        Self {
            request: value.request,
            actions: value.actions,
            context_refs: value.context_refs,
            draft: value.draft,
            attachments: value.attachments,
            feedback: value.feedback,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAttachment {
    pub attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    pub contents: Vec<u8>,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionAttachment {
    pub attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    pub byte_size: u64,
    pub sha256: String,
    pub draft_path: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionPlan {
    pub request_id: String,
    pub project_id: String,
    pub agent: String,
    pub session_id: String,
    pub what_happened: String,
    pub actions: Vec<ActionInput>,
    pub attachments: Vec<SubmissionAttachment>,
    pub body_markdown: String,
    pub source_revision: u64,
    pub publication_id: String,
    pub body_sha256: String,
    pub submitted_at: String,
    pub package_uri: String,
    pub directory_path: String,
    pub temp_directory_path: String,
    pub markdown_path: String,
    pub manifest_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedFeedbackPackage {
    pub result: FeedbackResultView,
    pub manifest_sha256: String,
    pub published_at: String,
}

#[async_trait]
pub trait FeedbackPackagePublisher: Send + Sync {
    async fn publish(
        &self,
        plan: &SubmissionPlan,
    ) -> Result<PublishedFeedbackPackage, RepositoryError>;
}

impl FeedbackApplication {
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
        let project_id = input
            .project_id
            .as_deref()
            .map(|value| crate::feedback::canonical_uuid(value, "project_id"))
            .transpose()?;
        if let Some(agent) = input.agent.as_deref() {
            crate::feedback::validate_text("agent", agent, 1, 200)?;
        }
        if let Some(session_id) = input.session_id.as_deref() {
            crate::feedback::validate_text("session_id", session_id, 1, 200)?;
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
                project_id,
                agent: input.agent,
                session_id: input.session_id,
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

#[derive(Debug, Serialize, Deserialize)]
struct ListCursor {
    updated_at: String,
    request_id: String,
}

fn encode_list_cursor(summary: &FeedbackRequestSummary) -> Result<String, ApplicationError> {
    serde_json::to_vec(&ListCursor {
        updated_at: summary.updated_at.clone(),
        request_id: summary.request_id.clone(),
    })
    .map(hex::encode)
    .map_err(|_| ApplicationError::invalid_argument("cursor could not be encoded"))
}

fn decode_list_cursor(value: &str) -> Result<ListCursor, ApplicationError> {
    let bytes =
        hex::decode(value).map_err(|_| ApplicationError::invalid_argument("cursor is invalid"))?;
    let cursor: ListCursor = serde_json::from_slice(&bytes)
        .map_err(|_| ApplicationError::invalid_argument("cursor is invalid"))?;
    let request_id = crate::feedback::canonical_uuid(&cursor.request_id, "cursor")?;
    if cursor.updated_at.is_empty() {
        return Err(ApplicationError::invalid_argument("cursor is invalid"));
    }
    Ok(ListCursor {
        updated_at: cursor.updated_at,
        request_id,
    })
}

fn validate_file_name(value: &str) -> Result<String, ApplicationError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 255 {
        return Err(ApplicationError::invalid_argument(
            "file_name must contain 1 to 255 characters",
        ));
    }
    if value.contains(['/', '\\', '\0']) || value == "." || value == ".." {
        return Err(ApplicationError::invalid_argument(
            "file_name must be a plain file name",
        ));
    }
    Ok(value.to_owned())
}

fn detect_image_media_type(contents: &[u8]) -> Option<&'static str> {
    if contents.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if contents.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if contents.starts_with(b"GIF87a") || contents.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if contents.len() >= 12
        && contents.starts_with(b"RIFF")
        && contents.get(8..12) == Some(b"WEBP")
    {
        Some("image/webp")
    } else {
        None
    }
}

fn normalize_image_file_name(file_name: &str, media_type: &str) -> String {
    let allowed_extensions: &[&str] = match media_type {
        "image/png" => &["png"],
        "image/jpeg" => &["jpg", "jpeg", "jfif"],
        "image/gif" => &["gif"],
        "image/webp" => &["webp"],
        _ => &[],
    };
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    if extension
        .as_deref()
        .is_some_and(|extension| allowed_extensions.contains(&extension))
    {
        return file_name.to_owned();
    }
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .filter(|stem| !stem.is_empty())
        .unwrap_or(file_name);
    format!("{stem}.{}", allowed_extensions.first().unwrap_or(&"image"))
}
