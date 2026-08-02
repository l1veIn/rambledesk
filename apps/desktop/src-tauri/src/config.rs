use std::path::PathBuf;

use rambledesk_local_server::{AccessToken, DEFAULT_PORT, default_token_path};
use tauri::Manager;

pub(super) fn configured_port() -> Result<u16, String> {
    match std::env::var("RAMBLEDESK_LOCAL_SERVER_PORT") {
        Ok(value) => value.parse().map_err(|_| {
            "RAMBLEDESK_LOCAL_SERVER_PORT must be an unsigned 16-bit integer".to_owned()
        }),
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_PORT),
        Err(error) => Err(format!(
            "failed to read RAMBLEDESK_LOCAL_SERVER_PORT: {error}"
        )),
    }
}

pub(super) fn configured_path(
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

pub(super) fn configured_database_path() -> Result<PathBuf, String> {
    configured_path("RAMBLEDESK_DATABASE_FILE", || {
        rambledesk_storage::default_database_path().map_err(|error| error.to_string())
    })
}

pub(super) fn configured_token_path() -> Result<PathBuf, String> {
    configured_path("RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE", || {
        default_token_path().map_err(|error| error.to_string())
    })
}

pub(super) fn configured_speech_model_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    configured_path("RAMBLEDESK_SHERPA_MODEL_DIR", || {
        app.path()
            .app_local_data_dir()
            .map(|directory| directory.join("models").join("sherpa-x-asr"))
            .map_err(|error| format!("无法确定 Sherpa 模型目录：{error}"))
    })
}

pub(super) fn generic_mcp_configuration(endpoint: &str, token: &AccessToken) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "rambledesk": {
                "type": "http",
                "url": endpoint,
                "headers": {
                    "Authorization": format!("Bearer {}", token.secret())
                }
            }
        }
    }))
    .expect("static MCP configuration must serialize")
}
