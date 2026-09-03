//! Configurable global shortcuts, including context-dependent speech review.
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
pub const DEFAULT_SPEECH_ACCEPT_SHORTCUT: &str = "Ctrl+Shift+Enter";
pub const DEFAULT_SPEECH_DISCARD_SHORTCUT: &str = "Ctrl+Shift+Backspace";

const CONFIG_FILE: &str = "shortcuts.json";
const RAMBLE_TOGGLE_ACTION: &str = "rambleToggle";
const SCREEN_CAPTURE_ACTION: &str = "screenCapture";
const SPEECH_ACCEPT_ACTION: &str = "speechAccept";
const SPEECH_DISCARD_ACTION: &str = "speechDiscard";
const ACTIONS: [&str; 4] = [
    RAMBLE_TOGGLE_ACTION,
    SCREEN_CAPTURE_ACTION,
    SPEECH_ACCEPT_ACTION,
    SPEECH_DISCARD_ACTION,
];
const REVIEW_ACTIONS: [&str; 2] = [SPEECH_ACCEPT_ACTION, SPEECH_DISCARD_ACTION];
const SPEECH_REVIEW_EVENT: &str = "speech-review-shortcut";
const RAMBLE_TOGGLE_EVENT: &str = "ramble-toggle-shortcut";
const SCREEN_CAPTURE_EVENT: &str = "screen-capture-shortcut";
const MAIN_LABEL: &str = "main";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct ShortcutConfig {
    /// Global shortcut that starts/stops the voice Ramble.
    pub ramble_toggle: String,
    /// Global shortcut that starts a screen capture.
    pub screen_capture: String,
    pub speech_accept: String,
    pub speech_discard: String,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            ramble_toggle: DEFAULT_RAMBLE_TOGGLE_SHORTCUT.to_owned(),
            screen_capture: DEFAULT_SCREEN_CAPTURE_SHORTCUT.to_owned(),
            speech_accept: DEFAULT_SPEECH_ACCEPT_SHORTCUT.to_owned(),
            speech_discard: DEFAULT_SPEECH_DISCARD_SHORTCUT.to_owned(),
        }
    }
}

pub struct ShortcutSettings {
    config: Mutex<ShortcutConfig>,
    capture_active: AtomicBool,
    ramble_pressed: AtomicBool,
    screen_capture_pressed: AtomicBool,
    speech_accept_pressed: AtomicBool,
    speech_discard_pressed: AtomicBool,
    speech_review_active: AtomicBool,
    speech_review_registered: AtomicBool,
}

