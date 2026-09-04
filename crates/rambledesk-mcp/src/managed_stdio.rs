//! Instance companion inspired by Codeg 3ebdfed's delegation/companion.rs and
//! connection.rs injection (Apache-2.0; see THIRD_PARTY_NOTICES). Changed: use
//! RambleDesk's existing private HTTP scope, environment-only credentials, and
//! nonblocking durable feedback. This process never owns continuation or identity.
mod bounded_input;

use std::{fmt, net::IpAddr, sync::Arc, time::Duration};

use rambledesk_core::ManagedFeedbackEndpoint;
use rmcp::{
    ErrorData, Peer, RoleClient, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, ClientInfo, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use tokio::io::{AsyncRead, AsyncWrite};

pub const URL_ENV: &str = "RAMBLEDESK_MANAGED_MCP_URL";
pub const TOKEN_ENV: &str = "RAMBLEDESK_MANAGED_MCP_TOKEN";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CALL_TIMEOUT: Duration = Duration::from_secs(60);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Intentionally excludes upstream diagnostics, input, headers and credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedStdioError {
    MissingCapability,
    InvalidCapability,
    UpstreamUnavailable,
    StdioClosed,
    RuntimeUnavailable,
}
impl fmt::Display for ManagedStdioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MissingCapability => "Managed feedback companion requires its instance capability environment",
            Self::InvalidCapability => "Managed feedback companion received an invalid local instance capability",
            Self::UpstreamUnavailable => "Managed feedback binding is unavailable or was revoked; retain the original feedback request ID",
            Self::StdioClosed => "Managed feedback companion stdio closed before initialization",
            Self::RuntimeUnavailable => "Managed feedback companion could not start its runtime",
        })
    }
}
impl std::error::Error for ManagedStdioError {}

pub fn endpoint_from_env() -> Result<ManagedFeedbackEndpoint, ManagedStdioError> {
    let endpoint = ManagedFeedbackEndpoint {
        url: std::env::var(URL_ENV).map_err(|_| ManagedStdioError::MissingCapability)?,
        bearer_token: std::env::var(TOKEN_ENV).map_err(|_| ManagedStdioError::MissingCapability)?,
    };
    validate(&endpoint)?;
    Ok(endpoint)
}

fn validate(endpoint: &ManagedFeedbackEndpoint) -> Result<(), ManagedStdioError> {
    let invalid = || ManagedStdioError::InvalidCapability;
    let url = reqwest::Url::parse(&endpoint.url).map_err(|_| invalid())?;
    let host = url.host_str().ok_or_else(invalid)?.trim_matches(['[', ']']);
    if url.scheme() != "http"
        || !host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/mcp-managed"
        || url.port_or_known_default().is_none_or(|port| port == 0)
        || endpoint.bearer_token.len() != 64
        || !endpoint
            .bearer_token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid());
    }
    Ok(())
}

fn unavailable() -> ErrorData {
    ErrorData::internal_error(ManagedStdioError::UpstreamUnavailable.to_string(), None)
}

struct Forwarder {
    upstream: Peer<RoleClient>,
    info: ServerInfo,
    failed: Arc<tokio::sync::Notify>,
}
impl Forwarder {
    fn upstream_failed(&self) -> ErrorData {
        self.failed.notify_one();
        unavailable()
    }
}
impl ServerHandler for Forwarder {
    fn get_info(&self) -> ServerInfo {
        self.info.clone()
    }

    async fn list_tools(
        &self,
        input: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        tokio::select! {
            _ = context.ct.cancelled() => Err(unavailable()),
            result = tokio::time::timeout(CALL_TIMEOUT, self.upstream.list_tools(input)) => {
                let mut result = result.map_err(|_| self.upstream_failed())?.map_err(|_| self.upstream_failed())?;
                result.tools.retain(|tool| matches!(tool.name.as_ref(), "request_feedback" | "get_feedback" | "recover_feedback"));
                Ok(result)
            }
        }
    }

    async fn call_tool(
        &self,
        input: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if !matches!(
            input.name.as_ref(),
            "request_feedback" | "get_feedback" | "recover_feedback"
        ) {
            return Err(ErrorData::invalid_request(
                "Only managed feedback request/get/recover tools are available",
                None,
            ));
        }
        tokio::select! {
            _ = context.ct.cancelled() => Err(unavailable()),
            result = tokio::time::timeout(CALL_TIMEOUT, self.upstream.call_tool(input)) => result.map_err(|_| self.upstream_failed())?.map(Into::into).map_err(|_| self.upstream_failed()),
        }
    }
}

