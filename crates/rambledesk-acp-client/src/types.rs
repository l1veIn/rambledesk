use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use rambledesk_core::kernel::SessionId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;

/// Stable identity used to resolve a configured launch profile.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LaunchProfileRef {
    pub agent_profile_id: String,
    pub launch_profile_id: String,
}

/// Concrete process launch knowledge. This is local configuration, never ACP
/// wire data and never part of the domain Session identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchProfile {
    pub profile_ref: LaunchProfileRef,
    pub command: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Agent-specific mapping from RambleDesk's small Launch form onto the
    /// config options advertised by this ACP server. The client applies only
    /// mappings declared here; an unknown Agent never receives guessed access
    /// or model settings.
    #[serde(default)]
    pub configuration: LaunchConfigurationPolicy,
    /// Whether this Agent can receive RambleDesk's per-Session MCP Toolset.
    /// A successful ACP handshake is not enough to launch a structured Ramble
    /// when the Agent rejects or ignores client-provided MCP servers.
    #[serde(default = "session_toolset_required")]
    pub session_toolset: SessionToolsetPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigOptionSelector {
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
}

impl ConfigOptionSelector {
    pub fn new(ids: &[&str], categories: &[&str]) -> Self {
        Self {
            ids: ids.iter().map(|value| (*value).to_owned()).collect(),
            categories: categories.iter().map(|value| (*value).to_owned()).collect(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessModeConfiguration {
    #[serde(default)]
    pub transport: AccessModeTransport,
    pub selector: Option<ConfigOptionSelector>,
    #[serde(default)]
    pub read_only: Vec<String>,
    #[serde(default)]
    pub workspace_write: Vec<String>,
    #[serde(default)]
    pub yolo: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessModeTransport {
    #[default]
    ConfigOption,
    /// The selected values are complete process arguments that must be
    /// prepended before the Agent's ACP subcommand. Grok uses this path because
    /// it exposes permission mode only as a root-level CLI flag.
    ProcessArguments,
    /// The Agent owns its approval prompts but exposes no mutable access-mode
    /// selector. RambleDesk offers only its safe, approval-gated default.
    ImplicitWorkspaceWrite,
}

impl AccessModeConfiguration {
    pub fn mapped(
        selector: ConfigOptionSelector,
        read_only: &[&str],
        workspace_write: &[&str],
        yolo: &[&str],
    ) -> Self {
        Self {
            transport: AccessModeTransport::ConfigOption,
            selector: Some(selector),
            read_only: read_only.iter().map(|value| (*value).to_owned()).collect(),
            workspace_write: workspace_write
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            yolo: yolo.iter().map(|value| (*value).to_owned()).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchConfigurationPolicy {
    pub model: Option<ConfigOptionSelector>,
    pub reasoning_effort: Option<ConfigOptionSelector>,
    #[serde(default)]
    pub access_mode: AccessModeConfiguration,
}

impl Default for LaunchConfigurationPolicy {
    fn default() -> Self {
        Self {
            model: Some(ConfigOptionSelector::new(&["model"], &["model"])),
            reasoning_effort: Some(ConfigOptionSelector::new(
                &["reasoning_effort", "thought_level"],
                &["reasoning_effort", "thought_level"],
            )),
            access_mode: AccessModeConfiguration::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionToolsetPolicy {
    #[default]
    Required,
    Unsupported,
}

fn session_toolset_required() -> SessionToolsetPolicy {
    SessionToolsetPolicy::Required
}

impl LaunchProfile {
    pub fn for_builtin(
        spec: &crate::BuiltinAgentSpec,
        command: PathBuf,
        args: Vec<String>,
        extra_env: BTreeMap<String, String>,
    ) -> Self {
        let mut env = match spec.distribution {
            crate::BuiltinAgentDistribution::Npm { env, .. }
            | crate::BuiltinAgentDistribution::Binary { env, .. } => env
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect::<BTreeMap<_, _>>(),
        };
        env.extend(extra_env);
        if spec.id == "codex" {
            env.insert("DISABLE_MCP_CONFIG_FILTERING".to_owned(), "true".to_owned());
        }
        let access = spec.access_modes;
        let selector = (!access.selector_ids.is_empty() || !access.selector_categories.is_empty())
            .then(|| ConfigOptionSelector::new(access.selector_ids, access.selector_categories));
        Self {
            profile_ref: LaunchProfileRef {
                agent_profile_id: spec.id.to_owned(),
                // Preserve the profile identity already persisted by the
                // Codex-only ACP-first prototype.
                launch_profile_id: if spec.id == "codex" {
                    "codex-acp-npx".to_owned()
                } else {
                    format!("{}-acp-managed", spec.id)
                },
            },
            command,
            args,
            env,
            configuration: LaunchConfigurationPolicy {
                model: Some(ConfigOptionSelector::new(&["model"], &["model"])),
                reasoning_effort: Some(ConfigOptionSelector::new(
                    &["reasoning_effort", "thought_level", "reasoning"],
                    &["reasoning_effort", "thought_level"],
                )),
                access_mode: AccessModeConfiguration {
                    transport: match spec.id {
                        "grok" => AccessModeTransport::ProcessArguments,
                        "code_buddy" => AccessModeTransport::ImplicitWorkspaceWrite,
                        _ => AccessModeTransport::ConfigOption,
                    },
                    selector,
                    read_only: access
                        .read_only
                        .iter()
                        .map(|value| (*value).to_owned())
                        .collect(),
                    workspace_write: access
                        .workspace_write
                        .iter()
                        .map(|value| (*value).to_owned())
                        .collect(),
                    yolo: access
                        .yolo
                        .iter()
                        .map(|value| (*value).to_owned())
                        .collect(),
                },
            },
            session_toolset: if spec.supports_session_mcp {
                SessionToolsetPolicy::Required
            } else {
                SessionToolsetPolicy::Unsupported
            },
        }
    }

    /// Current first-party Codex ACP Adapter used by RambleDesk. The version is
    /// pinned so wire behaviour does not change during an App release.
    pub fn codex_npx() -> Self {
        let mut env = BTreeMap::new();
        env.insert(
            "DISABLE_MCP_CONFIG_FILTERING".to_string(),
            "true".to_string(),
        );
        Self {
            profile_ref: LaunchProfileRef {
                agent_profile_id: "codex".to_string(),
                launch_profile_id: "codex-acp-npx".to_string(),
            },
            command: PathBuf::from("npx"),
            args: vec![
                "-y".to_string(),
                "@agentclientprotocol/codex-acp@1.7.0".to_string(),
            ],
            env,
            configuration: LaunchConfigurationPolicy {
                model: Some(ConfigOptionSelector::new(&["model"], &["model"])),
                reasoning_effort: Some(ConfigOptionSelector::new(
                    &["reasoning_effort", "thought_level"],
                    &["reasoning_effort", "thought_level"],
                )),
                access_mode: AccessModeConfiguration::mapped(
                    ConfigOptionSelector::new(&["mode"], &["mode"]),
                    &[],
                    &["read-only"],
                    &["agent-full-access"],
                ),
            },
            session_toolset: SessionToolsetPolicy::Required,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_profile_preserves_the_per_run_session_toolset() {
        let profile = LaunchProfile::codex_npx();
        assert_eq!(
            profile.env.get("DISABLE_MCP_CONFIG_FILTERING"),
            Some(&"true".to_string())
        );
    }
}

#[derive(Debug, Clone)]
pub struct AcpClientConfig {
    pub profiles: Vec<LaunchProfile>,
    /// Separate budget for first-run package acquisition and ACP capability
    /// probing. Normal Session RPCs retain the shorter operation timeout.
    pub preflight_timeout: Duration,
    pub operation_timeout: Duration,
    pub shutdown_grace: Duration,
    pub event_capacity: usize,
}

impl Default for AcpClientConfig {
    fn default() -> Self {
        Self {
            profiles: vec![LaunchProfile::codex_npx()],
            preflight_timeout: Duration::from_secs(120),
            operation_timeout: Duration::from_secs(20),
            shutdown_grace: Duration::from_secs(2),
            event_capacity: 256,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionScope {
    pub session_id: SessionId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitySnapshot {
    pub protocol_version: u32,
    pub load_session: bool,
    pub resume_session: bool,
    pub close_session: bool,
    pub mcp_http: bool,
    pub elicitation_form: bool,
    pub raw_agent_capabilities: Value,
}

/// One ordered value selected from the Agent's Launch Schema. Values remain
/// opaque to callers: ACP currently defines string select values and boolean
/// toggles, while this shape can preserve future JSON scalar kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchConfigSelection {
    pub id: String,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchConfigSource {
    Agent,
    Profile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchSelectOption {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchSelectGroup {
    pub id: String,
    pub name: String,
    pub options: Vec<LaunchSelectOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LaunchConfigKind {
    Select {
        #[serde(rename = "currentValue")]
        current_value: String,
        options: Vec<LaunchSelectOption>,
        groups: Vec<LaunchSelectGroup>,
    },
    Boolean {
        #[serde(rename = "currentValue")]
        current_value: bool,
    },
    Unsupported {
        #[serde(rename = "rawType")]
        raw_type: String,
        #[serde(rename = "currentValue")]
        current_value: Value,
        raw: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchConfigOption {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub source: LaunchConfigSource,
    #[serde(flatten)]
    pub kind: LaunchConfigKind,
}

/// Durable, versioned projection of Launch selections. The advertised schema
/// is deliberately not persisted: a resumed Agent remains the authority for
/// its current option descriptions and choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLaunchConfig {
    pub version: u32,
    pub schema_digest: String,
    pub values: Vec<LaunchConfigSelection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreflightReport {
    pub profile_ref: LaunchProfileRef,
    pub available: bool,
    pub agent_version: Option<String>,
    pub capabilities: CapabilitySnapshot,
    pub config_options: Vec<LaunchConfigOption>,
    pub schema_digest: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryMethod {
    New,
    Resume,
    Load,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Ready,
    Running,
    WaitingForPermission,
    WaitingForQuestion,
    Stopped,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagedSessionSnapshot {
    pub session_id: SessionId,
    pub acp_session_id: String,
    pub recovery_method: RecoveryMethod,
    pub capabilities: CapabilitySnapshot,
    pub config_options: Vec<Value>,
    pub state: RunState,
    pub permissions: Vec<PermissionRequest>,
    pub questions: Vec<AskQuestion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub live_request_id: String,
    pub session_id: SessionId,
    pub tool_call: Value,
    pub request_meta: Value,
    pub options: Vec<PermissionOption>,
    pub queue_position: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AskFieldKind {
    Text,
    Boolean,
    Number,
    Integer,
    SingleSelect,
    MultiSelect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AskField {
    pub field_id: String,
    pub title: String,
    pub description: Option<String>,
    pub kind: AskFieldKind,
    pub required: bool,
    pub secret: bool,
    pub options: Vec<AskOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AskOption {
    pub label: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AskQuestion {
    pub live_request_id: String,
    pub session_id: SessionId,
    pub tool_call_id: Option<String>,
    pub message: String,
    pub fields: Vec<AskField>,
    pub queue_position: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionAnswer {
    pub session_id: SessionId,
    pub live_request_id: String,
    pub option_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionAction {
    Accept,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestionAnswer {
    pub session_id: SessionId,
    pub live_request_id: String,
    pub action: QuestionAction,
    pub content: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveAnswerOutcome {
    pub live_request_id: String,
    pub accepted: bool,
    pub remaining: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelOutcome {
    pub session_id: SessionId,
    pub notification_sent: bool,
    pub live_requests_cancelled: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShutdownOutcome {
    pub runs_stopped: usize,
    pub forced_process_trees: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LiveSessionEvent {
    StateChanged {
        session_id: SessionId,
        state: RunState,
    },
    SessionUpdate {
        session_id: SessionId,
        update: Value,
    },
    PermissionQueued {
        request: PermissionRequest,
    },
    PermissionResolved {
        session_id: SessionId,
        live_request_id: String,
    },
    QuestionQueued {
        question: AskQuestion,
    },
    QuestionResolved {
        session_id: SessionId,
        live_request_id: String,
    },
    Disconnected {
        session_id: SessionId,
        reason: String,
    },
}

pub type LiveSessionEventReceiver = broadcast::Receiver<LiveSessionEvent>;
