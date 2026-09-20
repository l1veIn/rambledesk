use axum::{
    Json, Router,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use rambledesk_core::{ApplicationError, DescribeWorkbenchInput, ListWorkbenchesInput};

pub(super) fn router<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/workbenches/list", post(list))
        .route("/workbenches/describe", post(describe))
}

async fn list(Json(input): Json<ListWorkbenchesInput>) -> Response {
    response(rambledesk_core::list_workbenches(&input))
}

async fn describe(Json(input): Json<DescribeWorkbenchInput>) -> Response {
    response(rambledesk_core::describe_workbench(&input))
}

fn response<T: serde::Serialize>(result: Result<T, ApplicationError>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => crate::api_error_payload(
            StatusCode::BAD_REQUEST,
            error.code(),
            error.message(),
            error.retryable(),
        ),
    }
}
