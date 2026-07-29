//! MCP transport adapter for RambleDesk.

mod token;

use std::{
    net::{Ipv4Addr, SocketAddr},
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
use rambledesk_core::HealthSnapshot;
use rmcp::{
    RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Json},
    model::{Implementation, ServerCapabilities, ServerInfo},
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

#[derive(Debug, Clone)]
struct RambleDeskMcp {
    tool_router: ToolRouter<Self>,
}

impl RambleDeskMcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
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
            health: HealthSnapshot::m0(),
            protocol_version,
            client_supports_tasks,
        })
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RambleDeskMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("rambledesk", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "RambleDesk M0 exposes a read-only health probe. Feedback tools arrive in M1.",
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

pub async fn start_server(config: ServerConfig) -> Result<ServerHandle, ServerError> {
    let cancellation = CancellationToken::new();
    let transport_config = StreamableHttpServerConfig::default()
        .with_allowed_origins(config.allowed_origins)
        .with_max_request_body_bytes(256 * 1024)
        .with_cancellation_token(cancellation.child_token());

    let service: StreamableHttpService<RambleDeskMcp, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(RambleDeskMcp::new()),
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
