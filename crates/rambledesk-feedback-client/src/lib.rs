//! A short-lived command client. It never initializes the desktop, owns a
//! database, selects a session, or falls back to the external feedback API.
mod command;

use std::{net::IpAddr, time::Duration};

use rambledesk_core::ManagedFeedbackEndpoint;
use serde_json::Value;

pub const URL_ENV: &str = "RAMBLEDESK_FEEDBACK_URL";
pub const TOKEN_ENV: &str = "RAMBLEDESK_FEEDBACK_TOKEN";
pub const COMMAND_ENV: &str = "RAMBLEDESK_COMMAND";
pub const MANAGED_ENV: &str = "RAMBLEDESK_MANAGED_SESSION";
pub const MAX_INPUT_BYTES: usize = 96 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientError {
    MissingCapability,
    InvalidCapability,
    RevokedCapability,
    InvalidInput,
    InputUnavailable,
    UpstreamUnavailable,
    InvalidResponse,
    RuntimeUnavailable,
}

impl ClientError {
    pub fn json(self, request_id: Option<&str>) -> Value {
        let (code, message, retryable) = match self {
            Self::MissingCapability => (
                "missing_capability",
                "This command requires a RambleDesk-managed Agent session.",
                false,
            ),
            Self::InvalidCapability => (
                "invalid_capability",
                "The local feedback capability is invalid. Reconnect the Agent session.",
                false,
            ),
            Self::RevokedCapability => (
                "revoked_capability",
                "This feedback capability was revoked. Reconnect the Agent session and recover the original request_id.",
                false,
            ),
            Self::InvalidInput => (
                "invalid_input",
                "Provide valid feedback JSON using --input <file> or --input -. Use feedback --help for commands.",
                false,
            ),
            Self::InputUnavailable => (
                "input_unavailable",
                "The feedback input could not be read or exceeds 96 MiB.",
                false,
            ),
            Self::UpstreamUnavailable => (
                "upstream_unavailable",
                "Feedback delivery is uncertain. Recover using this same request_id before retrying. Do not create a replacement request.",
                true,
            ),
            Self::InvalidResponse => (
                "invalid_response",
                "The feedback response could not be read. Recover using the same request_id.",
                true,
            ),
            Self::RuntimeUnavailable => (
                "runtime_unavailable",
                "The feedback command could not start.",
                true,
            ),
        };
        let mut value =
            serde_json::json!({"code": code, "message": message, "retryable": retryable});
        if let Some(id) = request_id {
            value["request_id"] = id.into();
        }
        value
    }
}

pub fn endpoint_from_env() -> Result<ManagedFeedbackEndpoint, ClientError> {
    let endpoint = ManagedFeedbackEndpoint {
        url: std::env::var(URL_ENV).map_err(|_| ClientError::MissingCapability)?,
        bearer_token: std::env::var(TOKEN_ENV).map_err(|_| ClientError::MissingCapability)?,
    };
    validate_endpoint(&endpoint)?;
    Ok(endpoint)
}

pub fn validate_endpoint(endpoint: &ManagedFeedbackEndpoint) -> Result<reqwest::Url, ClientError> {
    let invalid = ClientError::InvalidCapability;
    let url = reqwest::Url::parse(&endpoint.url).map_err(|_| invalid)?;
    let host = url.host_str().ok_or(invalid)?.trim_matches(['[', ']']);
    if url.scheme() != "http"
        || !host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/agent-feedback"
        || url.port_or_known_default().is_none_or(|port| port == 0)
        || endpoint.bearer_token.len() != 64
        || !endpoint
            .bearer_token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid);
    }
    Ok(url)
}

/// Exactly one bounded HTTP call; waiting for the human belongs to the durable
/// server outbox, never to a child command process.
pub async fn call(
    endpoint: &ManagedFeedbackEndpoint,
    operation: &str,
    input: &Value,
) -> Result<(bool, Value), ClientError> {
    if !matches!(operation, "request" | "get" | "recover") {
        return Err(ClientError::InvalidInput);
    }
    let mut url = validate_endpoint(endpoint)?;
    url.set_path(&format!("/agent-feedback/{operation}"));
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| ClientError::RuntimeUnavailable)?;
    let mut response = client
        .post(url)
        .bearer_auth(&endpoint.bearer_token)
        .json(input)
        .send()
        .await
        .map_err(|_| ClientError::UpstreamUnavailable)?;
    let success = response.status().is_success();
    if matches!(response.status().as_u16(), 401 | 403) {
        return Err(ClientError::RevokedCapability);
    }
    if response.status().is_redirection() {
        return Err(ClientError::InvalidResponse);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ClientError::InvalidResponse)?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(ClientError::InvalidResponse);
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| ClientError::InvalidResponse)?;
    if !value.is_object() {
        return Err(ClientError::InvalidResponse);
    }
    Ok((success, value))
}

pub fn process_requested() -> bool {
    std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "feedback")
}

pub fn run_process() -> i32 {
    command::run()
}

#[cfg(test)]
mod tests;
