//! Configurable global shortcuts (voice toggle + screen capture).
//!
//! The config is persisted as JSON in the app data root and re-applied at
//! startup. Rebind commands unregister the previous binding, register the new
//! one, and roll back to the old binding if registration fails, so a shortcut
//! that is taken by the OS or another app never silently disappears.
//!
//! While the settings dialog is recording a new combination
//! (`set_shortcut_capture_active`), the handlers swallow presses so that
//! typing the currently active shortcut during capture does not trigger the
//! action it stands for.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub const DEFAULT_RAMBLE_TOGGLE_SHORTCUT: &str = "Ctrl+Shift+R";
pub const DEFAULT_SCREEN_CAPTURE_SHORTCUT: &str = "Ctrl+1";

const CONFIG_FILE: &str = "shortcuts.json";
const RAMBLE_TOGGLE_ACTION: &str = "rambleToggle";
const SCREEN_CAPTURE_ACTION: &str = "screenCapture";
const RAMBLE_TOGGLE_EVENT: &str = "ramble-toggle-shortcut";
const SCREEN_CAPTURE_EVENT: &str = "screen-capture-shortcut";
const MAIN_LABEL: &str = "main";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutConfig {
    /// Global shortcut that starts/stops the voice Ramble.
    pub ramble_toggle: String,
    /// Global shortcut that starts a screen capture.
    pub screen_capture: String,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            ramble_toggle: DEFAULT_RAMBLE_TOGGLE_SHORTCUT.to_owned(),
            screen_capture: DEFAULT_SCREEN_CAPTURE_SHORTCUT.to_owned(),
        }
    }
}

pub struct ShortcutSettings {
    config: Mutex<ShortcutConfig>,
    capture_active: AtomicBool,
}

impl ShortcutSettings {
    /// Loads the persisted config (or the defaults), registers both shortcuts,
    /// and returns the manager for `app.manage`.
    pub fn initialize(app: &AppHandle) -> Self {
        let config = load_config();
        let settings = Self {
            config: Mutex::new(config),
            capture_active: AtomicBool::new(false),
        };
        settings.apply_all(app);
        settings
    }

    fn apply_all(&self, app: &AppHandle) {
        for action in [RAMBLE_TOGGLE_ACTION, SCREEN_CAPTURE_ACTION] {
            if let Err(error) = self.register_action(app, action) {
                tracing::warn!(%error, action, "global shortcut is unavailable");
            }
        }
    }

    fn register_action(&self, app: &AppHandle, action: &'static str) -> Result<(), String> {
        let config = self
            .config
            .lock()
            .map_err(|_| "快捷键配置锁已损坏".to_owned())?;
        let value = value_for_action(action, &config)?;
        let shortcut = validate_shortcut(value)?;
        app.global_shortcut()
            .on_shortcut(shortcut, move |app, _shortcut, event| {
                handle_pressed(app, action, event.state());
            })
            .map_err(|error| format!("无法注册快捷键「{value}」：{error}"))?;
        tracing::info!(%action, %value, "registered global shortcut");
        Ok(())
    }

    /// Replaces one binding, persisting only after the new shortcut is
    /// registered successfully. On registration failure the previous binding
    /// is restored and the config file is left unchanged.
    fn set_shortcut(
        &self,
        app: &AppHandle,
        action: &str,
        shortcut: &str,
    ) -> Result<ShortcutConfig, String> {
        let action_key = static_action(action)?;
        let validated = validate_shortcut(shortcut)?;
        let mut config = self
            .config
            .lock()
            .map_err(|_| "快捷键配置锁已损坏".to_owned())?;
        let previous = value_for_action(action_key, &config)?.to_owned();
        if let Some(conflict) =
            other_value_for_action(action_key, &config)?.filter(|value| *value == shortcut)
        {
            return Err(format!("「{shortcut}」已经被另一个动作使用：{conflict}"));
        }
        if previous == shortcut {
            return Ok(config.clone());
        }
        if let Ok(previous_shortcut) = previous.parse::<Shortcut>() {
            let _ = app.global_shortcut().unregister(previous_shortcut);
        }
        if let Err(error) =
            app.global_shortcut()
                .on_shortcut(validated, move |app, _shortcut, event| {
                    handle_pressed(app, action_key, event.state());
                })
        {
            // Roll back to the previous binding; keep reporting the real error.
            if let Ok(previous_shortcut) = previous.parse::<Shortcut>()
                && let Err(rollback) = app.global_shortcut().on_shortcut(
                    previous_shortcut,
                    move |app, _shortcut, event| {
                        handle_pressed(app, action_key, event.state());
                    },
                )
            {
                tracing::warn!(%rollback, action = action_key, "failed to restore previous global shortcut");
            }
            return Err(format!("无法注册快捷键「{shortcut}」：{error}"));
        }
        set_value_for_action(action_key, &mut config, shortcut.to_owned());
        save_config(&config);
        tracing::info!(
            action = action_key,
            value = shortcut,
            "updated global shortcut"
        );
        Ok(config.clone())
    }