impl ShortcutSettings {
    /// Loads the persisted config (or the defaults), registers both shortcuts,
    /// and returns the manager for `app.manage`.
    pub fn initialize(app: &AppHandle) -> Self {
        let config = load_config();
        let settings = Self {
            config: Mutex::new(config),
            capture_active: AtomicBool::new(false),
            ramble_pressed: AtomicBool::new(false),
            screen_capture_pressed: AtomicBool::new(false),
            speech_accept_pressed: AtomicBool::new(false),
            speech_discard_pressed: AtomicBool::new(false),
            speech_review_active: AtomicBool::new(false),
            speech_review_registered: AtomicBool::new(false),
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
        let value = {
            let config = self
                .config
                .lock()
                .map_err(|_| "快捷键配置锁已损坏".to_owned())?;
            value_for_action(action, &config)?.to_owned()
        };
        register_value(app, action, &value)?;
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
        validate_shortcut(shortcut)?;
        let mut config = self
            .config
            .lock()
            .map_err(|_| "快捷键配置锁已损坏".to_owned())?;
        let previous = value_for_action(action_key, &config)?.to_owned();
        if let Some(conflict) = conflicting_action(action_key, shortcut, &config) {
            return Err(format!("「{shortcut}」已经被另一个动作使用：{conflict}"));
        }
        if previous == shortcut {
            return Ok(config.clone());
        }
        let active = action_active(
            action_key,
            self.speech_review_registered.load(Ordering::SeqCst),
        );
        if active {
            unregister_value(app, &previous);
        }
        if let Err(error) = register_value(app, action_key, shortcut) {
            // Roll back to the previous binding; keep reporting the real error.
            if active && let Err(rollback) = register_value(app, action_key, &previous) {
                tracing::warn!(%rollback, action = action_key, "failed to restore previous global shortcut");
            }
            return Err(error);
        }
        // Validate inactive bindings against the OS without occupying them.
        if !active {
            unregister_value(app, shortcut);
        }
        let mut next = config.clone();
        set_value_for_action(action_key, &mut next, shortcut.to_owned());
        if let Err(error) = save_config(&next) {
            unregister_value(app, shortcut);
            if active && let Err(rollback) = register_value(app, action_key, &previous) {
                tracing::warn!(%rollback, action = action_key, "failed to restore previous global shortcut after persistence failure");
            }
            return Err(error);
        }
        *config = next;
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
        let previous = config.clone();
        let next = ShortcutConfig::default();
        if previous == next {
            save_config(&next)?;
            return Ok(next);
        }

        let review_active = self.speech_review_registered.load(Ordering::SeqCst);
        unregister_config(app, &previous, review_active);
        if let Err(error) = register_config(app, &next, review_active) {
            unregister_config(app, &next, review_active);
            restore_config(app, &previous, review_active);
            return Err(error);
        }
        if let Err(error) = save_config(&next) {
            unregister_config(app, &next, review_active);
            restore_config(app, &previous, review_active);
            return Err(error);
        }
        *config = next;
        Ok(config.clone())
    }

    fn set_speech_review_active(&self, app: &AppHandle, active: bool) -> Result<(), String> {
        let config = self
            .config
            .lock()
            .map_err(|_| "快捷键配置锁已损坏".to_owned())?;
        if active && !self.speech_review_registered.load(Ordering::SeqCst) {
            register_value(app, SPEECH_ACCEPT_ACTION, &config.speech_accept)?;
            if let Err(error) = register_value(app, SPEECH_DISCARD_ACTION, &config.speech_discard) {
                unregister_value(app, &config.speech_accept);
                return Err(error);
            }
            self.speech_review_registered.store(true, Ordering::SeqCst);
        } else if !active
            && !self.speech_accept_pressed.load(Ordering::SeqCst)
            && !self.speech_discard_pressed.load(Ordering::SeqCst)
        {
            unregister_value(app, &config.speech_accept);
            unregister_value(app, &config.speech_discard);
            self.speech_review_registered.store(false, Ordering::SeqCst);
        }
        self.speech_review_active.store(active, Ordering::SeqCst);
        // If a review key is held, defer unregistering until Released. Otherwise the
        // release is lost and reactivation either sticks or accepts a second group.
        Ok(())
    }

    fn release_idle_review_shortcuts(&self, app: &AppHandle) {
        let Ok(config) = self.config.lock() else {
            return;
        };
        if !self.speech_review_active.load(Ordering::SeqCst)
            && !self.speech_accept_pressed.load(Ordering::SeqCst)
            && !self.speech_discard_pressed.load(Ordering::SeqCst)
            && self.speech_review_registered.load(Ordering::SeqCst)
        {
            unregister_value(app, &config.speech_accept);
            unregister_value(app, &config.speech_discard);
            self.speech_review_registered.store(false, Ordering::SeqCst);
        }
    }
}

fn register_value(app: &AppHandle, action: &'static str, value: &str) -> Result<(), String> {
    let shortcut = validate_shortcut(value)?;
    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            handle_pressed(app, action, event.state());
        })
        .map_err(|error| format!("无法注册快捷键「{value}」：{error}"))
}

fn unregister_value(app: &AppHandle, value: &str) {
    if let Ok(shortcut) = value.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(shortcut);
    }
}

fn action_active(action: &str, review_active: bool) -> bool {
    !REVIEW_ACTIONS.contains(&action) || review_active
}

fn unregister_config(app: &AppHandle, config: &ShortcutConfig, review_active: bool) {
    for action in ACTIONS {
        if action_active(action, review_active) {
            unregister_value(app, value_for_action(action, config).unwrap());
        }
    }
}

fn register_config(
    app: &AppHandle,
    config: &ShortcutConfig,
    review_active: bool,
) -> Result<(), String> {
    for action in ACTIONS {
        if action_active(action, review_active) {
            register_value(app, action, value_for_action(action, config)?)?;
        }
    }
    Ok(())
}

fn restore_config(app: &AppHandle, config: &ShortcutConfig, review_active: bool) {
    if let Err(error) = register_config(app, config, review_active) {
        tracing::warn!(%error, "failed to restore previous global shortcut configuration");
    }
}