/// Forward MCP over owned stdio to one private instance endpoint. Never reads the
/// generic server token, modifies requests, polls feedback or schedules a turn.
/// Call from a companion process without a tracing subscriber: transport logs
/// are not a safe place to expose per-instance capabilities or tool arguments.
pub async fn run_managed_stdio<R, W>(
    endpoint: ManagedFeedbackEndpoint,
    input: R,
    output: W,
) -> Result<(), ManagedStdioError>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    validate(&endpoint)?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .pool_max_idle_per_host(0)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(|_| ManagedStdioError::UpstreamUnavailable)?;
    let config = StreamableHttpClientTransportConfig::with_uri(endpoint.url)
        .auth_header(endpoint.bearer_token)
        .max_sse_event_size(MAX_FRAME_BYTES)
        .reinit_on_expired_session(false);
    let transport = StreamableHttpClientTransport::with_client(client, config);
    let upstream = tokio::time::timeout(CONNECT_TIMEOUT, ClientInfo::default().serve(transport))
        .await
        .map_err(|_| ManagedStdioError::UpstreamUnavailable)?
        .map_err(|_| ManagedStdioError::UpstreamUnavailable)?;
    let peer_info = upstream
        .peer_info()
        .ok_or(ManagedStdioError::UpstreamUnavailable)?;
    // This companion implements only the three feedback tools, regardless of
    // future upstream capability expansion.
    let mut info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
        .with_server_info(peer_info.server_info.clone().unwrap_or_else(|| {
            Implementation::new("rambledesk-managed", env!("CARGO_PKG_VERSION"))
        }));
    info.instructions = peer_info.instructions.clone();
    let failed = Arc::new(tokio::sync::Notify::new());
    let forwarder = Forwarder {
        upstream: upstream.peer().clone(),
        info,
        failed: failed.clone(),
    };
    let upstream_cancel = upstream.cancellation_token();
    let mut upstream_wait = Box::pin(upstream.waiting());
    let input = bounded_input::BoundedInput::new(input, MAX_FRAME_BYTES);
    let downstream = tokio::select! {
        _ = &mut upstream_wait => return Err(ManagedStdioError::UpstreamUnavailable),
        result = tokio::time::timeout(CONNECT_TIMEOUT, forwarder.serve((input, output))) => {
            result.map_err(|_| ManagedStdioError::StdioClosed)?.map_err(|_| ManagedStdioError::StdioClosed)?
        }
    };
    let downstream_cancel = downstream.cancellation_token();
    let mut downstream_wait = Box::pin(downstream.waiting());
    let result = tokio::select! {
        _ = failed.notified() => {
            downstream_cancel.cancel();
            upstream_cancel.cancel();
            let _ = tokio::time::timeout(CLOSE_TIMEOUT, async { tokio::join!(&mut downstream_wait, &mut upstream_wait) }).await;
            Err(ManagedStdioError::UpstreamUnavailable)
        }
        _ = &mut upstream_wait => {
            downstream_cancel.cancel();
            let _ = tokio::time::timeout(CLOSE_TIMEOUT, &mut downstream_wait).await;
            Err(ManagedStdioError::UpstreamUnavailable)
        }
        _ = &mut downstream_wait => {
            upstream_cancel.cancel();
            let _ = tokio::time::timeout(CLOSE_TIMEOUT, &mut upstream_wait).await;
            Ok(())
        }
    };
    result
}

pub async fn run_from_env() -> Result<(), ManagedStdioError> {
    run_managed_stdio(
        endpoint_from_env()?,
        tokio::io::stdin(),
        tokio::io::stdout(),
    )
    .await
}

/// Process early dispatch runs before Tauri, tracing, database or tray setup.
/// The caller must exit with the returned status. Tokio's stdin reader may still
/// be blocked in an OS read when the upstream binding is revoked; bounded runtime
/// shutdown lets the process exit without waiting for the Agent to close stdin.
pub fn run_process() -> i32 {
    let result = if std::env::args_os().len() != 2 {
        Err(ManagedStdioError::InvalidCapability)
    } else {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|_| ManagedStdioError::RuntimeUnavailable)
            .and_then(|runtime| {
                let result = runtime.block_on(run_from_env());
                runtime.shutdown_timeout(Duration::from_millis(100));
                result
            })
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

#[cfg(test)]
mod tests;
