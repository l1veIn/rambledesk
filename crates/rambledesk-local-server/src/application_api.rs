use std::sync::Arc;

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, FromRequest, Multipart, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Response, StatusCode, header},
    response::IntoResponse,
    routing::post,
};
use rambledesk_core::{
    AddAttachmentInput, ApplicationCommandFacade, ApplicationError, ApplicationHostProfileView,
    ApproveFeedbackInput, CancelFeedbackInput, DeleteFeedbackRequestInput, GetFeedbackInput,
    HostSessionInput, ListFeedbackRequestsInput, ListHostSessionsInput, MAX_ATTACHMENT_BYTES,
    ReadAttachmentInput, RemoveAttachmentInput, RenameHostSessionInput, ReorderAttachmentsInput,
    SaveDraftInput, SetHostPinnedInput, SetHostSessionPinnedInput, SubmitFeedbackInput,
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{api_error_response, application_error_status};

#[derive(Clone)]
struct ApplicationApiState {
    commands: Arc<ApplicationCommandFacade>,
}

const MULTIPART_METADATA_ALLOWANCE_BYTES: usize = 64 * 1024;
const MAX_ATTACHMENT_UPLOAD_BODY_BYTES: usize =
    MAX_ATTACHMENT_BYTES + MULTIPART_METADATA_ALLOWANCE_BYTES;
const X_CONTENT_TYPE_OPTIONS: HeaderName = HeaderName::from_static("x-content-type-options");

struct ApplicationJson<T>(T);

impl<S, T> FromRequest<S> for ApplicationJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Response<Body>;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(request, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(|rejection| {
                invalid_argument_response(format!(
                    "invalid JSON request body: {}",
                    rejection.body_text()
                ))
            })
    }
}

struct ApplicationMultipart(Multipart);

impl<S> FromRequest<S> for ApplicationMultipart
where
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        Multipart::from_request(request, state)
            .await
            .map(Self)
            .map_err(|rejection| {
                invalid_argument_response(format!(
                    "invalid multipart request body: {}",
                    rejection.body_text()
                ))
            })
    }
}

pub fn application_router(commands: Arc<ApplicationCommandFacade>) -> Router {
    Router::new()
        .route("/application/listFeedbackInbox", post(list_feedback_inbox))
        .route("/application/listHostSessions", post(list_host_sessions))
        .route(
            "/application/listArchivedHostSessions",
            post(list_archived_host_sessions),
        )
        .route("/application/listHostProfiles", post(list_host_profiles))
        .route(
            "/application/listFeedbackRequests",
            post(list_feedback_requests),
        )
        .route(
            "/application/getFeedbackWorkspace",
            post(get_feedback_workspace),
        )
        .route(
            "/application/readPublishedFeedback",
            post(read_published_feedback),
        )
        .route("/application/saveFeedbackDraft", post(save_feedback_draft))
        .route(
            "/application/addFeedbackAttachment",
            post(add_feedback_attachment)
                .layer(DefaultBodyLimit::max(MAX_ATTACHMENT_UPLOAD_BODY_BYTES)),
        )
        .route(
            "/application/removeFeedbackAttachment",
            post(remove_feedback_attachment),
        )
        .route(
            "/application/reorderFeedbackAttachments",
            post(reorder_feedback_attachments),
        )
        .route("/application/submitFeedback", post(submit_feedback))
        .route(
            "/application/approveFeedbackRequest",
            post(approve_feedback_request),
        )
        .route(
            "/application/cancelFeedbackRequest",
            post(cancel_feedback_request),
        )
        .route("/application/renameHostSession", post(rename_host_session))
        .route(
            "/application/setHostSessionPinned",
            post(set_host_session_pinned),
        )
        .route(
            "/application/archiveHostSession",
            post(archive_host_session),
        )
        .route(
            "/application/unarchiveHostSession",
            post(unarchive_host_session),
        )
        .route("/application/deleteHostSession", post(delete_host_session))
        .route(
            "/application/deleteFeedbackRequest",
            post(delete_feedback_request),
        )
        .route("/application/setHostPinned", post(set_host_pinned))
        .route(
            "/application/readFeedbackAttachment",
            post(read_feedback_attachment),
        )
        .route(
            "/application/readRequestAttachment",
            post(read_request_attachment),
        )
        .with_state(ApplicationApiState { commands })
}

