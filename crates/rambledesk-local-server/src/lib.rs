//! Authenticated loopback server for RambleDesk local transports.

mod token;

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Extension, State},
    http::{
        HeaderValue, Request, Response, StatusCode,
        header::{AUTHORIZATION, HOST, ORIGIN, WWW_AUTHENTICATE},
    },
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post},
};
use rambledesk_core::{
    ApplicationError, ApproveFeedbackInput, CancelFeedbackInput, FeedbackApplication,
    FeedbackRequestView, FeedbackStatus, GetFeedbackInput, RecoverFeedbackInput,
    RequestFeedbackInput,
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use subtle::ConstantTimeEq;
use thiserror::Error;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;

pub use token::{AccessToken, TokenError, default_token_path};

pub use rambledesk_core::{HOST_ENV_KEY, HOST_HEADER};

pub const DEFAULT_PORT: u16 = 37_642;
pub const MCP_PATH: &str = "/mcp";
pub const API_PATH: &str = "/api";
const MAX_ATTACHMENT_REQUEST_BODY_BYTES: usize = 96 * 1024 * 1024;

const DEFAULT_ALLOWED_ORIGINS: &[&str] = &[
    "tauri://localhost",
    "http://tauri.localhost",
    "http://localhost:1420",
];

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub access_token: AccessToken,
    pub allowed_origins: Vec<String>,
}

impl ServerConfig {
    pub fn new(access_token: AccessToken) -> Self {
        Self {
            port: DEFAULT_PORT,
            access_token,
            allowed_origins: DEFAULT_ALLOWED_ORIGINS
                .iter()
                .map(ToString::to_string)
                .collect(),
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
}

#[derive(Clone)]
struct ApiState {
    application: FeedbackApplication,
}

#[derive(Clone)]
struct RequestHost(Option<String>);

fn apply_request_host(
    mut input: RequestFeedbackInput,
    request_host: Option<&str>,
) -> RequestFeedbackInput {
    if let Some(host) = request_host {
        input.host_id = host.to_owned();
    }
    input
}

async fn api_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ready": true,
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": ["final_approval", "request_recovery", "request_attachments"]
    }))
}

async fn api_request_feedback(
    State(state): State<ApiState>,
    Extension(request_host): Extension<RequestHost>,
    Json(input): Json<RequestFeedbackInput>,
) -> Response<Body> {
    let application = state.application.clone();
    api_feedback_result(
        &application,
        application
            .request_feedback(apply_request_host(input, request_host.0.as_deref()))
            .await,
        false,
    )
    .await
}

async fn api_get_feedback(
    State(state): State<ApiState>,
    Json(input): Json<GetFeedbackInput>,
) -> Response<Body> {
    let application = state.application.clone();
    api_feedback_result(&application, application.get_feedback(input).await, true).await
}

async fn api_wait_feedback(
    State(state): State<ApiState>,
    Json(input): Json<GetFeedbackInput>,
) -> Response<Body> {
    let application = state.application.clone();
    api_feedback_result(&application, application.wait_feedback(input).await, true).await
}

async fn api_recover_feedback(
    State(state): State<ApiState>,
    Extension(request_host): Extension<RequestHost>,
    Json(input): Json<RecoverFeedbackInput>,
) -> Response<Body> {
    let application = state.application.clone();
    let mut input = input;
    if request_host.0.is_some() {
        input.host_id = request_host.0;
    }
    api_feedback_result(
        &application,
        application.recover_feedback(input).await,
        true,
    )
    .await
}

async fn api_approve_feedback(
    State(state): State<ApiState>,
    Json(input): Json<ApproveFeedbackInput>,
) -> Response<Body> {
    let application = state.application.clone();
    api_feedback_result(
        &application,
        application.approve_feedback(input).await,
        false,
    )
    .await
}

async fn api_cancel_feedback(
    State(state): State<ApiState>,
    Json(input): Json<CancelFeedbackInput>,
) -> Response<Body> {
    let application = state.application.clone();
    api_feedback_result(
        &application,
        application.cancel_feedback(input).await,
        false,
    )
    .await
}

async fn api_feedback_result(
    application: &FeedbackApplication,
    result: Result<FeedbackRequestView, ApplicationError>,
    include_package_when_terminal: bool,
) -> Response<Body> {
    let value = match result {
        Ok(value) => value,
        Err(error) => return api_error_response(application_error_status(error.code()), error),
    };

    let mut structured = serde_json::to_value(&value).expect("application result must serialize");
    let object = structured
        .as_object_mut()
        .expect("feedback request view must serialize as an object");
    if include_package_when_terminal
        && (value.feedback.is_some() || value.status == FeedbackStatus::Cancelled)
    {
        match application.read_feedback_package(&value).await {
            Ok(package) => {
                object.insert(
                    "feedback_package".to_owned(),
                    serde_json::to_value(package).expect("feedback package must serialize"),
                );
            }
            Err(error) => {
                return api_error_response(application_error_status(error.code()), error);
            }
        }
    }

    Json(structured).into_response()
}