    fn reset_shortcuts(&self, app: &AppHandle) -> Result<ShortcutConfig, String> {
        let mut config = self
            .config
            .lock()
            .map_err(|_| "快捷键配置锁已损坏".to_owned())?;
        *config = ShortcutConfig::default();
        self.apply_all(app);
        save_config(&config);
        Ok(config.clone())
    }
}

fn static_action(action: &str) -> Result<&'static str, String> {
    match action {
        RAMBLE_TOGGLE_ACTION => Ok(RAMBLE_TOGGLE_ACTION),
        SCREEN_CAPTURE_ACTION => Ok(SCREEN_CAPTURE_ACTION),
        other => Err(format!("未知快捷键动作：{other}")),
    }
}

fn value_for_action<'a>(action: &str, config: &'a ShortcutConfig) -> Result<&'a str, String> {
    match action {
        RAMBLE_TOGGLE_ACTION => Ok(&config.ramble_toggle),
        SCREEN_CAPTURE_ACTION => Ok(&config.screen_capture),
        other => Err(format!("未知快捷键动作：{other}")),
    }
}

fn other_value_for_action<'a>(
    action: &str,
    config: &'a ShortcutConfig,
) -> Result<Option<&'a str>, String> {
    Ok(Some(match action {
        RAMBLE_TOGGLE_ACTION => &config.screen_capture,
        SCREEN_CAPTURE_ACTION => &config.ramble_toggle,
        other => return Err(format!("未知快捷键动作：{other}")),
    }))
}

fn set_value_for_action(action: &str, config: &mut ShortcutConfig, value: String) {
    match action {
        RAMBLE_TOGGLE_ACTION => config.ramble_toggle = value,
        SCREEN_CAPTURE_ACTION => config.screen_capture = value,
        _ => {}
    }
}

/// Parses and sanity-checks a shortcut string.
///
/// Global shortcuts need at least one modifier unless they are a function key
/// (F1–F24) on their own; Escape is reserved for cancelling the recorder, and
/// the OS-level combos are rejected by the registration step.
fn validate_shortcut(value: &str) -> Result<Shortcut, String> {
    let shortcut: Shortcut = value
        .parse()
        .map_err(|error| format!("无法识别快捷键「{value}」：{error}"))?;
    if shortcut.key == Code::Escape {
        return Err("Escape 用于取消录入，不能作为快捷键".to_owned());
    }
    if shortcut.mods == Modifiers::empty() && !is_function_key(shortcut.key) {
        return Err(
            "快捷键需要至少一个修饰键（Ctrl / Cmd / Alt / Shift），或使用 F1–F24 功能键".to_owned(),
        );
    }
    Ok(shortcut)
}

fn is_function_key(key: Code) -> bool {
    matches!(
        key,
        Code::F1
            | Code::F2
            | Code::F3
            | Code::F4
            | Code::F5
            | Code::F6
            | Code::F7
            | Code::F8
            | Code::F9
            | Code::F10
            | Code::F11
            | Code::F12
            | Code::F13
            | Code::F14
            | Code::F15
            | Code::F16
            | Code::F17
            | Code::F18
            | Code::F19
            | Code::F20
            | Code::F21
            | Code::F22
            | Code::F23
            | Code::F24
    )
}

