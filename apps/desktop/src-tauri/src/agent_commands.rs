use crate::WorkbenchState;
use rambledesk_core::{
    AgentCatalogEntry, AgentConfig, AgentInspection, AgentInstallJob, AgentInstallJobInput,
    AgentManagementError, CatalogAgentInput, InstallAgentInput, ResolveCatalogAgentInput,
};

#[tauri::command]
pub(crate) async fn resolve_catalog_agent(
    state: tauri::State<'_, WorkbenchState>,
    input: ResolveCatalogAgentInput,
) -> Result<AgentConfig, AgentManagementError> {
    state
        .application_commands
        .resolve_catalog_agent(input)
        .await
}

#[tauri::command]
pub(crate) fn list_available_agents(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<AgentCatalogEntry>, AgentManagementError> {
    state.application_commands.list_available_agents()
}
#[tauri::command]
pub(crate) async fn inspect_agent_installation(
    state: tauri::State<'_, WorkbenchState>,
    input: CatalogAgentInput,
) -> Result<AgentInspection, AgentManagementError> {
    state
        .application_commands
        .inspect_agent_installation(input)
        .await
}
#[tauri::command]
pub(crate) fn list_agent_install_jobs(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<AgentInstallJob>, AgentManagementError> {
    state.application_commands.list_agent_install_jobs()
}
#[tauri::command]
pub(crate) async fn install_agent(
    state: tauri::State<'_, WorkbenchState>,
    input: InstallAgentInput,
) -> Result<AgentInstallJob, AgentManagementError> {
    state.application_commands.install_agent(input)
}
#[tauri::command]
pub(crate) async fn cancel_agent_install(
    state: tauri::State<'_, WorkbenchState>,
    input: AgentInstallJobInput,
) -> Result<(), AgentManagementError> {
    state.application_commands.cancel_agent_install(input).await
}
