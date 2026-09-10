use super::{ApplicationCommandFacade, ManagedCommandErrorCode};
use crate::{
    AgentCatalogEntry, AgentConfig, AgentInspection, AgentInstallJob, AgentInstallJobInput,
    AgentManagementApplication, CatalogAgentInput, InstallAgentInput, ResolveCatalogAgentInput,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct AgentManagementError {
    pub code: ManagedCommandErrorCode,
    pub message: String,
    pub retryable: bool,
}
impl From<crate::AgentDriverError> for AgentManagementError {
    fn from(error: crate::AgentDriverError) -> Self {
        Self {
            code: ManagedCommandErrorCode::AgentOperationFailed,
            message: error.message,
            retryable: true,
        }
    }
}
impl ApplicationCommandFacade {
    pub async fn resolve_catalog_agent(
        &self,
        input: ResolveCatalogAgentInput,
    ) -> Result<AgentConfig, AgentManagementError> {
        let sessions = self.sessions.as_ref().ok_or_else(|| AgentManagementError {
            code: ManagedCommandErrorCode::ManagedRuntimeUnavailable,
            message: "Managed session runtime is unavailable".into(),
            retryable: false,
        })?;
        self.agent_management()?
            .resolve_configuration(sessions, input)
            .await
            .map_err(Into::into)
    }
    pub fn with_agent_management(mut self, agents: AgentManagementApplication) -> Self {
        self.agents = Some(agents);
        self
    }
    fn agent_management(&self) -> Result<&AgentManagementApplication, AgentManagementError> {
        self.agents.as_ref().ok_or_else(|| AgentManagementError {
            code: ManagedCommandErrorCode::ManagedRuntimeUnavailable,
            message: "Agent management is unavailable in this runtime".into(),
            retryable: false,
        })
    }
    pub fn list_available_agents(&self) -> Result<Vec<AgentCatalogEntry>, AgentManagementError> {
        Ok(self.agent_management()?.catalog())
    }
    pub async fn inspect_agent_installation(
        &self,
        input: CatalogAgentInput,
    ) -> Result<AgentInspection, AgentManagementError> {
        self.agent_management()?
            .inspect(input)
            .await
            .map_err(Into::into)
    }
    pub fn list_agent_install_jobs(&self) -> Result<Vec<AgentInstallJob>, AgentManagementError> {
        Ok(self.agent_management()?.jobs())
    }
    pub fn install_agent(
        &self,
        input: InstallAgentInput,
    ) -> Result<AgentInstallJob, AgentManagementError> {
        self.agent_management()?
            .start_install(input)
            .map_err(Into::into)
    }
    pub async fn cancel_agent_install(
        &self,
        input: AgentInstallJobInput,
    ) -> Result<(), AgentManagementError> {
        self.agent_management()?
            .cancel(input)
            .await
            .map_err(Into::into)
    }
}
