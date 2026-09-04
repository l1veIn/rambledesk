use agent_client_protocol::schema::v1::{
    EnvVariable, HttpHeader, McpServer, McpServerHttp, McpServerStdio,
};
use rambledesk_core::{AgentDriverError, FeedbackTransport, ManagedFeedbackEndpoint};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub(crate) const URL_ENV: &str = "RAMBLEDESK_MANAGED_MCP_URL";
pub(crate) const TOKEN_ENV: &str = "RAMBLEDESK_MANAGED_MCP_TOKEN";
// Reserved runtime capabilities are never read from persistent AgentConfig.env.
const PRIVATE_ENV: &[&str] = &[
    URL_ENV,
    TOKEN_ENV,
    "RAMBLEDESK_MANAGED_PI_WRAPPER",
    "RAMBLEDESK_MANAGED_PI_COMMAND",
    "RAMBLEDESK_MANAGED_PI_ARGS",
    "RAMBLEDESK_MANAGED_PI_EXTENSION",
];

pub(crate) fn public_environment(env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    env.iter()
        .filter(|(key, _)| {
            !PRIVATE_ENV
                .iter()
                .any(|private| key.eq_ignore_ascii_case(private))
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub(crate) fn inherited_private_env_to_remove(env: &BTreeMap<String, String>) -> Vec<&'static str> {
    PRIVATE_ENV
        .iter()
        .copied()
        .filter(|private| !env.keys().any(|key| key.eq_ignore_ascii_case(private)))
        .collect()
}

pub(crate) fn validate_companion(path: &Path) -> Result<PathBuf, AgentDriverError> {
    if !path.is_absolute() || !path.is_file() {
        return Err(AgentDriverError::new(
            "Managed feedback requires an existing absolute companion executable",
        ));
    }
    path.canonicalize()
        .map_err(|_| AgentDriverError::new("Managed feedback companion is unavailable"))
}

pub(crate) fn select(
    http: bool,
    companion: Option<&Path>,
) -> Result<Option<FeedbackTransport>, AgentDriverError> {
    if http {
        return Ok(Some(FeedbackTransport::Http));
    }
    companion
        .map(validate_companion)
        .transpose()
        .map(|path| path.map(|_| FeedbackTransport::Stdio))
}

pub(crate) fn server(
    transport: FeedbackTransport,
    endpoint: ManagedFeedbackEndpoint,
    companion: Option<&Path>,
) -> Result<McpServer, AgentDriverError> {
    match transport {
        FeedbackTransport::Http => Ok(McpServer::Http(
            McpServerHttp::new("rambledesk", endpoint.url).headers(vec![HttpHeader::new(
                "Authorization",
                format!("Bearer {}", endpoint.bearer_token),
            )]),
        )),
        FeedbackTransport::Stdio => {
            let companion = validate_companion(companion.ok_or_else(|| {
                AgentDriverError::new("Managed feedback companion is unavailable")
            })?)?;
            Ok(McpServer::Stdio(
                McpServerStdio::new("rambledesk", companion)
                    .args(vec!["managed-mcp-stdio".into()])
                    .env(vec![
                        EnvVariable::new(URL_ENV, endpoint.url),
                        EnvVariable::new(TOKEN_ENV, endpoint.bearer_token),
                    ]),
            ))
        }
        FeedbackTransport::PiExtension => Err(AgentDriverError::new(
            "Pi feedback requires its managed extension",
        )),
    }
}
