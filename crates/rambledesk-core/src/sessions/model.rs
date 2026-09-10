use std::{collections::BTreeMap, fmt};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SessionProtocol {
    Acp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum SessionManagement {
    External,
    Managed {
        protocol: SessionProtocol,
        agent_config_id: String,
        cwd: String,
        remote_session_id: Option<String>,
    },
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentConfig {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub catalog_id: Option<String>,
    pub name: String,
    pub host_id: String,
    pub protocol: SessionProtocol,
    pub enabled: bool,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub created_at: String,
    pub updated_at: String,
}

// Both argument lists and environment values may contain user credentials.
// The serializable configuration remains editable, but Debug must not expose it.
impl fmt::Debug for AgentConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgentConfig")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("host_id", &self.host_id)
            .field("protocol", &self.protocol)
            .field("enabled", &self.enabled)
            .field("command", &self.command)
            .field("argument_count", &self.args.len())
            .field("environment_keys", &self.env.keys().collect::<Vec<_>>())
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SaveAgentConfigInput {
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub catalog_id: Option<String>,
    pub name: String,
    pub host_id: String,
    pub protocol: SessionProtocol,
    pub enabled: bool,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

impl fmt::Debug for SaveAgentConfigInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SaveAgentConfigInput")
            .field("id", &self.id)
            .field("host_id", &self.host_id)
            .field("protocol", &self.protocol)
            .field("enabled", &self.enabled)
            .field("argument_count", &self.args.len())
            .field("environment_keys", &self.env.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentConfigInput {
    pub agent_config_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionRecord {
    // Stable local identity, backed by host_sessions.id.
    pub session_id: String,
    pub host_id: String,
    // Correlation identity used by the feedback adapters, not an ACP remote id.
    pub host_session_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lifecycle: Option<SessionLifecycle>,
    pub created_at: String,
    pub updated_at: String,
    pub management: SessionManagement,
}

impl SessionRecord {
    /// Legacy records and snapshots without this field are active conversations.
    pub fn is_prepared(&self) -> bool {
        self.lifecycle == Some(SessionLifecycle::Prepared)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SessionLifecycle {
    Prepared,
    #[default]
    Active,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct PrepareManagedSessionInput {
    pub agent_config_id: String,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct CreateManagedSessionInput {
    pub agent_config_id: String,
    pub cwd: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ManagedSessionInput {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewManagedSession {
    pub session_id: String,
    pub agent_config_id: String,
    pub cwd: String,
    pub title: String,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_configuration_debug_does_not_expose_credentials() {
        let config = AgentConfig {
            catalog_id: None,
            id: "config".into(),
            name: "Test".into(),
            host_id: "dsh".into(),
            protocol: SessionProtocol::Acp,
            enabled: true,
            command: "agent".into(),
            args: vec!["secret-argument".into()],
            env: BTreeMap::from([("TOKEN".into(), "secret-environment".into())]),
            created_at: "now".into(),
            updated_at: "now".into(),
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("secret-argument"));
        assert!(!debug.contains("secret-environment"));
        assert!(debug.contains("TOKEN"));
    }
}
