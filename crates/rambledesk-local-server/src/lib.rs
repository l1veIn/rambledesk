//! Authenticated loopback server for RambleDesk local transports.

mod application_api;
mod token;
mod web_access;
mod web_access_server;
mod web_security;
mod web_session;

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
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use futures::StreamExt;
use rambledesk_core::{
    ApplicationError, ApplicationErrorCode, ApproveFeedbackInput, CancelFeedbackInput,
    FeedbackApplication, FeedbackRequestView, FeedbackStatus, GetFeedbackInput,
    RecoverFeedbackInput, RequestFeedbackInput,
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use std::{convert::Infallible, time::Duration};
use thiserror::Error;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tower_service::Service;

pub use application_api::{
    MAX_APPLICATION_JSON_BODY_BYTES, MAX_ATTACHMENT_UPLOAD_BODY_BYTES, REVISION_HEADER,
    RUNTIME_GENERATION_HEADER, application_router,
};
pub use token::{AccessToken, TokenError, default_token_path};
pub use web_access::{
    EVENT_CREDENTIAL_PROTOCOL_PREFIX, EVENT_PROTOCOL, WebAccessRouteConfig,
    WebSessionAuthenticator, web_access_router,
};
pub use web_access_server::{
    DEFAULT_WEB_ACCESS_PORT, SpaAsset, SpaAssetCachePolicy, SpaAssetSource,
    WebAccessSecurityLimits, WebAccessServerConfig, WebAccessServerError, WebAccessServerHandle,
    start_web_access_server,
};
pub use web_session::{
    DurableWebAccessToken, WebSessionAuthorization, WebSessionClock, WebSessionError,
    WebSessionManager, WebSessionPolicy,
};

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
        input.host_id = Some(host.to_owned());
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
        Err(error) => {
            return api_error_response(application_error_status(error.code_enum()), error);
        }
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
                return api_error_response(application_error_status(error.code_enum()), error);
            }
        }
    }

    Json(structured).into_response()
}

pub(crate) const fn application_error_status(code: ApplicationErrorCode) -> StatusCode {
    match code {
        ApplicationErrorCode::InvalidArgument => StatusCode::BAD_REQUEST,
        ApplicationErrorCode::RequestNotFound
        | ApplicationErrorCode::AttachmentNotFound
        | ApplicationErrorCode::HostSessionNotFound => StatusCode::NOT_FOUND,
        ApplicationErrorCode::RecoveryAmbiguous
        | ApplicationErrorCode::RequestConflict
        | ApplicationErrorCode::RequestAlreadyCompleted
        | ApplicationErrorCode::RequestTerminal
        | ApplicationErrorCode::DraftConflict
        | ApplicationErrorCode::AttachmentLimit
        | ApplicationErrorCode::HostSessionHasOpenRequests
        | ApplicationErrorCode::DeleteRequiresArchivedHostSession
        | ApplicationErrorCode::RequestNotTerminal => StatusCode::CONFLICT,
        ApplicationErrorCode::PackagePublishFailure
        | ApplicationErrorCode::FeedbackPackageReadFailure
        | ApplicationErrorCode::StorageFailure => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(crate) fn api_error_response(status: StatusCode, error: ApplicationError) -> Response<Body> {
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
            expected: Arc::from(token.secret().as_bytes()),
            allowed_origins: Arc::from(allowed_origins),
        }
    }

    fn accepts(&self, value: Option<&HeaderValue>) -> bool {
        let Some(actual) = web_security::bearer_credential(value) else {
            return false;
        };
        web_security::constant_time_bytes_eq(self.expected.as_ref(), actual.as_bytes())
    }

    fn accepts_origin(&self, value: Option<&HeaderValue>) -> bool {
        let Some(origin) = value.and_then(web_security::header_text) else {
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
        .and_then(web_security::header_text)
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

async fn handle_mcp_request(
    State(mut service): State<
        StreamableHttpService<rambledesk_mcp::RambleDeskMcp, LocalSessionManager>,
    >,
    mut request: Request<Body>,
) -> Response<Body> {
    let is_sse_handshake = request.method() == axum::http::Method::GET
        && request
            .headers()
            .get(axum::http::header::ACCEPT)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|h| h.contains("text/event-stream"))
        && !request.headers().contains_key("mcp-session-id");

    if is_sse_handshake {
        let host = request
            .headers()
            .get(HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("127.0.0.1:37642");
        let endpoint_url = format!("http://{host}{MCP_PATH}");
        let initial_event =
            Ok::<_, Infallible>(Event::default().event("endpoint").data(endpoint_url));
        let stream =
            futures::stream::once(async move { initial_event }).chain(futures::stream::pending());
        return Sse::new(stream)
            .keep_alive(
                KeepAlive::new()
                    .interval(Duration::from_secs(15))
                    .text("keepalive"),
            )
            .into_response();
    }

    if let Some(query) = request.uri().query()
        && !request.headers().contains_key("mcp-session-id")
    {
        for pair in query.split('&') {
            if let Some((k, v)) = pair.split_once('=')
                && (k.eq_ignore_ascii_case("sessionid") || k.eq_ignore_ascii_case("session_id"))
                && let Ok(val) = HeaderValue::from_str(v)
            {
                request.headers_mut().insert(
                    axum::http::header::HeaderName::from_static("mcp-session-id"),
                    val,
                );
                break;
            }
        }
    }

    if request.method() == axum::http::Method::POST {
        let (parts, body) = request.into_parts();
        let bytes = match axum::body::to_bytes(body, MAX_ATTACHMENT_REQUEST_BODY_BYTES).await {
            Ok(b) => b,
            Err(_) => {
                return (StatusCode::BAD_REQUEST, "Failed to read request body").into_response();
            }
        };

        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
            && value.get("method").and_then(|m| m.as_str()) == Some("subscriptions/listen")
        {
            let id = value.get("id").cloned().unwrap_or(serde_json::Value::Null);
            let response_body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            });

            let mut resp_builder = Response::builder().status(StatusCode::OK);
            if let Some(session_id) = parts.headers.get("mcp-session-id") {
                resp_builder = resp_builder.header("mcp-session-id", session_id);
            }

            let accept = parts
                .headers
                .get(axum::http::header::ACCEPT)
                .and_then(|h| h.to_str().ok())
                .unwrap_or_default();

            if accept.contains("text/event-stream") {
                let sse_data = format!(
                    "data: {}\n\n",
                    serde_json::to_string(&response_body).unwrap_or_default()
                );
                return resp_builder
                    .header(axum::http::header::CONTENT_TYPE, "text/event-stream")
                    .header(axum::http::header::CACHE_CONTROL, "no-cache")
                    .body(Body::from(sse_data))
                    .unwrap_or_else(|_| {
                        (StatusCode::INTERNAL_SERVER_ERROR, "error").into_response()
                    });
            } else {
                return resp_builder
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&response_body).unwrap_or_default(),
                    ))
                    .unwrap_or_else(|_| {
                        (StatusCode::INTERNAL_SERVER_ERROR, "error").into_response()
                    });
            }
        }

        let mut req = Request::from_parts(parts, Body::from(bytes));
        let current_accept = req
            .headers()
            .get(axum::http::header::ACCEPT)
            .and_then(|h| h.to_str().ok())
            .unwrap_or_default();
        if !current_accept.contains("application/json")
            || !current_accept.contains("text/event-stream")
        {
            req.headers_mut().insert(
                axum::http::header::ACCEPT,
                HeaderValue::from_static("application/json, text/event-stream"),
            );
        }
        request = req;
    }

    match service.call(request).await {
        Ok(response) => response.into_response(),
        Err(infallible) => match infallible {},
    }
}