fn invalid_argument_response(message: impl Into<String>) -> Response<Body> {
    let error = ApplicationError::invalid_argument(message);
    api_error_response(application_error_status(error.code_enum()), error)
}

fn application_result<T: Serialize>(result: Result<T, ApplicationError>) -> Response<Body> {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => api_error_response(application_error_status(error.code_enum()), error),
    }
}

fn application_void_result(result: Result<(), ApplicationError>) -> Response<Body> {
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => api_error_response(application_error_status(error.code_enum()), error),
    }
}

async fn list_feedback_inbox(State(state): State<ApplicationApiState>) -> Response<Body> {
    application_result(state.commands.list_feedback_inbox().await)
}

async fn list_host_sessions(State(state): State<ApplicationApiState>) -> Response<Body> {
    application_result(state.commands.list_host_sessions().await)
}

async fn list_archived_host_sessions(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ListHostSessionsInput>,
) -> Response<Body> {
    application_result(state.commands.list_archived_host_sessions(input).await)
}

async fn list_host_profiles(
    State(state): State<ApplicationApiState>,
) -> Json<Vec<ApplicationHostProfileView>> {
    Json(state.commands.list_host_profiles())
}

async fn list_feedback_requests(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ListFeedbackRequestsInput>,
) -> Response<Body> {
    application_result(state.commands.list_feedback_requests(input).await)
}

async fn get_feedback_workspace(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<GetFeedbackInput>,
) -> Response<Body> {
    application_result(state.commands.get_feedback_workspace(input).await)
}

async fn read_published_feedback(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<GetFeedbackInput>,
) -> Response<Body> {
    application_result(state.commands.read_published_feedback(input).await)
}

async fn save_feedback_draft(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<SaveDraftInput>,
) -> Response<Body> {
    application_result(state.commands.save_feedback_draft(input).await)
}

async fn add_feedback_attachment(
    State(state): State<ApplicationApiState>,
    ApplicationMultipart(mut multipart): ApplicationMultipart,
) -> Response<Body> {
    let mut request_id = None;
    let mut file_name = None;
    let mut expected_revision = None;
    let mut contents = None;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(error) => {
                return invalid_argument_response(format!(
                    "invalid multipart request body: {error}"
                ));
            }
        };
        let Some(name) = field.name().map(str::to_owned) else {
            return invalid_argument_response("multipart field name is required");
        };
        match name.as_str() {
            "request_id" => match take_text_field(field, &name, &mut request_id).await {
                Ok(()) => {}
                Err(response) => return response,
            },
            "file_name" => match take_text_field(field, &name, &mut file_name).await {
                Ok(()) => {}
                Err(response) => return response,
            },
            "expected_revision" => {
                let mut raw_revision = None;
                if let Err(response) = take_text_field(field, &name, &mut raw_revision).await {
                    return response;
                }
                expected_revision = match raw_revision
                    .expect("text field helper sets the value")
                    .parse::<u64>()
                {
                    Ok(value) if expected_revision.is_none() => Some(value),
                    Ok(_) => return invalid_argument_response("duplicate expected_revision field"),
                    Err(_) => {
                        return invalid_argument_response(
                            "expected_revision must be an unsigned integer",
                        );
                    }
                };
            }
            "file" => {
                if contents.is_some() {
                    return invalid_argument_response("duplicate file field");
                }
                match field.bytes().await {
                    Ok(value) => contents = Some(value.to_vec()),
                    Err(error) => {
                        return invalid_argument_response(format!(
                            "invalid multipart file field: {error}"
                        ));
                    }
                }
            }
            _ => {
                return invalid_argument_response(format!("unsupported multipart field: {name}"));
            }
        }
    }

    let Some(request_id) = request_id else {
        return invalid_argument_response("missing request_id field");
    };
    let Some(file_name) = file_name else {
        return invalid_argument_response("missing file_name field");
    };
    let Some(expected_revision) = expected_revision else {
        return invalid_argument_response("missing expected_revision field");
    };
    let Some(contents) = contents else {
        return invalid_argument_response("missing file field");
    };

    application_result(
        state
            .commands
            .add_feedback_attachment(AddAttachmentInput {
                request_id,
                file_name,
                contents,
                expected_revision,
            })
            .await,
    )
}

