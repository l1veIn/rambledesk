//! MCP transport adapter for RambleDesk.

mod token;

use std::{
    net::{Ipv4Addr, SocketAddr},
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use axum::{
    Router,
    body::Body,
    http::{
        HeaderValue, Request, Response, StatusCode,
        header::{AUTHORIZATION, WWW_AUTHENTICATE},
    },
    middleware::{self, Next},
    response::IntoResponse,
};
use rambledesk_core::{
    ApplicationError, CancelFeedbackInput, FeedbackApplication, FeedbackRequestView,
    FeedbackStatus, GetFeedbackInput, HealthSnapshot, RequestFeedbackInput,
};
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use subtle::ConstantTimeEq;
use thiserror::Error;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;

pub use token::{AccessToken, TokenError, default_token_path};

pub const DEFAULT_PORT: u16 = 37_642;
pub const MCP_PATH: &str = "/mcp";

/// Install-time / client-config host identity (env on the MCP client entry).
pub const HOST_ENV_KEY: &str = "RAMBLEDESK_HOST";
/// HTTP header mirror so the loopback server can see the installed host id.
pub const HOST_HEADER: &str = "x-rambledesk-host";

const DEFAULT_ALLOWED_ORIGINS: &[&str] = &[
    "tauri://localhost",
    "http://tauri.localhost",
    "http://localhost:1420",
];

tokio::task_local! {
    static REQUEST_HOST: Option<String>;
}

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
struct RambleDeskMcp {
    tool_router: ToolRouter<Self>,
    application: FeedbackApplication,
}

impl RambleDeskMcp {
    fn new(application: FeedbackApplication) -> Self {
        Self {
            tool_router: Self::tool_router(),
            application,
        }
    }
}

fn current_request_host() -> Option<String> {
    REQUEST_HOST.try_with(|host| host.clone()).ok().flatten()
}

fn resolve_agent(mut input: RequestFeedbackInput) -> RequestFeedbackInput {
    if let Some(host) = current_request_host() {
        // Install-time host id is authoritative for this MCP client entry.
        input.agent = host;
    }
    input
}

#[tool_router]
impl RambleDeskMcp {
    #[tool(
        name = "request_feedback",
        description = "Persist a feedback request and return immediately with a durable handle (request_id). After creating, stop the current turn — do not poll. When the human submits feedback, call get_feedback with the same request_id to read the package. Reusing request_id with identical input is idempotent. If the MCP client was auto-registered, RAMBLEDESK_HOST / X-RambleDesk-Host identify the host."
    )]
    async fn request_feedback(
        &self,
        Parameters(input): Parameters<RequestFeedbackInput>,
    ) -> CallToolResult {
        let input = resolve_agent(input);
        feedback_tool_result(self.application.request_feedback(input).await, false).await
    }

    #[tool(
        name = "get_feedback",
        description = "Read the current state of a durable feedback request without changing it. Use after resume or for diagnostics. When status is completed, the response includes the full feedback package (manifest, markdown, attachment paths). Do not poll while waiting — end the turn after request_feedback and resume when notified."
    )]
    async fn get_feedback(
        &self,
        Parameters(input): Parameters<GetFeedbackInput>,
    ) -> CallToolResult {
        feedback_tool_result(self.application.get_feedback(input).await, true).await
    }

    #[tool(
        name = "cancel_feedback",
        description = "Cancel a waiting or in-progress feedback request. Repeated cancellation preserves the first cancellation."
    )]
    async fn cancel_feedback(
        &self,
        Parameters(input): Parameters<CancelFeedbackInput>,
    ) -> CallToolResult {
        feedback_tool_result(self.application.cancel_feedback(input).await, false).await
    }
}

async fn feedback_tool_result(
    result: Result<FeedbackRequestView, ApplicationError>,
    include_package_when_terminal: bool,
) -> CallToolResult {
    let value = match result {
        Ok(value) => value,
        Err(error) => return application_error_result(error),
    };

    let summary = match value.status {
        FeedbackStatus::Waiting => format!(
            "Feedback request {} is waiting for the human. End this turn; do not poll. When resumed, call get_feedback with this request_id.",
            value.request_id
        ),
        FeedbackStatus::InProgress => format!(
            "Feedback request {} is in progress. End this turn; when resumed, call get_feedback with this request_id.",
            value.request_id
        ),
        FeedbackStatus::Completed => {
            format!("Feedback request {} is completed.", value.request_id)
        }
        FeedbackStatus::Cancelled => {
            format!("Feedback request {} is cancelled.", value.request_id)
        }
    };

    let mut structured = serde_json::to_value(&value).expect("application result must serialize");
    let object = structured
        .as_object_mut()
        .expect("feedback request view must serialize as an object");

    object.insert(
        "server".to_owned(),
        serde_json::to_value(HealthSnapshot::ready()).expect("health must serialize"),
    );
    if let Some(host) = current_request_host() {
        object.insert("host".to_owned(), serde_json::Value::String(host));
    }

    if include_package_when_terminal
        && matches!(
            value.status,
            FeedbackStatus::Completed | FeedbackStatus::Cancelled
        )
    {
        match load_feedback_package(&value).await {
            Ok(package) => {
                object.insert("feedback_package".to_owned(), package);
            }
            Err(message) => {
                return structured_error_result("FEEDBACK_PACKAGE_READ_FAILURE", &message, true);
            }
        }
    }

    let mut result = CallToolResult::structured(structured);
    result.content = vec![ContentBlock::text(summary)];
    result
}