pub async fn start_server(
    config: ServerConfig,
    application: FeedbackApplication,
) -> Result<ServerHandle, ServerError> {
    let cancellation = CancellationToken::new();
    let allowed_origins = config.allowed_origins.clone();
    let transport_config = StreamableHttpServerConfig::default()
        // Feedback requests are durable application records keyed by
        // request_id. Generic hosts can wait on a human for hours, so their
        // next tool call must not depend on rmcp's five-minute legacy session.
        .with_legacy_session_mode(false)
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
    let mcp = Router::new()
        .fallback(handle_mcp_request)
        .with_state(service);
    let router = Router::new()
        .nest(MCP_PATH, mcp)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_error_status_covers_host_session_contracts() {
        assert_eq!(
            application_error_status(ApplicationErrorCode::HostSessionNotFound),
            StatusCode::NOT_FOUND
        );
        for code in [
            ApplicationErrorCode::HostSessionHasOpenRequests,
            ApplicationErrorCode::DeleteRequiresArchivedHostSession,
            ApplicationErrorCode::RequestNotTerminal,
        ] {
            assert_eq!(application_error_status(code), StatusCode::CONFLICT);
        }
    }

    #[test]
    fn local_integration_policy_reuses_parsing_but_keeps_optional_origin_semantics() {
        let token = AccessToken::parse("a".repeat(64)).expect("token");
        let auth = AuthState::new(&token, vec!["tauri://localhost".to_owned()]);
        for (origin, expected) in [
            (None, true),
            (Some("tauri://localhost"), true),
            (Some("http://attacker.example"), false),
        ] {
            let header = origin.map(|value| HeaderValue::from_str(value).expect("origin"));
            assert_eq!(auth.accepts_origin(header.as_ref()), expected, "{origin:?}");
        }
        for (host, expected) in [
            ("127.0.0.1:37642", true),
            ("localhost:37642", true),
            ("127.0.0.1.example:37642", false),
        ] {
            assert_eq!(
                host_is_loopback(Some(&HeaderValue::from_str(host).expect("host"))),
                expected,
                "{host}",
            );
        }
        assert!(auth.accepts(Some(
            &HeaderValue::from_str(&format!("Bearer {}", token.secret())).expect("bearer")
        )));
        assert!(!auth.accepts(Some(&HeaderValue::from_static("Bearer wrong"))));
    }
}
