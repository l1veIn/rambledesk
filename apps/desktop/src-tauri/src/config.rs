use std::{
    fs,
    io::{Read, Write},
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

pub(super) fn migrate_library(
    source: &Path,
    destination: &Path,
    progress: &dyn Fn(u64, u64),
) -> Result<u64, String> {
    if source == destination {
        return Ok(0);
    }
    if destination.starts_with(source) || source.starts_with(destination) {
        return Err("新旧数据存储位置不能互相包含".to_owned());
    }
    let total = directory_bytes(source)?;
    fs::create_dir_all(destination)
        .map_err(|error| format!("无法创建迁移目标 {}：{error}", destination.display()))?;
    let mut copied = 0;
    copy_directory(source, source, destination, total, &mut copied, progress)?;
    progress(total, total);
    Ok(total)
}

fn directory_bytes(path: &Path) -> Result<u64, String> {
    if !path.exists() {
        return Ok(0);
    }
    let mut total = 0;
    for entry in
        fs::read_dir(path).map_err(|error| format!("无法扫描 {}：{error}", path.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            total += directory_bytes(&entry.path())?;
        } else {
            total += metadata.len();
        }
    }
    Ok(total)
}

fn copy_directory(
    source_root: &Path,
    current: &Path,
    destination: &Path,
    total: u64,
    copied: &mut u64,
    progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    if !current.exists() {
        return Ok(());
    }
    for entry in
        fs::read_dir(current).map_err(|error| format!("无法读取 {}：{error}", current.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let relative = entry
            .path()
            .strip_prefix(source_root)
            .map_err(|error| error.to_string())?
            .to_path_buf();
        let target = destination.join(relative);
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|error| format!("无法创建 {}：{error}", target.display()))?;
            copy_directory(
                source_root,
                &entry.path(),
                destination,
                total,
                copied,
                progress,
            )?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let temporary = target.with_extension("rambledesk-migrate-part");
            let mut input = fs::File::open(entry.path()).map_err(|error| error.to_string())?;
            let mut output = fs::File::create(&temporary).map_err(|error| error.to_string())?;
            let mut buffer = vec![0u8; 256 * 1024];
            loop {
                let count = input.read(&mut buffer).map_err(|error| error.to_string())?;
                if count == 0 {
                    break;
                }
                output
                    .write_all(&buffer[..count])
                    .map_err(|error| error.to_string())?;
                *copied += count as u64;
                progress(*copied, total);
            }
            output.flush().map_err(|error| error.to_string())?;
            if target.exists() {
                fs::remove_file(&target).map_err(|error| error.to_string())?;
            }
            fs::rename(&temporary, &target)
                .map_err(|error| format!("迁移文件 {} 失败：{error}", target.display()))?;
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_migration_copies_nested_files_and_reports_progress() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        fs::create_dir_all(source.join("models/speech")).unwrap();
        fs::write(source.join("draft.txt"), b"draft").unwrap();
        fs::write(source.join("models/speech/model.bin"), b"model").unwrap();
        let events = std::sync::Mutex::new(Vec::new());

        let total = migrate_library(&source, &destination, &|copied, total| {
            events.lock().unwrap().push((copied, total));
        })
        .unwrap();

        assert_eq!(total, 10);
        assert_eq!(fs::read(destination.join("draft.txt")).unwrap(), b"draft");
        assert_eq!(
            fs::read(destination.join("models/speech/model.bin")).unwrap(),
            b"model"
        );
        assert_eq!(events.lock().unwrap().last(), Some(&(10, 10)));
        assert!(
            source.join("draft.txt").exists(),
            "source remains as a restart-safe backup"
        );
    }

    #[test]
    fn library_migration_rejects_nested_destinations() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("library");
        fs::create_dir_all(&source).unwrap();
        assert!(migrate_library(&source, &source.join("nested"), &|_, _| {}).is_err());
    }
}