async fn take_text_field(
    field: axum::extract::multipart::Field<'_>,
    name: &str,
    destination: &mut Option<String>,
) -> Result<(), Response<Body>> {
    if destination.is_some() {
        return Err(invalid_argument_response(format!("duplicate {name} field")));
    }
    match field.text().await {
        Ok(value) => {
            *destination = Some(value);
            Ok(())
        }
        Err(error) => Err(invalid_argument_response(format!(
            "invalid multipart {name} field: {error}"
        ))),
    }
}

async fn remove_feedback_attachment(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<RemoveAttachmentInput>,
) -> Response<Body> {
    application_result(state.commands.remove_feedback_attachment(input).await)
}

async fn reorder_feedback_attachments(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ReorderAttachmentsInput>,
) -> Response<Body> {
    application_result(state.commands.reorder_feedback_attachments(input).await)
}

fn attachment_bytes_response(result: Result<Vec<u8>, ApplicationError>) -> Response<Body> {
    match result {
        Ok(contents) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
            (headers, Bytes::from(contents)).into_response()
        }
        Err(error) => api_error_response(application_error_status(error.code_enum()), error),
    }
}

async fn read_feedback_attachment(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ReadAttachmentInput>,
) -> Response<Body> {
    attachment_bytes_response(state.commands.read_feedback_attachment(input).await)
}

async fn read_request_attachment(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ReadAttachmentInput>,
) -> Response<Body> {
    attachment_bytes_response(state.commands.read_request_attachment(input).await)
}

async fn submit_feedback(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<SubmitFeedbackInput>,
) -> Response<Body> {
    application_result(state.commands.submit_feedback(input).await)
}

async fn approve_feedback_request(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ApproveFeedbackInput>,
) -> Response<Body> {
    application_result(state.commands.approve_feedback_request(input).await)
}

async fn cancel_feedback_request(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<CancelFeedbackInput>,
) -> Response<Body> {
    application_result(state.commands.cancel_feedback_request(input).await)
}

async fn rename_host_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<RenameHostSessionInput>,
) -> Response<Body> {
    application_result(state.commands.rename_host_session(input).await)
}

async fn set_host_session_pinned(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<SetHostSessionPinnedInput>,
) -> Response<Body> {
    application_result(state.commands.set_host_session_pinned(input).await)
}

async fn archive_host_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<HostSessionInput>,
) -> Response<Body> {
    application_result(state.commands.archive_host_session(input).await)
}

async fn unarchive_host_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<HostSessionInput>,
) -> Response<Body> {
    application_result(state.commands.unarchive_host_session(input).await)
}

async fn delete_host_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<HostSessionInput>,
) -> Response<Body> {
    application_void_result(state.commands.delete_host_session(input).await)
}

async fn delete_feedback_request(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<DeleteFeedbackRequestInput>,
) -> Response<Body> {
    application_void_result(state.commands.delete_feedback_request(input).await)
}

async fn set_host_pinned(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<SetHostPinnedInput>,
) -> Response<Body> {
    application_result(state.commands.set_host_pinned(input).await)
}
