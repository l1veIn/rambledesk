use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::post,
};
use rambledesk_core::{
    ApplicationError, ApproveFeedbackInput, CancelFeedbackInput, DeleteFeedbackRequestInput,
    FeedbackApplication, FeedbackPackageView, GetFeedbackInput, HostSessionInput,
    ListFeedbackRequestsInput, ListHostSessionsInput, RenameHostSessionInput, SaveDraftInput,
    SetHostPinnedInput, SetHostSessionPinnedInput, SubmitFeedbackInput,
    WorkbenchTerminalOperations,
};
use rambledesk_hosts::{HostProfile, known_host_profiles};
use serde::Serialize;

use crate::{api_error_response, application_error_status};

#[derive(Clone)]
struct ApplicationApiState {
    application: FeedbackApplication,
    terminal_operations: WorkbenchTerminalOperations,
}

pub fn application_router(
    application: FeedbackApplication,
    terminal_operations: WorkbenchTerminalOperations,
) -> Router {
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
        .with_state(ApplicationApiState {
            application,
            terminal_operations,
        })
}

fn application_result<T: Serialize>(result: Result<T, ApplicationError>) -> Response<Body> {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => api_error_response(application_error_status(error.code()), error),
    }
}

fn application_void_result(result: Result<(), ApplicationError>) -> Response<Body> {
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => api_error_response(application_error_status(error.code()), error),
    }
}

async fn list_feedback_inbox(State(state): State<ApplicationApiState>) -> Response<Body> {
    application_result(state.application.list_open_feedback_requests().await)
}

async fn list_host_sessions(State(state): State<ApplicationApiState>) -> Response<Body> {
    application_result(state.application.list_host_sessions().await)
}

async fn list_archived_host_sessions(
    State(state): State<ApplicationApiState>,
    Json(input): Json<ListHostSessionsInput>,
) -> Response<Body> {
    application_result(state.application.list_archived_host_sessions(input).await)
}

async fn list_host_profiles() -> Json<Vec<HostProfile>> {
    Json(known_host_profiles())
}

async fn list_feedback_requests(
    State(state): State<ApplicationApiState>,
    Json(input): Json<ListFeedbackRequestsInput>,
) -> Response<Body> {
    application_result(state.application.list_feedback_requests(input).await)
}

async fn get_feedback_workspace(
    State(state): State<ApplicationApiState>,
    Json(input): Json<GetFeedbackInput>,
) -> Response<Body> {
    application_result(
        state
            .application
            .get_feedback_workspace(input.request_id)
            .await,
    )
}

async fn read_published_feedback(
    State(state): State<ApplicationApiState>,
    Json(input): Json<GetFeedbackInput>,
) -> Response<Body> {
    let request = match state.application.get_feedback(input).await {
        Ok(request) => request,
        Err(error) => return api_error_response(application_error_status(error.code()), error),
    };
    application_result(
        state
            .application
            .read_feedback_package(&request)
            .await
            .map(|content| content.map(FeedbackPackageView::from)),
    )
}

async fn save_feedback_draft(
    State(state): State<ApplicationApiState>,
    Json(input): Json<SaveDraftInput>,
) -> Response<Body> {
    application_result(state.application.save_feedback_draft(input).await)
}

async fn submit_feedback(
    State(state): State<ApplicationApiState>,
    Json(input): Json<SubmitFeedbackInput>,
) -> Response<Body> {
    application_result(state.terminal_operations.submit_feedback(input).await)
}

async fn approve_feedback_request(
    State(state): State<ApplicationApiState>,
    Json(input): Json<ApproveFeedbackInput>,
) -> Response<Body> {
    application_result(state.terminal_operations.approve_feedback(input).await)
}

async fn cancel_feedback_request(
    State(state): State<ApplicationApiState>,
    Json(input): Json<CancelFeedbackInput>,
) -> Response<Body> {
    application_result(state.terminal_operations.cancel_feedback(input).await)
}

async fn rename_host_session(
    State(state): State<ApplicationApiState>,
    Json(input): Json<RenameHostSessionInput>,
) -> Response<Body> {
    application_result(state.application.rename_host_session(input).await)
}

async fn set_host_session_pinned(
    State(state): State<ApplicationApiState>,
    Json(input): Json<SetHostSessionPinnedInput>,
) -> Response<Body> {
    application_result(state.application.set_host_session_pinned(input).await)
}

async fn archive_host_session(
    State(state): State<ApplicationApiState>,
    Json(input): Json<HostSessionInput>,
) -> Response<Body> {
    application_result(state.application.archive_host_session(input).await)
}

async fn unarchive_host_session(
    State(state): State<ApplicationApiState>,
    Json(input): Json<HostSessionInput>,
) -> Response<Body> {
    application_result(state.application.unarchive_host_session(input).await)
}

async fn delete_host_session(
    State(state): State<ApplicationApiState>,
    Json(input): Json<HostSessionInput>,
) -> Response<Body> {
    application_void_result(state.application.delete_host_session(input).await)
}

async fn delete_feedback_request(
    State(state): State<ApplicationApiState>,
    Json(input): Json<DeleteFeedbackRequestInput>,
) -> Response<Body> {
    application_void_result(state.application.delete_feedback_request(input).await)
}

async fn set_host_pinned(
    State(state): State<ApplicationApiState>,
    Json(input): Json<SetHostPinnedInput>,
) -> Response<Body> {
    application_result(state.application.set_host_pinned(input).await)
}
