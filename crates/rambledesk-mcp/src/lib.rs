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
    FeedbackStatus, GetFeedbackInput, HealthSnapshot, ListFeedbackRequestsInput,
    ListFeedbackRequestsOutput, RequestFeedbackInput,
};
use rmcp::{
    RoleServer, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use schemars::JsonSchema;
use serde::Serialize;
use subtle::ConstantTimeEq;
use thiserror::Error;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;

pub use token::{AccessToken, TokenError, default_token_path};

pub const DEFAULT_PORT: u16 = 37_642;
pub const MCP_PATH: &str = "/mcp";

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

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct McpHealthSnapshot {
    #[serde(flatten)]
    health: HealthSnapshot,
    protocol_version: String,
    client_supports_tasks: bool,
}

#[tool_router]
impl RambleDeskMcp {
    #[tool(
        name = "rambledesk_health",
        description = "Read-only M0 health probe for the local RambleDesk workbench."
    )]
    fn health(&self, context: RequestContext<RoleServer>) -> Json<McpHealthSnapshot> {
        let protocol_version = context
            .protocol_version()
            .map(|version| version.to_string())
            .unwrap_or_else(|| "unknown".to_owned());
        let client_supports_tasks = context
            .client_capabilities()
            .is_some_and(|capabilities| capabilities.supports_tasks());

        Json(McpHealthSnapshot {
            health: HealthSnapshot::ready(),
            protocol_version,
            client_supports_tasks,
        })
    }

    #[tool(
        name = "request_feedback",
        description = "Persist a feedback request and return immediately with a durable handle. Reusing request_id with identical input is idempotent; call wait_for_feedback once to await the terminal result."
    )]
    async fn request_feedback(
        &self,
        Parameters(input): Parameters<RequestFeedbackInput>,
    ) -> CallToolResult {
        application_result(self.application.request_feedback(input).await)
    }

    #[tool(
        name = "get_feedback",
        description = "Compatibility and recovery tool: read the current state of a persistent feedback request without changing it. Normal clients should use wait_for_feedback instead of polling."
    )]
    async fn get_feedback(
        &self,
        Parameters(input): Parameters<GetFeedbackInput>,
    ) -> CallToolResult {
        application_result(self.application.get_feedback(input).await)
    }

    #[tool(
        name = "wait_for_feedback",
        description = "Wait without polling until a persistent feedback request is completed or cancelled. A completed response includes the parsed manifest, full Markdown, and absolute attachment paths. The call is safe to retry after a client timeout or reconnect."
    )]
    async fn wait_for_feedback(
        &self,
        Parameters(input): Parameters<GetFeedbackInput>,
    ) -> CallToolResult {
        wait_application_result(self.application.wait_for_feedback(input).await).await
    }

    #[tool(
        name = "list_feedback_requests",
        description = "List persistent feedback request summaries with optional project, agent, session, status, cursor, and limit filters. Defaults to open requests."
    )]
    async fn list_feedback_requests(
        &self,
        Parameters(input): Parameters<ListFeedbackRequestsInput>,
    ) -> CallToolResult {
        list_application_result(self.application.list_feedback_requests(input).await)
    }

    #[tool(
        name = "cancel_feedback",
        description = "Cancel a waiting or in-progress feedback request. Repeated cancellation preserves the first cancellation."
    )]
    async fn cancel_feedback(
        &self,
        Parameters(input): Parameters<CancelFeedbackInput>,
    ) -> CallToolResult {
        application_result(self.application.cancel_feedback(input).await)
    }
}

fn list_application_result(
    result: Result<ListFeedbackRequestsOutput, ApplicationError>,
) -> CallToolResult {
    match result {
        Ok(value) => {
            let summary = format!(
                "Listed {} feedback request summaries{}.",
                value.requests.len(),
                if value.next_cursor.is_some() {
                    "; more results are available"
                } else {
                    ""
                }
            );
            let mut result = CallToolResult::structured(
                serde_json::to_value(value).expect("application result must serialize"),
            );
            result.content = vec![ContentBlock::text(summary)];
            result
        }
        Err(error) => application_error_result(error),
    }
}

fn application_result(result: Result<FeedbackRequestView, ApplicationError>) -> CallToolResult {
    match result {
        Ok(value) => {
            let summary = match value.status {
                FeedbackStatus::Waiting => format!(
                    "Feedback request {} is waiting; call wait_for_feedback once to await the result.",
                    value.request_id
                ),
                FeedbackStatus::InProgress => format!(
                    "Feedback request {} is in progress; wait_for_feedback will return when it becomes terminal.",
                    value.request_id
                ),
                FeedbackStatus::Completed => {
                    format!("Feedback request {} is completed.", value.request_id)
                }
                FeedbackStatus::Cancelled => {
                    format!("Feedback request {} is cancelled.", value.request_id)
                }
            };
            let mut result = CallToolResult::structured(
                serde_json::to_value(value).expect("application result must serialize"),
            );
            result.content = vec![ContentBlock::text(summary)];
            result
        }
        Err(error) => application_error_result(error),
    }
}

async fn wait_application_result(
    result: Result<FeedbackRequestView, ApplicationError>,
) -> CallToolResult {
    let value = match result {
        Ok(value) => value,
        Err(error) => return application_error_result(error),
    };
    let package = match load_feedback_package(&value).await {
        Ok(package) => package,
        Err(message) => {
            return structured_error_result("FEEDBACK_PACKAGE_READ_FAILURE", &message, true);
        }
    };
    let summary = match value.status {
        FeedbackStatus::Completed => format!(
            "Feedback request {} completed; the full feedback package is attached.",
            value.request_id
        ),
        FeedbackStatus::Cancelled => {
            format!("Feedback request {} was cancelled.", value.request_id)
        }
        FeedbackStatus::Waiting | FeedbackStatus::InProgress => {
            format!("Feedback request {} is not terminal.", value.request_id)
        }
    };
    let mut structured = serde_json::to_value(&value).expect("application result must serialize");
    structured
        .as_object_mut()
        .expect("feedback request view must serialize as an object")
        .insert("feedback_package".to_owned(), package);
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
                "RambleDesk persists feedback requests before returning. Use request_feedback, then call wait_for_feedback once. Keep get_feedback only for reconnect recovery and diagnostics; do not poll it while waiting.",
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

async fn require_bearer(
    axum::extract::State(state): axum::extract::State<AuthState>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    if state.accepts(request.headers().get(AUTHORIZATION)) {
        return next.run(request).await;
    }

    let mut response = (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    response.headers_mut().insert(
        WWW_AUTHENTICATE,
        HeaderValue::from_static("Bearer realm=\"RambleDesk\""),
    );
    response
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
