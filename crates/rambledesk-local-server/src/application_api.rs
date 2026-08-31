use axum::{
    Json, Router, body::Body, extract::State, http::Response, response::IntoResponse, routing::post,
};
use rambledesk_core::{
    ApplicationError, FeedbackApplication, FeedbackPackageView, GetFeedbackInput,
    ListFeedbackRequestsInput, ListHostSessionsInput,
};
use rambledesk_hosts::{HostProfile, known_host_profiles};
use serde::Serialize;

use crate::{api_error_response, application_error_status};

#[derive(Clone)]
struct ApplicationApiState {
    application: FeedbackApplication,
}

pub fn application_router(application: FeedbackApplication) -> Router {
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
        .with_state(ApplicationApiState { application })
}

fn application_result<T: Serialize>(result: Result<T, ApplicationError>) -> Response<Body> {
    match result {
        Ok(value) => Json(value).into_response(),
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
