mod clipboard_capture;
mod generic_mcp_install;
mod pi_install;
mod platform;
mod screen_capture;

use rambledesk_core::FeedbackApplication;
use rambledesk_hosts::{ContinuationRouter, known_continuation_strategies};
use rambledesk_local_server::{AccessToken, ServerConfig, ServerHandle, start_server};
use rambledesk_speech::SpeechSession;
use std::{path::PathBuf, sync::atomic::AtomicU32};
use tauri::{
    Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::Color,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const TRAY_ID: &str = "rambledesk-main";
const RAMBLE_CONSOLE_LABEL: &str = "ramble-console";
const RAMBLE_TOGGLE_SHORTCUT: &str = "Ctrl+Shift+R";
const RAMBLE_CONSOLE_WIDTH: f64 = 58.0;
const RAMBLE_CONSOLE_HEIGHT: f64 = 304.0;
const RAMBLE_CONSOLE_EDGE_GAP: f64 = 10.0;
const RESUME_PROMPT_EVENT: &str = "rambledesk://resume-prompt";
const OPEN_ADAPTERS_EVENT: &str = "rambledesk://open-adapters";
const BASE_TRAY_ICON: Image<'static> = tauri::include_image!("./icons/32x32.png");

struct WorkbenchState {
    local_server: ServerHandle,
    application: FeedbackApplication,
    generic_mcp_configuration: String,
    continuation: ContinuationRouter,
    pending_count: AtomicU32,
    library_root: PathBuf,
    speech_session: tokio::sync::Mutex<Option<SpeechSession>>,
}

mod window;

use window::{position_ramble_console, show_main_window};

mod commands;

use commands::*;

mod continuation;

mod config;

use config::*;

mod tray;

use tray::pending_tray_icon;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rambledesk=info".into()),
        )
        .with_target(false)
        .init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let console = WebviewWindowBuilder::new(
                app,
                RAMBLE_CONSOLE_LABEL,
                WebviewUrl::App("ramble-console".into()),
            )
            .title("RambleDesk · Ramble Console")
            .inner_size(RAMBLE_CONSOLE_WIDTH, RAMBLE_CONSOLE_HEIGHT)
            .min_inner_size(RAMBLE_CONSOLE_WIDTH, RAMBLE_CONSOLE_HEIGHT)
            .max_inner_size(RAMBLE_CONSOLE_WIDTH, RAMBLE_CONSOLE_HEIGHT)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .background_color(Color(0, 0, 0, 0))
            .accept_first_mouse(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible_on_all_workspaces(true)
            .visible(false)
            .build()?;
            position_ramble_console(app.handle(), &console)?;
            let console_to_hide = console.clone();
            console.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = console_to_hide.hide();
                }
            });
            if let Err(error) =
                app.global_shortcut()
                    .on_shortcut(RAMBLE_TOGGLE_SHORTCUT, |app, _, event| {
                        if event.state == ShortcutState::Pressed
                            && let Err(error) = app.emit_to(
                                "main",
                                "ramble-toggle-shortcut",
                                RAMBLE_TOGGLE_SHORTCUT,
                            )
                        {
                            tracing::warn!(%error, "failed to emit Ramble toggle shortcut");
                        }
                    })
            {
                tracing::warn!(
                    %error,
                    shortcut = RAMBLE_TOGGLE_SHORTCUT,
                    "Ramble toggle global shortcut is unavailable"
                );
            }
            if let Err(error) = app.global_shortcut().on_shortcut(
                screen_capture::SCREEN_CAPTURE_SHORTCUT,
                |app, _, event| {
                    if event.state == ShortcutState::Pressed
                        && let Err(error) = app.emit_to(
                            "main",
                            "screen-capture-shortcut",
                            screen_capture::SCREEN_CAPTURE_SHORTCUT,
                        )
                    {
                        tracing::warn!(%error, "failed to emit screen capture shortcut");
                    }
                },
            ) {
                tracing::warn!(
                    %error,
                    shortcut = screen_capture::SCREEN_CAPTURE_SHORTCUT,
                    "screen capture global shortcut is unavailable"
                );
            }
            let token = AccessToken::load_or_create(&configured_token_path()?)?;
            let database_path = configured_database_path()?;
            let library_root = configured_library_path()?;
            let store = tauri::async_runtime::block_on(
                rambledesk_storage::SqliteFeedbackStore::connect_with_library(
                    &database_path,
                    &library_root,
                ),
            )?;
            let application = store.into_application();
            let config = ServerConfig::new(token.clone()).with_port(configured_port()?);
            let handle = tauri::async_runtime::block_on(start_server(config, application.clone()))?;
            let configuration = generic_mcp_configuration(handle.endpoint(), &token);
            let open_item = MenuItem::with_id(app, "open", "打开 RambleDesk", true, None::<&str>)?;
            let adapters_item =
                MenuItem::with_id(app, "adapters", "适配器设置", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &adapters_item, &quit_item])?;
            TrayIconBuilder::with_id(TRAY_ID)
                .icon(pending_tray_icon(0))
                .tooltip("RambleDesk · 没有待处理反馈")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main_window(app),
                    "adapters" => {
                        show_main_window(app);
                        if let Err(error) = app.emit(OPEN_ADAPTERS_EVENT, ()) {
                            tracing::warn!(%error, "failed to emit open adapters event");
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            if let Some(window) = app.get_webview_window("main") {
                let window_to_hide = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_to_hide.hide();
                    }
                });
            }
            app.manage(WorkbenchState {
                local_server: handle,
                application,
                generic_mcp_configuration: configuration,
                continuation: ContinuationRouter::new(known_continuation_strategies()),
                pending_count: AtomicU32::new(0),
                library_root,
                speech_session: tokio::sync::Mutex::new(None),
            });
            app.manage(screen_capture::ScreenCaptureState::default());
            app.manage(clipboard_capture::ClipboardCaptureState::default());
            if let Err(error) = screen_capture::prepare_screen_capture_overlay(app.handle()) {
                tracing::warn!(%error, "failed to prewarm the screenshot editor");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_generic_mcp_configuration,
            get_data_storage_settings,
            set_data_storage_path,
            list_host_profiles,
            detect_generic_mcp_hosts,
            install_generic_mcp_hosts,
            install_pi_package,
            set_pending_count,
            list_feedback_inbox,
            list_host_sessions,
            list_feedback_requests,
            get_feedback_workspace,
            save_feedback_draft,
            add_feedback_attachment,
            import_feedback_attachment_path,
            remove_feedback_attachment,
            reorder_feedback_attachments,
            read_feedback_attachment,
            submit_feedback,
            approve_feedback_request,
            cancel_feedback_request,
            get_speech_model,
            download_speech_model,
            delete_speech_model,
            list_speech_input_devices,
            start_voice_ramble,
            stop_voice_ramble,
            clipboard_capture::capture_clipboard_once,
            clipboard_capture::start_clipboard_capture,
            clipboard_capture::stop_clipboard_capture,
            clipboard_capture::read_clipboard_capture_image,
            clipboard_capture::discard_clipboard_capture_image,
            screen_capture::overlay::begin_screen_capture,
            screen_capture::overlay::get_active_capture_info,
            screen_capture::overlay::read_capture_rgba_bytes,
            screen_capture::overlay::show_screen_capture_overlay,
            screen_capture::overlay::complete_screen_capture,
            screen_capture::pin::pin_screen_capture,
            screen_capture::pin::read_pinned_screen_capture,
            screen_capture::pin::close_pinned_screen_capture,
            screen_capture::scroll::begin_scrolling_capture,
            screen_capture::scroll::get_scrolling_capture_info,
            screen_capture::scroll::append_scrolling_capture_frame,
            screen_capture::scroll::finish_scrolling_capture,
            screen_capture::lifecycle::read_completed_screen_capture,
            screen_capture::lifecycle::discard_screen_capture,
            screen_capture::lifecycle::cancel_screen_capture,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build RambleDesk desktop app");

    app.run(|app_handle, event| {
        if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. })
            && let Some(state) = app_handle.try_state::<WorkbenchState>()
        {
            state.local_server.cancel();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::right_center_position;
    use rambledesk_local_server::default_token_path;
    use tauri::{PhysicalPosition, PhysicalRect, PhysicalSize};

    #[test]
    fn default_port_is_stable_when_env_is_absent() {
        // The environment is intentionally not mutated because tests may run concurrently.
        if std::env::var_os("RAMBLEDESK_LOCAL_SERVER_PORT").is_none() {
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
        if std::env::var_os("RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE").is_none() {
            assert_eq!(
                configured_token_path().expect("default token"),
                default_token_path().expect("token default")
            );
        }
    }

    #[test]
    fn pending_tray_badge_changes_pixels_without_resizing_icon() {
        let idle = pending_tray_icon(0);
        let pending = pending_tray_icon(3);
        assert_eq!(idle.width(), pending.width());
        assert_eq!(idle.height(), pending.height());
        assert_ne!(idle.rgba(), pending.rgba());
    }

    #[test]
    fn ramble_console_defaults_to_right_center_with_logical_ten_pixel_gap() {
        let position = right_center_position(
            PhysicalRect {
                position: PhysicalPosition::new(-1_920, 40),
                size: PhysicalSize::new(1_920, 1_040),
            },
            PhysicalSize::new(132, 608),
            2.0,
        );
        assert_eq!(position, PhysicalPosition::new(-152, 256));
    }

    #[test]
    fn copied_generic_mcp_configuration_contains_http_endpoint_and_bearer_token() {
        let token =
            AccessToken::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .expect("token");
        let configuration = generic_mcp_configuration("http://127.0.0.1:37642/mcp", &token);
        let value: serde_json::Value =
            serde_json::from_str(&configuration).expect("configuration JSON");
        assert_eq!(
            value["mcpServers"]["rambledesk"]["url"],
            "http://127.0.0.1:37642/mcp"
        );
        assert_eq!(
            value["mcpServers"]["rambledesk"]["headers"]["Authorization"],
            format!("Bearer {}", token.secret())
        );
    }
}
