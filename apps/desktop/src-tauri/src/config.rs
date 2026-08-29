use std::{
    ffi::OsString,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use rambledesk_local_server::{AccessToken, DEFAULT_PORT, default_token_path};
use serde::{Deserialize, Serialize};

const NESTED_LIBRARY_PATH_ERROR: &str = "新旧数据存储位置不能互相包含";
const DESTINATION_RECURSION_ERROR: &str = "迁移目标进入了迁移源扫描范围";

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
    let comparable_source = comparable_library_path(source)?;
    let comparable_destination = comparable_library_path(destination)?;
    if comparable_source == comparable_destination {
        return Ok(0);
    }
    if paths_overlap(&comparable_source, &comparable_destination) {
        return Err(NESTED_LIBRARY_PATH_ERROR.to_owned());
    }
    let total = directory_bytes(source, &comparable_destination)?;
    fs::create_dir_all(destination)
        .map_err(|error| format!("无法创建迁移目标 {}：{error}", destination.display()))?;
    let mut copied = 0;
    copy_directory(
        source,
        source,
        destination,
        &comparable_destination,
        total,
        &mut copied,
        progress,
    )?;
    progress(total, total);
    Ok(total)
}

fn comparable_library_path(path: &Path) -> Result<PathBuf, String> {
    // Canonicalize the deepest existing ancestor so a destination that does
    // not exist yet can still be compared with an already-canonicalized source.
    let mut candidate = path;
    let mut missing_components = Vec::<OsString>::new();
    let mut comparable = loop {
        match dunce::canonicalize(candidate) {
            Ok(path) => break path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let component = candidate
                    .file_name()
                    .ok_or_else(|| format!("无法解析迁移路径 {}：{error}", path.display()))?;
                missing_components.push(component.to_os_string());
                candidate = candidate
                    .parent()
                    .ok_or_else(|| format!("无法解析迁移路径 {}：{error}", path.display()))?;
            }
            Err(error) => {
                return Err(format!("无法解析迁移路径 {}：{error}", path.display()));
            }
        }
    };
    for component in missing_components.iter().rev() {
        comparable.push(component);
    }
    Ok(comparable)
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn reject_destination_recursion(path: &Path, destination: &Path) -> Result<(), String> {
    let comparable = comparable_library_path(path)?;
    let comparable_destination = comparable_library_path(destination)?;
    if paths_overlap(&comparable, &comparable_destination) {
        Err(DESTINATION_RECURSION_ERROR.to_owned())
    } else {
        Ok(())
    }
}

fn directory_bytes(path: &Path, destination: &Path) -> Result<u64, String> {
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
            reject_destination_recursion(&entry.path(), destination)?;
            total += directory_bytes(&entry.path(), destination)?;
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
    comparable_destination: &Path,
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
            reject_destination_recursion(&entry.path(), comparable_destination)?;
            fs::create_dir_all(&target)
                .map_err(|error| format!("无法创建 {}：{error}", target.display()))?;
            copy_directory(
                source_root,
                &entry.path(),
                destination,
                comparable_destination,
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
    let canonical = dunce::canonicalize(path)
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
    fn empty_library_migration_reports_zero_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        fs::create_dir_all(&source).unwrap();
        let events = std::sync::Mutex::new(Vec::new());

        let total = migrate_library(&source, &destination, &|copied, total| {
            events.lock().unwrap().push((copied, total));
        })
        .unwrap();

        assert_eq!(total, 0);
        assert!(destination.is_dir());
        assert_eq!(events.lock().unwrap().as_slice(), &[(0, 0)]);
    }

    #[test]
    fn library_migration_rejects_nested_destinations() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("library");
        fs::create_dir_all(&source).unwrap();
        assert_eq!(
            migrate_library(&source, &source.join("nested"), &|_, _| {}).unwrap_err(),
            NESTED_LIBRARY_PATH_ERROR
        );
    }

    #[cfg(windows)]
    #[test]
    fn library_migration_rejects_nested_destination_with_mixed_windows_path_forms() {
        let temp = tempfile::tempdir().unwrap();
        let source_normal = temp.path().join("library");
        let destination_normal = source_normal.join("RambleDesk");
        fs::create_dir_all(&destination_normal).unwrap();
        let source_canonical = fs::canonicalize(&source_normal).unwrap();

        assert_ne!(source_canonical, source_normal);
        assert_eq!(
            migrate_library(&source_canonical, &destination_normal, &|_, _| {}).unwrap_err(),
            NESTED_LIBRARY_PATH_ERROR
        );
        assert_eq!(fs::read_dir(destination_normal).unwrap().count(), 0);
    }

    #[cfg(windows)]
    #[test]
    fn library_migration_treats_mixed_windows_forms_as_the_same_directory() {
        let temp = tempfile::tempdir().unwrap();
        let normal = temp.path().join("library");
        fs::create_dir_all(&normal).unwrap();
        let canonical = fs::canonicalize(&normal).unwrap();

        assert_ne!(canonical, normal);
        assert_eq!(migrate_library(&canonical, &normal, &|_, _| {}).unwrap(), 0);
    }

    #[cfg(windows)]
    #[test]
    fn library_migration_rejects_parent_destination_with_mixed_windows_path_forms() {
        let temp = tempfile::tempdir().unwrap();
        let destination_normal = temp.path().join("library");
        let source_normal = destination_normal.join("nested");
        fs::create_dir_all(&source_normal).unwrap();
        let source_canonical = fs::canonicalize(&source_normal).unwrap();

        assert_eq!(
            migrate_library(&source_canonical, &destination_normal, &|_, _| {}).unwrap_err(),
            NESTED_LIBRARY_PATH_ERROR
        );
    }

    #[cfg(windows)]
    #[test]
    fn recursive_guard_rejects_mixed_windows_destination_forms() {
        let temp = tempfile::tempdir().unwrap();
        let destination_normal = temp.path().join("library");
        fs::create_dir_all(&destination_normal).unwrap();
        let destination_canonical = fs::canonicalize(&destination_normal).unwrap();

        assert_eq!(
            reject_destination_recursion(&destination_canonical, &destination_normal).unwrap_err(),
            DESTINATION_RECURSION_ERROR
        );
    }
}