async fn load_feedback_package(value: &FeedbackRequestView) -> Result<serde_json::Value, String> {
    let Some(feedback) = value.feedback.as_ref() else {
        return Ok(serde_json::Value::Null);
    };
    let manifest_text = tokio::fs::read_to_string(&feedback.manifest_path)
        .await
        .map_err(|error| format!("could not read feedback manifest: {error}"))?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|error| format!("feedback manifest is invalid: {error}"))?;
    let markdown = tokio::fs::read_to_string(&feedback.markdown_path)
        .await
        .map_err(|error| format!("could not read feedback markdown: {error}"))?;
    let directory = Path::new(&feedback.directory_path);
    let attachment_paths = manifest
        .get("attachments")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(|attachment| {
            let relative = attachment
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "feedback manifest attachment path is missing".to_owned())?;
            let relative_path = PathBuf::from(relative);
            let mut components = relative_path.components();
            if components.next() != Some(Component::Normal("attachments".as_ref()))
                || components.next().is_none()
                || components.next().is_some()
            {
                return Err("feedback manifest attachment path is unsafe".to_owned());
            }
            Ok(directory.join(relative_path).to_string_lossy().into_owned())
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(serde_json::json!({
        "manifest": manifest,
        "markdown": markdown,
        "attachment_paths": attachment_paths,
    }))
}

fn application_error_result(error: ApplicationError) -> CallToolResult {
    structured_error_result(error.code(), error.message(), error.retryable())
}

fn structured_error_result(code: &str, message: &str, retryable: bool) -> CallToolResult {
    let mut result = CallToolResult::structured_error(serde_json::json!({
        "code": code,
        "message": message,
        "retryable": retryable,
        "server": HealthSnapshot::ready(),
    }));
    result.content = vec![ContentBlock::text(format!(
        "RambleDesk {}: {}",
        code, message
    ))];
    result
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RambleDeskMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("rambledesk", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "RambleDesk tools: request_feedback, get_feedback, cancel_feedback. \
Create a durable request with request_feedback, then end the current turn — do not poll and do not wait on a long tool call. \
When the human finishes and the session is resumed, call get_feedback(request_id) to load the package. \
Auto-registered clients set RAMBLEDESK_HOST (and X-RambleDesk-Host) so the host identity is known without guessing.",
            )
    }
}

#[derive(Clone)]
struct AuthState {
    expected: Arc<[u8]>,
}

impl AuthState {
    fn new(token: &AccessToken) -> Self {
        Self {
            expected: Arc::from(format!("Bearer {}", token.secret()).into_bytes()),
        }
    }

    fn accepts(&self, value: Option<&HeaderValue>) -> bool {
        let Some(actual) = value.and_then(|value| value.to_str().ok()) else {
            return false;
        };
        bool::from(self.expected.as_ref().ct_eq(actual.as_bytes()))
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
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    if !state.accepts(request.headers().get(AUTHORIZATION)) {
        let mut response = (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        response.headers_mut().insert(
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"RambleDesk\""),
        );
        return response;
    }

    let host = extract_request_host(&request);
    REQUEST_HOST.scope(host, next.run(request)).await
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
    #[error("failed to bind RambleDesk MCP loopback listener: {0}")]
    Bind(#[source] std::io::Error),
    #[error("RambleDesk MCP server failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("RambleDesk MCP server task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
}

pub async fn start_server(
    config: ServerConfig,
    application: FeedbackApplication,
) -> Result<ServerHandle, ServerError> {
    let cancellation = CancellationToken::new();
    let transport_config = StreamableHttpServerConfig::default()
        .with_allowed_origins(config.allowed_origins)
        .with_max_request_body_bytes(256 * 1024)
        .with_cancellation_token(cancellation.child_token());

    let service: StreamableHttpService<RambleDeskMcp, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(RambleDeskMcp::new(application.clone())),
            Default::default(),
            transport_config,
        );

    let auth = AuthState::new(&config.access_token);
    let router = Router::new()
        .nest_service(MCP_PATH, service)
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

    tracing::info!(%address, "RambleDesk MCP listening on loopback");

    Ok(ServerHandle {
        address,
        endpoint,
        cancellation,
        task,
    })
}
