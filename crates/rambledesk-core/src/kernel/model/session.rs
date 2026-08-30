use serde::{Deserialize, Serialize};

use super::{AcpSessionLinkId, SessionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessMode {
    ReadOnly,
    WorkspaceWrite,
    Yolo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Managed,
    Connected,
}

/// Durable lifecycle only. Running and waiting-for-feedback are projections
/// from live Agent state and durable Feedback Requests respectively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLifecycle {
    Ready,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchConfiguration {
    pub agent_profile_id: String,
    pub launch_profile_id: String,
    pub workspace_reference: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub access_mode: AccessMode,
    /// Opaque, versioned Agent configuration. JSON validation belongs to the
    /// protocol/persistence Adapter, not the domain Module.
    pub agent_config_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: SessionId,
    pub kind: SessionKind,
    pub title: String,
    pub lifecycle: SessionLifecycle,
    pub launch_configuration: Option<LaunchConfiguration>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpSessionLinkObservation {
    pub session_id: SessionId,
    pub agent_profile_id: String,
    pub launch_profile_id: String,
    pub acp_session_id: String,
    pub capabilities_json: String,
    pub session_toolset_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentObservation {
    AcpSessionLinked(AcpSessionLinkObservation),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpSessionLinkSnapshot {
    pub link_id: AcpSessionLinkId,
    pub session_id: SessionId,
    pub agent_profile_id: String,
    pub launch_profile_id: String,
    pub acp_session_id: String,
    pub capabilities_json: String,
    pub session_toolset_digest: String,
    pub is_current: bool,
    pub created_at: String,
    pub last_used_at: String,
}