fn static_action(action: &str) -> Result<&'static str, String> {
    match action {
        RAMBLE_TOGGLE_ACTION => Ok(RAMBLE_TOGGLE_ACTION),
        SCREEN_CAPTURE_ACTION => Ok(SCREEN_CAPTURE_ACTION),
        SPEECH_ACCEPT_ACTION => Ok(SPEECH_ACCEPT_ACTION),
        SPEECH_DISCARD_ACTION => Ok(SPEECH_DISCARD_ACTION),
        other => Err(format!("未知快捷键动作：{other}")),
    }
}

fn value_for_action<'a>(action: &str, config: &'a ShortcutConfig) -> Result<&'a str, String> {
    match action {
        RAMBLE_TOGGLE_ACTION => Ok(&config.ramble_toggle),
        SCREEN_CAPTURE_ACTION => Ok(&config.screen_capture),
        SPEECH_ACCEPT_ACTION => Ok(&config.speech_accept),
        SPEECH_DISCARD_ACTION => Ok(&config.speech_discard),
        other => Err(format!("未知快捷键动作：{other}")),
    }
}

fn conflicting_action(action: &str, value: &str, config: &ShortcutConfig) -> Option<&'static str> {
    let shortcut = validate_shortcut(value).ok()?;
    ACTIONS.into_iter().find(|other| {
        *other != action
            && value_for_action(other, config)
                .ok()
                .and_then(|value| validate_shortcut(value).ok())
                == Some(shortcut)
    })
}

fn set_value_for_action(action: &str, config: &mut ShortcutConfig, value: String) {
    match action {
        RAMBLE_TOGGLE_ACTION => config.ramble_toggle = value,
        SCREEN_CAPTURE_ACTION => config.screen_capture = value,
        SPEECH_ACCEPT_ACTION => config.speech_accept = value,
        SPEECH_DISCARD_ACTION => config.speech_discard = value,
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
    let Some(settings) = app.try_state::<ShortcutSettings>() else {
        return;
    };
    let pressed = match action {
        RAMBLE_TOGGLE_ACTION => &settings.ramble_pressed,
        SCREEN_CAPTURE_ACTION => &settings.screen_capture_pressed,
        SPEECH_ACCEPT_ACTION => &settings.speech_accept_pressed,
        SPEECH_DISCARD_ACTION => &settings.speech_discard_pressed,
        _ => return,
    };
    let emit = should_emit_shortcut(
        state,
        pressed,
        settings.capture_active.load(Ordering::SeqCst)
            || !action_active(action, settings.speech_review_active.load(Ordering::SeqCst)),
    );
    if state == ShortcutState::Released
        && REVIEW_ACTIONS.contains(&action)
        && !settings.speech_review_active.load(Ordering::SeqCst)
    {
        // The plugin holds its shortcut registry lock while invoking this handler.
        // Schedule cleanup after it returns; unregistering here deadlocks the UI.
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(settings) = handle.try_state::<ShortcutSettings>() {
                    settings.release_idle_review_shortcuts(&handle);
                }
            });
        });
    }
    if !emit {
        return;
    }
    if REVIEW_ACTIONS.contains(&action) {
        let payload = if action == SPEECH_ACCEPT_ACTION {
            "accept"
        } else {
            "discard"
        };
        if let Err(error) = app.emit_to(MAIN_LABEL, SPEECH_REVIEW_EVENT, payload) {
            tracing::warn!(%error, action, "failed to emit speech review shortcut");
        }
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

fn should_emit_shortcut(state: ShortcutState, pressed: &AtomicBool, capture_active: bool) -> bool {
    if state == ShortcutState::Released {
        pressed.store(false, Ordering::SeqCst);
        return false;
    }
    !pressed.swap(true, Ordering::SeqCst) && !capture_active
}

fn config_path() -> Option<PathBuf> {
    rambledesk_storage::default_app_data_root()
        .ok()
        .map(|root| root.join(CONFIG_FILE))
}

/// Retain legacy bindings and fill in the new speech actions without collisions.
fn parse_config(raw: &str) -> Option<ShortcutConfig> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let mut config: ShortcutConfig = serde_json::from_value(value.clone()).ok()?;
    for (action, key) in [
        (SPEECH_ACCEPT_ACTION, "Enter"),
        (SPEECH_DISCARD_ACTION, "Backspace"),
    ] {
        if value.get(action).is_none()
            && conflicting_action(action, value_for_action(action, &config).ok()?, &config)
                .is_some()
        {
            let replacement = ["Ctrl+Shift", "Ctrl+Alt", "Ctrl+Alt+Shift", "Alt+Shift"]
                .into_iter()
                .map(|modifiers| format!("{modifiers}+{key}"))
                .find(|candidate| conflicting_action(action, candidate, &config).is_none())?;
            set_value_for_action(action, &mut config, replacement);
        }
    }
    ACTIONS
        .into_iter()
        .all(|action| {
            let value = value_for_action(action, &config).unwrap();
            validate_shortcut(value).is_ok() && conflicting_action(action, value, &config).is_none()
        })
        .then_some(config)
}