fn handle_pressed(app: &AppHandle, action: &'static str, state: ShortcutState) {
    if state != ShortcutState::Pressed {
        return;
    }
    if app
        .try_state::<ShortcutSettings>()
        .is_some_and(|settings| settings.capture_active.load(Ordering::SeqCst))
    {
        return;
    }
    let event = match action {
        RAMBLE_TOGGLE_ACTION => RAMBLE_TOGGLE_EVENT,
        _ => SCREEN_CAPTURE_EVENT,
    };
    let payload = app
        .try_state::<ShortcutSettings>()
        .and_then(|settings| {
            settings
                .config
                .lock()
                .ok()
                .and_then(|config| value_for_action(action, &config).ok().map(str::to_owned))
        })
        .unwrap_or_default();
    if let Err(error) = app.emit_to(MAIN_LABEL, event, payload) {
        tracing::warn!(%error, action, "failed to emit global shortcut event");
    }
}

fn config_path() -> Option<PathBuf> {
    rambledesk_storage::default_app_data_root()
        .ok()
        .map(|root| root.join(CONFIG_FILE))
}

/// The stored config is trusted only when both bindings parse and differ.
fn load_config() -> ShortcutConfig {
    let loaded = config_path()
        .as_deref()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str::<ShortcutConfig>(&raw).ok())
        .filter(|config| {
            validate_shortcut(&config.ramble_toggle).is_ok()
                && validate_shortcut(&config.screen_capture).is_ok()
                && config.ramble_toggle != config.screen_capture
        });
    loaded.unwrap_or_default()
}

fn save_config(config: &ShortcutConfig) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let Ok(json) = serde_json::to_string_pretty(config) else {
        return;
    };
    if let Err(error) = fs::write(&path, json) {
        tracing::warn!(%error, path = %path.display(), "failed to save shortcut settings");
    }
}

#[tauri::command]
pub fn get_shortcut_settings(
    state: tauri::State<'_, ShortcutSettings>,
) -> Result<ShortcutConfig, String> {
    state
        .config
        .lock()
        .map(|config| config.clone())
        .map_err(|_| "快捷键配置锁已损坏".to_owned())
}

#[tauri::command]
pub fn set_shortcut_setting(
    action: String,
    shortcut: String,
    app: AppHandle,
    state: tauri::State<'_, ShortcutSettings>,
) -> Result<ShortcutConfig, String> {
    state.set_shortcut(&app, &action, &shortcut)
}

#[tauri::command]
pub fn reset_shortcut_settings(
    app: AppHandle,
    state: tauri::State<'_, ShortcutSettings>,
) -> Result<ShortcutConfig, String> {
    state.reset_shortcuts(&app)
}

/// While true, the registered shortcuts are swallowed so key presses during
/// recording do not trigger the actions they still represent.
#[tauri::command]
pub fn set_shortcut_capture_active(
    active: bool,
    state: tauri::State<'_, ShortcutSettings>,
) -> Result<(), String> {
    state.capture_active.store(active, Ordering::SeqCst);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_distinct() {
        let config = ShortcutConfig::default();
        assert!(validate_shortcut(&config.ramble_toggle).is_ok());
        assert!(validate_shortcut(&config.screen_capture).is_ok());
        assert_ne!(config.ramble_toggle, config.screen_capture);
    }

    #[test]
    fn bare_letter_is_rejected_without_a_modifier() {
        assert!(validate_shortcut("R").is_err());
        assert!(validate_shortcut("1").is_err());
    }

    #[test]
    fn function_key_alone_is_valid() {
        assert!(validate_shortcut("F7").is_ok());
        assert!(validate_shortcut("Ctrl+F7").is_ok());
    }

    #[test]
    fn modifier_combinations_are_accepted() {
        assert!(validate_shortcut("Ctrl+Shift+R").is_ok());
        assert!(validate_shortcut("Cmd+Alt+1").is_ok());
        assert!(validate_shortcut("Ctrl+Shift+ArrowUp").is_ok());
    }

    #[test]
    fn escape_is_reserved_for_the_recorder() {
        assert!(validate_shortcut("Escape").is_err());
        assert!(validate_shortcut("Ctrl+Escape").is_err());
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(validate_shortcut("Ctrl+FakeKey").is_err());
        assert!(validate_shortcut("Ctrl+").is_err());
    }

    #[test]
    fn loaded_config_must_parse_and_stay_distinct() {
        assert!(load_config().ramble_toggle != load_config().screen_capture);
    }
}
