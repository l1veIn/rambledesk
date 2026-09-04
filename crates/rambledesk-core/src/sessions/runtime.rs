use super::SessionRecord;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SessionConnectionState {
    Stopped,
    Connecting,
    Connected,
    Disconnected,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SessionActivityState {
    Idle,
    Running,
    WaitingPermission,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentSessionCapabilities {
    pub load_session: bool,
    pub resume_session: bool,
    pub http_mcp: bool,
    #[serde(default)]
    pub prompt: super::AgentPromptCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionRuntime {
    pub connection: SessionConnectionState,
    pub activity: SessionActivityState,
    pub instance_id: Option<String>,
    pub config_updated_at: Option<String>,
    pub capabilities: AgentSessionCapabilities,
    #[serde(default)]
    pub configuration: super::SessionConfiguration,
    pub last_error: Option<String>,
}

impl Default for SessionRuntime {
    fn default() -> Self {
        Self {
            connection: SessionConnectionState::Stopped,
            activity: SessionActivityState::Idle,
            instance_id: None,
            config_updated_at: None,
            capabilities: AgentSessionCapabilities::default(),
            configuration: super::SessionConfiguration::default(),
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ManagedSessionSnapshot {
    pub recovery: Option<super::SessionRecovery>,
    pub session: SessionRecord,
    pub runtime: SessionRuntime,
    pub activities: Vec<super::SessionActivity>,
    pub permissions: Vec<super::SessionPermission>,
    pub deliveries: Vec<super::FeedbackDelivery>,
    pub deleting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentConnectionCheck {
    pub ok: bool,
    pub message: String,
    pub details: Vec<String>,
}
