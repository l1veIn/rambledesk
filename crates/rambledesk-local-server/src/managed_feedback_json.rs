use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, FromRequest, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use rambledesk_core::{GetFeedbackInput, ManagedFeedbackRecoverInput, ManagedFeedbackRequestInput};
use serde_json::Value;

use super::LocalManagedFeedbackProvider;
use crate::{MAX_ATTACHMENT_REQUEST_BODY_BYTES, api_error_payload, api_managed_feedback_result};

pub(super) fn router(provider: Arc<LocalManagedFeedbackProvider>) -> Router {
    Router::new()
        .route("/request", post(request_feedback))
        .route("/get", post(get_feedback))
        .route("/recover", post(recover_feedback))
        .layer(DefaultBodyLimit::max(MAX_ATTACHMENT_REQUEST_BODY_BYTES))
        .with_state(provider)
}

enum Command {
    Request,
    Get,
    Recover,
}

enum Operation {
    Request(ManagedFeedbackRequestInput),
    Get(GetFeedbackInput),
    Recover(ManagedFeedbackRecoverInput),
}

impl Command {
    fn parse(self, value: Value) -> Result<Operation, serde_json::Error> {
        match self {
            Self::Request => serde_json::from_value(value).map(Operation::Request),
            Self::Get => serde_json::from_value(value).map(Operation::Get),
            Self::Recover => serde_json::from_value(value).map(Operation::Recover),
        }
    }
}

async fn request_feedback(
    State(provider): State<Arc<LocalManagedFeedbackProvider>>,
    request: Request<Body>,
) -> Response {
    handle_request(provider, request, Command::Request).await
}

async fn get_feedback(
    State(provider): State<Arc<LocalManagedFeedbackProvider>>,
    request: Request<Body>,
) -> Response {
    handle_request(provider, request, Command::Get).await
}

async fn recover_feedback(
    State(provider): State<Arc<LocalManagedFeedbackProvider>>,
    request: Request<Body>,
) -> Response {
    handle_request(provider, request, Command::Recover).await
}

async fn handle_request(
    provider: Arc<LocalManagedFeedbackProvider>,
    request: Request<Body>,
    command: Command,
) -> Response {
    let binding = match provider.authenticate(request.headers()).await {
        Ok(binding) => binding,
        Err(status) => return status.into_response(),
    };
    let active = binding.active.read().await;
    if !*active {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    // Reading an unfinished body is not an admitted business operation. Revoking
    // its instance must wake this request instead of waiting for the client.
    let value = tokio::select! {
        biased;
        _ = binding.cancellation.cancelled() => return StatusCode::UNAUTHORIZED.into_response(),
        value = Json::<Value>::from_request(request, &()) => match value {
            Ok(Json(value)) => value,
            Err(error) => return api_error_payload(
                error.status(), "INVALID_ARGUMENT", "Invalid JSON request body", false,
            ),
        },
    };
    let operation = match command.parse(value) {
        Ok(operation) => operation,
        Err(_) => {
            return api_error_payload(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                "Invalid feedback command input",
                false,
            );
        }
    };
    let lease = tokio::select! {
        biased;
        _ = binding.cancellation.cancelled() => return StatusCode::UNAUTHORIZED.into_response(),
        lease = binding.scope.lease() => match lease {
            Some(lease) => lease,
            None => return StatusCode::UNAUTHORIZED.into_response(),
        },
    };
    drop(active);
    // Once admitted, finish the durable operation and package its result even if
    // revocation starts. Both HTTP and MCP share this same scope lease boundary.
    let application = &provider.application;
    let (result, include_package) = match operation {
        Operation::Request(input) => (
            application
                .request_managed_feedback(lease.scope(), input.into())
                .await,
            false,
        ),
        Operation::Get(input) => (
            application.get_managed_feedback(lease.scope(), input).await,
            true,
        ),
        Operation::Recover(input) => (
            application
                .recover_managed_feedback(lease.scope(), input.request_id)
                .await,
            true,
        ),
    };
    api_managed_feedback_result(application, result, include_package).await
}
