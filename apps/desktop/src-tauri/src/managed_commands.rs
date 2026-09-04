use rambledesk_core::{
    AgentConfig, AgentConfigInput, AgentConnectionCheck, CreateManagedSessionInput,
    ManagedCommandError, ManagedSessionInput, ManagedSessionSnapshot, ResolveFeedbackDeliveryInput,
    RespondManagedPermissionInput, SaveAgentConfigInput, SendManagedPromptInput,
};

use crate::WorkbenchState;

#[tauri::command]
pub(crate) async fn list_agent_configs(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<AgentConfig>, ManagedCommandError> {
    state.application_commands.list_agent_configs().await
}

#[tauri::command]
pub(crate) async fn save_agent_config(
    state: tauri::State<'_, WorkbenchState>,
    input: SaveAgentConfigInput,
) -> Result<AgentConfig, ManagedCommandError> {
    state.application_commands.save_agent_config(input).await
}

#[tauri::command]
pub(crate) async fn delete_agent_config(
    state: tauri::State<'_, WorkbenchState>,
    input: AgentConfigInput,
) -> Result<(), ManagedCommandError> {
    state.application_commands.delete_agent_config(input).await
}

#[tauri::command]
pub(crate) async fn check_agent_config(
    state: tauri::State<'_, WorkbenchState>,
    input: AgentConfigInput,
) -> Result<AgentConnectionCheck, ManagedCommandError> {
    state.application_commands.check_agent_config(input).await
}

#[tauri::command]
pub(crate) async fn create_managed_session(
    state: tauri::State<'_, WorkbenchState>,
    input: CreateManagedSessionInput,
) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
    state
        .application_commands
        .create_managed_session(input)
        .await
}

#[tauri::command]
pub(crate) async fn get_managed_session(
    state: tauri::State<'_, WorkbenchState>,
    input: ManagedSessionInput,
) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
    state.application_commands.get_managed_session(input).await
}

#[tauri::command]
pub(crate) async fn start_managed_session(
    state: tauri::State<'_, WorkbenchState>,
    input: ManagedSessionInput,
) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
    state
        .application_commands
        .start_managed_session(input)
        .await
}

#[tauri::command]
pub(crate) async fn stop_managed_session(
    state: tauri::State<'_, WorkbenchState>,
    input: ManagedSessionInput,
) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
    state.application_commands.stop_managed_session(input).await
}

#[tauri::command]
pub(crate) async fn send_managed_prompt(
    state: tauri::State<'_, WorkbenchState>,
    input: SendManagedPromptInput,
) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
    state.application_commands.send_managed_prompt(input).await
}

#[tauri::command]
pub(crate) async fn cancel_managed_prompt(
    state: tauri::State<'_, WorkbenchState>,
    input: ManagedSessionInput,
) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
    state
        .application_commands
        .cancel_managed_prompt(input)
        .await
}

#[tauri::command]
pub(crate) async fn respond_managed_permission(
    state: tauri::State<'_, WorkbenchState>,
    input: RespondManagedPermissionInput,
) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
    state
        .application_commands
        .respond_managed_permission(input)
        .await
}

#[tauri::command]
pub(crate) async fn resolve_feedback_delivery(
    state: tauri::State<'_, WorkbenchState>,
    input: ResolveFeedbackDeliveryInput,
) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
    state
        .application_commands
        .resolve_feedback_delivery(input)
        .await
}

#[tauri::command]
pub(crate) async fn delete_managed_session(
    state: tauri::State<'_, WorkbenchState>,
    input: ManagedSessionInput,
) -> Result<(), ManagedCommandError> {
    state
        .application_commands
        .delete_managed_session(input)
        .await
}