fn load_config() -> ShortcutConfig {
    let loaded = config_path()
        .as_deref()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|raw| parse_config(&raw));
    loaded.unwrap_or_default()
}

fn save_config(config: &ShortcutConfig) -> Result<(), String> {
    let path = config_path().ok_or_else(|| "无法定位快捷键配置目录".to_owned())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建快捷键配置目录：{error}"))?;
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|error| format!("无法编码快捷键配置：{error}"))?;
    fs::write(&path, json).map_err(|error| format!("无法保存快捷键配置：{error}"))
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
    if !active {
        state.ramble_pressed.store(false, Ordering::SeqCst);
        state.screen_capture_pressed.store(false, Ordering::SeqCst);
        state.speech_accept_pressed.store(false, Ordering::SeqCst);
        state.speech_discard_pressed.store(false, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
pub fn set_speech_review_shortcuts_active(
    active: bool,
    app: AppHandle,
    state: tauri::State<'_, ShortcutSettings>,
) -> Result<(), String> {
    state.set_speech_review_active(&app, active)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_distinct() {
        let config = ShortcutConfig::default();
        for action in ACTIONS {
            let value = value_for_action(action, &config).unwrap();
            assert!(validate_shortcut(value).is_ok());
            assert!(conflicting_action(action, value, &config).is_none());
        }
        assert!(validate_shortcut(&config.ramble_toggle).is_ok());
        assert!(validate_shortcut(&config.screen_capture).is_ok());
        assert_ne!(config.ramble_toggle, config.screen_capture);
    }

    #[test]
    fn legacy_config_keeps_custom_bindings_even_if_new_defaults_collide() {
        let config =
            parse_config(r#"{"rambleToggle":"Ctrl+Shift+Enter","screenCapture":"Alt+2"}"#).unwrap();
        assert_eq!(config.ramble_toggle, "Ctrl+Shift+Enter");
        assert_eq!(config.screen_capture, "Alt+2");
        assert_eq!(config.speech_accept, "Ctrl+Alt+Enter");
        assert_eq!(config.speech_discard, DEFAULT_SPEECH_DISCARD_SHORTCUT);
        let restored = parse_config(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(config, restored);
    }

    #[test]
    fn collisions_compare_parsed_keys_including_modifier_order() {
        let config = ShortcutConfig::default();
        assert_eq!(
            conflicting_action(SPEECH_DISCARD_ACTION, "Shift+Ctrl+Enter", &config),
            Some(SPEECH_ACCEPT_ACTION)
        );
        assert!(
            parse_config(r#"{"rambleToggle":"Ctrl+Shift+R","screenCapture":"Shift+Ctrl+R"}"#)
                .is_none()
        );
    }

    #[test]
    fn review_shortcuts_only_occupy_keys_while_review_is_active() {
        for action in REVIEW_ACTIONS {
            assert!(!action_active(action, false));
            assert!(action_active(action, true));
        }
        assert!(action_active(RAMBLE_TOGGLE_ACTION, false));
        assert!(action_active(SCREEN_CAPTURE_ACTION, false));
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

    #[test]
    fn repeated_pressed_events_emit_once_until_release() {
        let pressed = AtomicBool::new(false);
        assert!(should_emit_shortcut(
            ShortcutState::Pressed,
            &pressed,
            false
        ));
        assert!(!should_emit_shortcut(
            ShortcutState::Pressed,
            &pressed,
            false
        ));
        assert!(!should_emit_shortcut(
            ShortcutState::Released,
            &pressed,
            false
        ));
        assert!(should_emit_shortcut(
            ShortcutState::Pressed,
            &pressed,
            false
        ));
    }

    #[test]
    fn shortcut_capture_swallows_the_pressed_event() {
        let pressed = AtomicBool::new(false);
        assert!(!should_emit_shortcut(
            ShortcutState::Pressed,
            &pressed,
            true
        ));
        assert!(!should_emit_shortcut(
            ShortcutState::Released,
            &pressed,
            true
        ));
    }
}
