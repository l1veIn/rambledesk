use rambledesk_core::{
    ApplicationError, DraftView, FeedbackApplication, FeedbackRequestSummary, FeedbackRequestView,
    FeedbackWorkspaceView, HealthSnapshot, SaveDraftInput, SubmitFeedbackInput,
};
use rambledesk_mcp::{AccessToken, ServerConfig, ServerHandle, default_token_path, start_server};
use std::path::PathBuf;
use tauri::{Manager, RunEvent};

struct WorkbenchState {
    handle: ServerHandle,
    application: FeedbackApplication,
}

#[tauri::command]
fn get_health() -> HealthSnapshot {
    rambledesk_storage::health_snapshot()
}

#[tauri::command]
fn get_mcp_endpoint(state: tauri::State<'_, WorkbenchState>) -> String {
    state.handle.endpoint().to_owned()
}

#[tauri::command]
async fn list_feedback_inbox(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<FeedbackRequestSummary>, ApplicationError> {
    let application = state.application.clone();
    application.list_open_feedback_requests().await
}

#[tauri::command]
async fn get_feedback_workspace(
    request_id: String,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackWorkspaceView, ApplicationError> {
    let application = state.application.clone();
    application.get_feedback_workspace(request_id).await
}

#[tauri::command]
async fn save_feedback_draft(
    input: SaveDraftInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<DraftView, ApplicationError> {
    let application = state.application.clone();
    application.save_feedback_draft(input).await
}

#[tauri::command]
async fn submit_feedback(
    input: SubmitFeedbackInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackRequestView, ApplicationError> {
    let application = state.application.clone();
    application.submit_feedback(input).await
}

fn configured_port() -> Result<u16, String> {
    match std::env::var("RAMBLEDESK_MCP_PORT") {
        Ok(value) => value
            .parse()
            .map_err(|_| "RAMBLEDESK_MCP_PORT must be an unsigned 16-bit integer".to_owned()),
        Err(std::env::VarError::NotPresent) => Ok(rambledesk_mcp::DEFAULT_PORT),
        Err(error) => Err(format!("failed to read RAMBLEDESK_MCP_PORT: {error}")),
    }
}

fn configured_path(
    variable: &str,
    default: impl FnOnce() -> Result<PathBuf, String>,
) -> Result<PathBuf, String> {
    match std::env::var(variable) {
        Ok(value) => {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err(format!("{variable} must be an absolute path"));
            }
            Ok(path)
        }
        Err(std::env::VarError::NotPresent) => default(),
        Err(error) => Err(format!("failed to read {variable}: {error}")),
    }
}

fn configured_database_path() -> Result<PathBuf, String> {
    configured_path("RAMBLEDESK_DATABASE_FILE", || {
        rambledesk_storage::default_database_path().map_err(|error| error.to_string())
    })
}

fn configured_token_path() -> Result<PathBuf, String> {
    configured_path("RAMBLEDESK_TOKEN_FILE", || {
        default_token_path().map_err(|error| error.to_string())
    })
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rambledesk=info".into()),
        )
        .with_target(false)
        .init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let token = AccessToken::load_or_create(&configured_token_path()?)?;
            let database_path = configured_database_path()?;
            let store = tauri::async_runtime::block_on(
                rambledesk_storage::SqliteFeedbackStore::connect(&database_path),
            )?;
            let application = store.into_application();
            let config = ServerConfig::new(token).with_port(configured_port()?);
            let handle = tauri::async_runtime::block_on(start_server(config, application.clone()))?;
            app.manage(WorkbenchState {
                handle,
                application,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_health,
            get_mcp_endpoint,
            list_feedback_inbox,
            get_feedback_workspace,
            save_feedback_draft,
            submit_feedback,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build RambleDesk desktop app");

    app.run(|app_handle, event| {
        if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. })
            && let Some(state) = app_handle.try_state::<WorkbenchState>()
        {
            state.handle.cancel();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_port_is_stable_when_env_is_absent() {
        // The environment is intentionally not mutated because tests may run concurrently.
        if std::env::var_os("RAMBLEDESK_MCP_PORT").is_none() {
            assert_eq!(configured_port().expect("default port"), 37_642);
        }
    }

    #[test]
    fn configured_paths_default_when_overrides_are_absent() {
        if std::env::var_os("RAMBLEDESK_DATABASE_FILE").is_none() {
            assert_eq!(
                configured_database_path().expect("default database"),
                rambledesk_storage::default_database_path().expect("storage default")
            );
        }
        if std::env::var_os("RAMBLEDESK_TOKEN_FILE").is_none() {
            assert_eq!(
                configured_token_path().expect("default token"),
                default_token_path().expect("token default")
            );
        }
    }
}
