//! One native preference and writer gate for every persisted diagnostic source.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock, RwLockReadGuard};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticsSettings {
    pub enabled: bool,
}

struct ControlState {
    settings: DiagnosticsSettings,
    load_error: Option<String>,
}

pub(crate) struct DiagnosticsControl {
    root: PathBuf,
    state: RwLock<ControlState>,
}

pub(crate) struct RecordingGuard<'a> {
    _state: RwLockReadGuard<'a, ControlState>,
}

impl DiagnosticsControl {
    pub(crate) fn load(root: PathBuf) -> Self {
        let loaded = match fs::read(root.join("diagnostics-settings.json")) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|_| "无法读取诊断记录设置".to_owned())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(DiagnosticsSettings { enabled: true })
            }
            Err(_) => Err("无法读取诊断记录设置".to_owned()),
        };
        let (settings, load_error) = match loaded {
            Ok(settings) => (settings, None),
            // A damaged preference must not silently opt the user back into recording.
            Err(error) => (DiagnosticsSettings { enabled: false }, Some(error)),
        };
        Self {
            root,
            state: RwLock::new(ControlState {
                settings,
                load_error,
            }),
        }
    }

    fn settings_path(&self) -> PathBuf {
        self.root.join("diagnostics-settings.json")
    }

    pub(crate) fn settings(&self) -> Result<DiagnosticsSettings, String> {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match &state.load_error {
            Some(error) => Err(error.clone()),
            None => Ok(state.settings),
        }
    }

    pub(crate) fn recording_guard(&self) -> Option<RecordingGuard<'_>> {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state
            .settings
            .enabled
            .then_some(RecordingGuard { _state: state })
    }

    pub(crate) fn set_enabled(&self, enabled: bool) -> Result<DiagnosticsSettings, String> {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let settings = DiagnosticsSettings { enabled };
        let temporary = self
            .root
            .join(format!("diagnostics-settings.{}.tmp", uuid::Uuid::now_v7()));
        let persist = || -> Result<(), Box<dyn std::error::Error>> {
            fs::create_dir_all(&self.root)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&serde_json::to_vec(&settings)?)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, self.settings_path())?;
            Ok(())
        };
        if persist().is_err() {
            let _ = fs::remove_file(&temporary);
            return Err("无法保存诊断记录设置".to_owned());
        }
        state.settings = settings;
        state.load_error = None;
        Ok(settings)
    }

    pub(crate) fn clear(&self) -> Result<(), String> {
        // Writers hold a read guard for the complete append. The active rolling
        // file remains open on Windows, so truncate it instead of removing it.
        let _state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let events_directory = self.root.join("diagnostics");
        if regular_directory(&events_directory)? {
            truncate_diagnostic_file(&events_directory.join("events.jsonl"))?;
            truncate_diagnostic_file(&events_directory.join("events.jsonl.tmp"))?;
        }
        let log_directory = self.root.join("logs");
        if regular_directory(&log_directory)? {
            for entry in fs::read_dir(&log_directory).map_err(|_| "无法读取诊断日志目录")?
            {
                let entry = entry.map_err(|_| "无法读取诊断日志文件")?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if is_recorded_log(&name) {
                    truncate_diagnostic_file(&entry.path())?;
                }
            }
        }
        Ok(())
    }
}

fn is_recorded_log(name: &str) -> bool {
    if matches!(name, "rambledesk.log" | "rambledesk.log.init-error") {
        return true;
    }
    let Some(date) = name.strip_prefix("rambledesk.log.") else {
        return false;
    };
    let parts = date.split('-').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        parts[0].parse::<i32>(),
        parts[1].parse::<u8>(),
        parts[2].parse::<u8>(),
    ) else {
        return false;
    };
    time::Month::try_from(month)
        .is_ok_and(|month| time::Date::from_calendar_date(year, month, day).is_ok())
}

fn regular_directory(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(true),
        Ok(_) => Err("诊断目录不是普通目录，未清除其内容".to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err("无法读取诊断目录".to_owned()),
    }
}

fn truncate_diagnostic_file(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => fs::OpenOptions::new()
            .write(true)
            .open(path)
            .and_then(|file| file.set_len(0))
            .map_err(|_| "无法清除诊断文件".to_owned()),
        Ok(_) => Err("诊断文件不是普通文件，未清除其内容".to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("无法读取诊断文件".to_owned()),
    }
}

