use crate::WorkbenchState;
use rambledesk_core::{
    ApplicationError, OpenTerminalSessionInput, ReadTerminalSessionInput,
    ResizeTerminalSessionInput, TerminalSessionInput, TerminalSessionSnapshot,
    WriteTerminalSessionInput,
};

#[tauri::command]
pub(crate) async fn open_terminal_session(
    state: tauri::State<'_, WorkbenchState>,
    input: OpenTerminalSessionInput,
) -> Result<TerminalSessionSnapshot, ApplicationError> {
    state
        .application_commands
        .open_terminal_session(input)
        .await
}
#[tauri::command]
pub(crate) async fn read_terminal_session(
    state: tauri::State<'_, WorkbenchState>,
    input: ReadTerminalSessionInput,
) -> Result<TerminalSessionSnapshot, ApplicationError> {
    state
        .application_commands
        .read_terminal_session(input)
        .await
}
#[tauri::command]
pub(crate) async fn write_terminal_session(
    state: tauri::State<'_, WorkbenchState>,
    input: WriteTerminalSessionInput,
) -> Result<TerminalSessionSnapshot, ApplicationError> {
    state
        .application_commands
        .write_terminal_session(input)
        .await
}
#[tauri::command]
pub(crate) async fn resize_terminal_session(
    state: tauri::State<'_, WorkbenchState>,
    input: ResizeTerminalSessionInput,
) -> Result<TerminalSessionSnapshot, ApplicationError> {
    state
        .application_commands
        .resize_terminal_session(input)
        .await
}
#[tauri::command]
pub(crate) async fn stop_terminal_session(
    state: tauri::State<'_, WorkbenchState>,
    input: TerminalSessionInput,
) -> Result<TerminalSessionSnapshot, ApplicationError> {
    state
        .application_commands
        .stop_terminal_session(input)
        .await
}
