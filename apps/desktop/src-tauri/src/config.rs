use std::{
    fs,
    path::{Path, PathBuf},
};

use rambledesk_local_server::{AccessToken, DEFAULT_PORT, default_token_path};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct StoragePreferences {
    pub data_storage_path: Option<PathBuf>,
}

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

pub(super) fn storage_preferences_path() -> Result<PathBuf, String> {
    rambledesk_storage::default_app_data_root()
        .map(|root| root.join("settings.json"))
        .map_err(|error| error.to_string())
}

pub(super) fn load_storage_preferences() -> Result<StoragePreferences, String> {
    let path = storage_preferences_path()?;
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents)
            .map_err(|error| format!("无法读取存储设置 {}：{error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(StoragePreferences::default())
        }
        Err(error) => Err(format!("无法读取存储设置 {}：{error}", path.display())),
    }
}

pub(super) fn configured_library_path() -> Result<PathBuf, String> {
    configured_path("RAMBLEDESK_LIBRARY_DIR", || {
        let preferences = load_storage_preferences()?;
        preferences.data_storage_path.map_or_else(
            || rambledesk_storage::default_library_path().map_err(|error| error.to_string()),
            Ok,
        )
    })
}

pub(super) fn save_library_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("数据存储位置必须是绝对路径".to_owned());
    }
    fs::create_dir_all(path)
        .map_err(|error| format!("无法创建数据存储位置 {}：{error}", path.display()))?;
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("无法访问数据存储位置 {}：{error}", path.display()))?;
    let probe = canonical.join(".rambledesk-write-probe");
    fs::write(&probe, b"rambledesk")
        .map_err(|error| format!("数据存储位置不可写 {}：{error}", canonical.display()))?;
    let _ = fs::remove_file(probe);

    let settings_path = storage_preferences_path()?;
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("无法创建设置目录 {}：{error}", parent.display()))?;
    }
    let contents = serde_json::to_string_pretty(&StoragePreferences {
        data_storage_path: Some(canonical.clone()),
    })
    .map_err(|error| format!("无法序列化存储设置：{error}"))?;
    fs::write(&settings_path, format!("{contents}\n"))
        .map_err(|error| format!("无法写入存储设置 {}：{error}", settings_path.display()))?;
    Ok(canonical)
}

pub(super) fn configured_speech_model_path(library_root: &Path) -> Result<PathBuf, String> {
    configured_path("RAMBLEDESK_SHERPA_MODEL_DIR", || {
        Ok(library_root
            .join("models")
            .join("speech")
            .join("sherpa-x-asr"))
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