pub(crate) fn global() -> Result<&'static Arc<DiagnosticsControl>, String> {
    static CONTROL: OnceLock<Result<Arc<DiagnosticsControl>, String>> = OnceLock::new();
    CONTROL
        .get_or_init(|| {
            rambledesk_storage::default_app_data_root()
                .map(|root| Arc::new(DiagnosticsControl::load(root)))
                .map_err(|_| "无法定位诊断设置目录".to_owned())
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub(crate) fn recording_guard() -> Option<RecordingGuard<'static>> {
    global().ok()?.recording_guard()
}

#[tauri::command]
pub fn get_diagnostics_settings() -> Result<DiagnosticsSettings, String> {
    global()?.settings()
}

#[tauri::command]
pub fn set_diagnostics_enabled(enabled: bool) -> Result<DiagnosticsSettings, String> {
    global()?.set_enabled(enabled)
}

#[tauri::command]
pub fn clear_diagnostics() -> Result<(), String> {
    global()?.clear()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn recording_defaults_on_and_persists_both_choices() {
        let directory = tempfile::tempdir().unwrap();
        let control = DiagnosticsControl::load(directory.path().to_owned());
        assert!(control.settings().unwrap().enabled);
        assert!(control.recording_guard().is_some());
        control.set_enabled(false).unwrap();
        let reopened = DiagnosticsControl::load(directory.path().to_owned());
        assert!(!reopened.settings().unwrap().enabled);
        assert!(reopened.recording_guard().is_none());
        reopened.set_enabled(true).unwrap();
        assert!(
            DiagnosticsControl::load(directory.path().to_owned())
                .settings()
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn failed_setting_write_preserves_the_live_preference() {
        let directory = tempfile::tempdir().unwrap();
        let control = DiagnosticsControl::load(directory.path().to_owned());
        std::fs::create_dir(control.settings_path()).unwrap();
        assert!(control.set_enabled(false).is_err());
        assert!(control.settings().unwrap().enabled);
    }

    #[test]
    fn unreadable_settings_do_not_resume_recording_and_can_be_repaired() {
        let directory = tempfile::tempdir().unwrap();
        let control = DiagnosticsControl::load(directory.path().to_owned());
        std::fs::write(control.settings_path(), "invalid json").unwrap();
        let reopened = DiagnosticsControl::load(directory.path().to_owned());
        assert!(reopened.settings().is_err());
        assert!(reopened.recording_guard().is_none());
        reopened.set_enabled(false).unwrap();
        assert!(!reopened.settings().unwrap().enabled);
    }

    #[test]
    fn clear_preserves_business_data_settings_and_exported_packages() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let control = DiagnosticsControl::load(root.to_owned());
        control.set_enabled(false).unwrap();
        for folder in ["diagnostics", "logs", "attachments"] {
            std::fs::create_dir(root.join(folder)).unwrap();
        }
        let diagnostics = [
            "diagnostics/events.jsonl",
            "logs/rambledesk.log.2026-09-07",
            "logs/rambledesk.log.init-error",
            "diagnostics/events.jsonl.tmp",
        ];
        let preserved = [
            "feedback.db",
            "settings.json",
            "attachments/image.png",
            "diagnostics/export.zip",
            "logs/other.log",
            "logs/rambledesk.logger-not-a-log",
            "logs/rambledesk.log.export.zip",
            "logs/rambledesk.log.notes",
        ];
        for relative in diagnostics.iter().chain(preserved.iter()) {
            std::fs::write(root.join(relative), "keep this text").unwrap();
        }
        // Retaining the active file handle matches the rolling logger on Windows.
        let mut active = std::fs::OpenOptions::new()
            .append(true)
            .open(root.join(diagnostics[1]))
            .unwrap();
        control.clear().unwrap();
        for relative in diagnostics {
            assert_eq!(std::fs::read(root.join(relative)).unwrap(), b"");
        }
        for relative in preserved {
            assert_eq!(
                std::fs::read(root.join(relative)).unwrap(),
                b"keep this text"
            );
        }
        assert!(
            !DiagnosticsControl::load(root.to_owned())
                .settings()
                .unwrap()
                .enabled
        );
        active.write_all(b"new entry\n").unwrap();
        assert_eq!(
            std::fs::read(root.join(diagnostics[1])).unwrap(),
            b"new entry\n"
        );
    }

    #[test]
    fn clear_is_a_noop_without_diagnostic_files() {
        let directory = tempfile::tempdir().unwrap();
        let control = DiagnosticsControl::load(directory.path().to_owned());
        control.clear().unwrap();
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}
