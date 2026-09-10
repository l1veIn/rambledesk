use super::AgentSessionCapabilities;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentCheckConnection {
    Connected,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentConnectionCheck {
    /// Retains the managed feedback requirement; use connection for the handshake fact.
    pub ok: bool,
    #[ts(as = "Option<AgentCheckConnection>", optional)]
    pub connection: AgentCheckConnection,
    pub message: String,
    pub details: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub failure: Option<super::AgentFailure>,
}

impl AgentConnectionCheck {
    pub fn connected(caps: &AgentSessionCapabilities) -> Self {
        let feedback = caps.feedback_transport.is_some();
        Self {
            ok: feedback,
            connection: AgentCheckConnection::Connected,
            message: if feedback {
                "ACP handshake passed and the feedback command is available. Model access and Ramble handoff still require an actual session."
            } else {
                "ACP connected, but no managed feedback transport is configured for this Agent"
            }
            .into(),
            details: vec![format!(
                "Load: {}; resume: {}; HTTP MCP: {}; managed feedback: {}",
                caps.load_session,
                caps.resume_session,
                caps.http_mcp,
                caps.feedback_transport
                    .map(|transport| transport.as_str())
                    .unwrap_or("unavailable")
            )],
            failure: (!feedback).then(|| {
                super::AgentFailure::new(
                    super::AgentFailureStage::Launch,
                    super::AgentFailureReason::Configuration,
                    "RambleDesk managed feedback companion is unavailable",
                )
            }),
        }
    }

    pub fn failed(failure: super::AgentFailure) -> Self {
        Self {
            ok: false,
            connection: AgentCheckConnection::Failed,
            message: failure.message.clone(),
            details: vec![],
            failure: Some(failure),
        }
    }
}
