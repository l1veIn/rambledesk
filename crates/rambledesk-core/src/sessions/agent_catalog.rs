//! Agent catalog contracts. Inspired by Codeg 3ebdfed's registry/preflight model
//! (Apache-2.0; see THIRD_PARTY_NOTICES);
//! RambleDesk separates installation evidence from managed-feedback verification.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::SaveAgentConfigInput;
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentConnectionKind {
    Native,
    Bridge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentDistribution {
    Npm {
        package: String,
        pinned_version: String,
        command: String,
        node_required: String,
    },
    Manual {
        command: String,
        version: String,
        instructions: String,
        docs_url: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentVerificationStatus {
    Verified,
    Unsupported,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentVerification {
    pub status: AgentVerificationStatus,
    pub versions: Vec<String>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentDependency {
    pub command: String,
    pub required: bool,
    pub package: Option<String>,
    pub pinned_version: Option<String>,
    pub instructions: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentCatalogEntry {
    pub id: String,
    pub name: String,
    pub host_id: String,
    pub description: String,
    pub connection_kind: AgentConnectionKind,
    pub distribution: AgentDistribution,
    pub args: Vec<String>,
    pub dependencies: Vec<AgentDependency>,
    pub verification: AgentVerification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentInstallSource {
    Managed,
    System,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentCheckStatus {
    Pass,
    Fail,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentCatalogCheck {
    pub id: String,
    pub status: AgentCheckStatus,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentDependencyInspection {
    pub command: String,
    pub required: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentInspection {
    pub agent_id: String,
    pub source: AgentInstallSource,
    pub version: Option<String>,
    pub command: Option<String>,
    pub args: Vec<String>,
    /// Launch defaults for this installation, never inherited account credentials.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub env: Option<std::collections::BTreeMap<String, String>>,
    pub dependencies: Vec<AgentDependencyInspection>,
    pub checks: Vec<AgentCatalogCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct InstallAgentInput {
    pub agent_id: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentInstallPhase {
    Preparing,
    Installing,
    Verifying,
    Complete,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentInstallProgress {
    pub phase: AgentInstallPhase,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct InstalledAgent {
    pub agent_id: String,
    pub version: String,
    pub config: SaveAgentConfigInput,
}

pub type AgentInstallObserver = Arc<dyn Fn(AgentInstallProgress) + Send + Sync>;

#[async_trait]
pub trait AgentCatalogProvider: Send + Sync {
    fn catalog(&self) -> Vec<AgentCatalogEntry>;
    async fn inspect(&self, agent_id: &str) -> Result<AgentInspection, super::AgentDriverError>;
    async fn install(
        &self,
        input: InstallAgentInput,
        on_progress: AgentInstallObserver,
    ) -> Result<InstalledAgent, super::AgentDriverError>;
    /// Signal the active install for this catalog entry. The install future
    /// finishes only after its owned subprocess and incomplete files are cleaned.
    async fn cancel_install(&self, agent_id: &str) -> Result<(), super::AgentDriverError>;
}