fn application_error_status(code: &str) -> StatusCode {
    match code {
        "INVALID_ARGUMENT" => StatusCode::BAD_REQUEST,
        "REQUEST_NOT_FOUND" | "ATTACHMENT_NOT_FOUND" => StatusCode::NOT_FOUND,
        "REQUEST_CONFLICT"
        | "RECOVERY_AMBIGUOUS"
        | "REQUEST_ALREADY_COMPLETED"
        | "REQUEST_TERMINAL"
        | "DRAFT_CONFLICT"
        | "ATTACHMENT_LIMIT" => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn api_error_response(status: StatusCode, error: ApplicationError) -> Response<Body> {
    api_error_payload(status, error.code(), error.message(), error.retryable())
}

fn api_error_payload(
    status: StatusCode,
    code: &str,
    message: &str,
    retryable: bool,
) -> Response<Body> {
    (
        status,
        Json(serde_json::json!({
            "code": code,
            "message": message,
            "retryable": retryable,
        })),
    )
        .into_response()
}

#[derive(Clone)]
struct AuthState {
    expected: Arc<[u8]>,
    allowed_origins: Arc<[String]>,
}

impl AuthState {
    fn new(token: &AccessToken, allowed_origins: Vec<String>) -> Self {
        Self {
            expected: Arc::from(format!("Bearer {}", token.secret()).into_bytes()),
            allowed_origins: Arc::from(allowed_origins),
        }
    }

    fn accepts(&self, value: Option<&HeaderValue>) -> bool {
        let Some(actual) = value.and_then(|value| value.to_str().ok()) else {
            return false;
        };
        bool::from(self.expected.as_ref().ct_eq(actual.as_bytes()))
    }

    fn accepts_origin(&self, value: Option<&HeaderValue>) -> bool {
        let Some(origin) = value.and_then(|value| value.to_str().ok()) else {
            return true;
        };
        self.allowed_origins.iter().any(|allowed| allowed == origin)
    }
}

fn extract_request_host(request: &Request<Body>) -> Option<String> {
    request
        .headers()
        .get(HOST_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

async fn require_bearer(
    axum::extract::State(state): axum::extract::State<AuthState>,
    mut request: Request<Body>,
    next: Next,
) -> Response<Body> {
    if !host_is_loopback(request.headers().get(HOST)) {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    if !state.accepts_origin(request.headers().get(ORIGIN)) {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    if !state.accepts(request.headers().get(AUTHORIZATION)) {
        let mut response = (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        response.headers_mut().insert(
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"RambleDesk\""),
        );
        return response;
    }

    let host = extract_request_host(&request);
    request.extensions_mut().insert(RequestHost(host.clone()));
    rambledesk_mcp::with_request_host(host, next.run(request)).await
}

fn host_is_loopback(value: Option<&HeaderValue>) -> bool {
    let Some(host) = value
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    let host = host
        .rsplit_once(':')
        .map(|(host, _port)| host)
        .unwrap_or(host);
    matches!(host, "127.0.0.1" | "localhost")
}

pub struct ServerHandle {
    address: SocketAddr,
    endpoint: String,
    cancellation: CancellationToken,
    task: JoinHandle<Result<(), std::io::Error>>,
}

impl ServerHandle {
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub async fn shutdown(self) -> Result<(), ServerError> {
        self.cancellation.cancel();
        self.task.await??;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("failed to bind RambleDesk local server loopback listener: {0}")]
    Bind(#[source] std::io::Error),
    #[error("RambleDesk local server failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("RambleDesk local server task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
}

pub async fn start_server(
    config: ServerConfig,
    application: FeedbackApplication,
) -> Result<ServerHandle, ServerError> {
    let cancellation = CancellationToken::new();
    let allowed_origins = config.allowed_origins.clone();
    let transport_config = StreamableHttpServerConfig::default()
        .with_allowed_origins(config.allowed_origins)
        .with_max_request_body_bytes(MAX_ATTACHMENT_REQUEST_BODY_BYTES)
        .with_cancellation_token(cancellation.child_token());

    let mcp_application = application.clone();
    let service: StreamableHttpService<rambledesk_mcp::RambleDeskMcp, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(rambledesk_mcp::RambleDeskMcp::new(mcp_application.clone())),
            Default::default(),
            transport_config,
        );

    let auth = AuthState::new(&config.access_token, allowed_origins);
    let api = Router::new()
        .route("/health", get(api_health))
        .route(
            "/feedback/request",
            post(api_request_feedback)
                .layer(DefaultBodyLimit::max(MAX_ATTACHMENT_REQUEST_BODY_BYTES)),
        )
        .route("/feedback/get", post(api_get_feedback))
        .route("/feedback/wait", post(api_wait_feedback))
        .route("/feedback/recover", post(api_recover_feedback))
        .route("/feedback/approve", post(api_approve_feedback))
        .route("/feedback/cancel", post(api_cancel_feedback))
        .with_state(ApiState {
            application: application.clone(),
        });
    let router = Router::new()
        .nest_service(MCP_PATH, service)
        .nest(API_PATH, api)
        .layer(middleware::from_fn_with_state(auth, require_bearer));

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.port))
        .await
        .map_err(ServerError::Bind)?;
    let address = listener.local_addr().map_err(ServerError::Bind)?;
    let endpoint = format!("http://{address}{MCP_PATH}");

    let task_cancellation = cancellation.clone();
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move { task_cancellation.cancelled_owned().await })
            .await
    });

    tracing::info!(%address, "RambleDesk local server listening on loopback");

    Ok(ServerHandle {
        address,
        endpoint,
        cancellation,
        task,
    })
}
