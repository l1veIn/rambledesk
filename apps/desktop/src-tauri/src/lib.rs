use std::path::PathBuf;

use rambledesk_core::HealthSnapshot;
use rambledesk_mcp::{AccessToken, ServerConfig, ServerHandle, start_server};
use tauri::{Manager, RunEvent};

struct McpState {
    handle: ServerHandle,
}

#[tauri::command]
fn get_health() -> HealthSnapshot {
    rambledesk_storage::health_snapshot()
}

#[tauri::command]
fn get_mcp_endpoint(state: tauri::State<'_, McpState>) -> String {
    state.handle.endpoint().to_owned()
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

fn token_path(app: &tauri::App) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|root| root.join("auth").join("mcp.token"))
        .map_err(|error| format!("failed to resolve app data directory: {error}"))
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
        .setup(|app| {
            let token = AccessToken::load_or_create(&token_path(app)?)?;
            let config = ServerConfig::new(token).with_port(configured_port()?);
            let handle = tauri::async_runtime::block_on(start_server(config))?;
            app.manage(McpState { handle });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_health, get_mcp_endpoint])
        .build(tauri::generate_context!())
        .expect("failed to build RambleDesk desktop app");

    app.run(|app_handle, event| {
        if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. })
            && let Some(state) = app_handle.try_state::<McpState>()
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
}
