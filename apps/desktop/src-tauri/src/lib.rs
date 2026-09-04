mod clipboard_capture;
mod diagnostics;
mod dsh_install;
mod logging;
mod macos_permissions;
mod managed_commands;
mod notification_sounds;
mod open_attachment;
mod pi_install;
mod screen_capture;
mod shortcuts;
mod speech_plugin;
mod web_access;

use rambledesk_core::{
    ApplicationChangeHub, ApplicationCommandFacade, ApplicationHostProfileView,
    FeedbackApplication, SessionApplication, WorkbenchTerminalOperations,
};
use rambledesk_hosts::{
    ContinuationMode, ContinuationRouter, HostAdapter, known_continuation_strategies,
    known_host_profiles,
};
use rambledesk_local_server::{
    AccessToken, LocalManagedFeedbackProvider, ServerConfig, ServerHandle,
    start_server_with_managed,
};
use rambledesk_speech::SpeechSession;
use std::{
    path::PathBuf,
    sync::{Arc, RwLock, atomic::AtomicU32},
};
use tauri::{
    Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::Color,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

const TRAY_ID: &str = "rambledesk-main";
const RAMBLE_CONSOLE_LABEL: &str = "ramble-console";
const RAMBLE_CONSOLE_WIDTH: f64 = 58.0;
const RAMBLE_CONSOLE_HEIGHT: f64 = 304.0;
const RAMBLE_CONSOLE_EDGE_GAP: f64 = 10.0;
const RESUME_PROMPT_EVENT: &str = "rambledesk://resume-prompt";
const OPEN_ADAPTERS_EVENT: &str = "rambledesk://open-adapters";
const BASE_TRAY_ICON: Image<'static> = tauri::include_image!("./icons/32x32.png");

struct WorkbenchState {
    local_server: ServerHandle,
    application: FeedbackApplication,
    application_commands: Arc<ApplicationCommandFacade>,
    sessions: SessionApplication,
    application_change_hub: Arc<ApplicationChangeHub>,
    web_access_lifecycle: tokio::sync::Mutex<web_access::WebAccessLifecycle>,
    web_access_credential_store: Arc<dyn web_access::WebAccessCredentialStore>,
    store: rambledesk_storage::SqliteFeedbackStore,
    generic_mcp_configuration: String,
    pending_count: AtomicU32,
    library_root: RwLock<PathBuf>,
    speech_session: tokio::sync::Mutex<Option<SpeechSession>>,
}

impl WorkbenchState {
    fn library_root(&self) -> PathBuf {
        self.library_root
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    fn activate_library_root(&self, path: PathBuf) {
        self.store.set_library_root(path.clone());
        *self
            .library_root
            .write()
            .unwrap_or_else(|error| error.into_inner()) = path;
    }
}

fn application_host_profiles() -> Vec<ApplicationHostProfileView> {
    known_host_profiles()
        .into_iter()
        .map(|profile| ApplicationHostProfileView {
            id: profile.id,
            label: profile.label,
            icon_svg: profile.icon_svg,
            default_adapter: match profile.default_adapter {
                HostAdapter::GenericMcp => "generic_mcp",
                HostAdapter::PiNative => "pi_native",
            }
            .into(),
            continuation_mode: match profile.continuation_mode {
                ContinuationMode::NotRequired => "not_required",
                ContinuationMode::Manual => "manual",
                ContinuationMode::Native => "native",
            }
            .into(),
        })
        .collect()
}

#[tauri::command]
fn log_frontend_error(context: String, message: String) {
    logging::frontend_error(&context, &message);
}

mod window;

use window::{
    attach_ramble_console_events, hide_ramble_console, position_ramble_console, show_main_window,
    show_ramble_console,
};

mod commands;

use commands::*;

mod continuation;

mod config;

use config::*;

mod tray;

use tray::pending_tray_icon;

pub fn run() {
    logging::init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let setup = (|| -> Result<(), Box<dyn std::error::Error>> {
                let console = WebviewWindowBuilder::new(
                    app,
                    RAMBLE_CONSOLE_LABEL,
                    WebviewUrl::App("index.html#ramble-console".into()),
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
                attach_ramble_console_events(&console);
                app.manage(window::SpeechOverlayVisibility::default());
                let speech_overlay = WebviewWindowBuilder::new(
                    app,
                    "speech-overlay",
                    WebviewUrl::App("index.html#speech-overlay".into()),
                )
                .title("RambleDesk · Speech")
                .inner_size(436.0, 140.0)
                .resizable(false)
                .decorations(false)
                .transparent(true)
                .background_color(Color(0, 0, 0, 0))
                .accept_first_mouse(true)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .visible_on_all_workspaces(true)
                .focused(false)
                .visible(false)
                .build()?;
                window::attach_speech_overlay_events(&speech_overlay);
                app.manage(shortcuts::ShortcutSettings::initialize(app.handle()));
                let token = AccessToken::load_or_create(&configured_token_path()?)?;
                let database_path = configured_database_path()?;
                let library_root = configured_library_path()?;
                let store = tauri::async_runtime::block_on(
                    rambledesk_storage::SqliteFeedbackStore::connect_with_library(
                        &database_path,
                        &library_root,
                    ),
                )?;
                let application_change_hub = Arc::new(ApplicationChangeHub::new());
                let application = store
                    .clone()
                    .into_application()
                    .with_change_observer(application_change_hub.clone());
                let terminal_observer =
                    Arc::new(continuation::DesktopTerminalOperationObserver::new(
                        app.handle().clone(),
                        ContinuationRouter::new(known_continuation_strategies()),
                        application.clone(),
                    ));
                let terminal_operations =
                    WorkbenchTerminalOperations::new(application.clone(), terminal_observer);
                let feedback_provider =
                    Arc::new(LocalManagedFeedbackProvider::new(application.clone()));
                let sessions = SessionApplication::new(
                    Arc::new(store.clone()),
                    Arc::new(store.clone()),
                    Arc::new(rambledesk_acp::AcpSessionDriver),
                )
                .with_change_observer(application_change_hub.clone())
                .with_feedback_provider(feedback_provider.clone())
                .with_deliveries(Arc::new(store.clone()));
                let application_commands = Arc::new(
                    ApplicationCommandFacade::new(
                        application.clone(),
                        terminal_operations,
                        application_host_profiles(),
                    )
                    .with_sessions(sessions.clone()),
                );
                let config = ServerConfig::new(token.clone()).with_port(configured_port()?);
                let handle = tauri::async_runtime::block_on(start_server_with_managed(
                    config,
                    application.clone(),
                    feedback_provider,
                ))?;
                let configuration = generic_mcp_configuration(handle.endpoint(), &token);
                let open_item =
                    MenuItem::with_id(app, "open", "打开 RambleDesk", true, None::<&str>)?;
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
                tauri::async_runtime::block_on(sessions.start_delivery_worker())?;
                app.manage(WorkbenchState {
                    local_server: handle,
                    application,
                    application_commands,
                    sessions,
                    application_change_hub,
                    web_access_lifecycle: tokio::sync::Mutex::new(
                        web_access::WebAccessLifecycle::default(),
                    ),
                    web_access_credential_store: Arc::new(web_access::OsWebAccessCredentialStore),
                    store,
                    generic_mcp_configuration: configuration,
                    pending_count: AtomicU32::new(0),
                    library_root: RwLock::new(library_root),
                    speech_session: tokio::sync::Mutex::new(None),
                });
                app.manage(screen_capture::ScreenCaptureState::default());
                app.manage(clipboard_capture::ClipboardCaptureState::default());
                if let Err(error) = screen_capture::prepare_screen_capture_overlay(app.handle()) {
                    tracing::warn!(%error, "failed to prewarm the screenshot editor");
                }
                diagnostics::record_event("app_started", None, None, Some("ok"), None, None);
                Ok(())
            })();

            // Tauri turns a setup-hook error into a panic from its event-loop
            // callback, which on macOS aborts the process without any visible
            // explanation. Handle failures here: log the reason, ask the user,
            // and exit cleanly instead.
            if let Err(error) = setup {
                let message = error.to_string();
                tracing::error!(%message, "RambleDesk setup failed");
                let title = format!("{} · 启动失败", app.package_info().name);
                let text = format!(
                    "RambleDesk 无法启动。\n\n{message}\n\n诊断日志目录：\n{}",
                    logging::directory_hint()
                );
                let handle = app.handle().clone();
                let (dialog_done_tx, dialog_done_rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = handle
                        .dialog()
                        .message(text)
                        .title(title)
                        .buttons(MessageDialogButtons::Ok)
                        .blocking_show();
                    let _ = dialog_done_tx.send(());
                });
                // Give the dialog a moment to be dismissed before exiting.
                let _ = dialog_done_rx.recv_timeout(std::time::Duration::from_secs(30));
                std::process::exit(1);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            managed_commands::list_agent_configs,
            managed_commands::save_agent_config,
            managed_commands::delete_agent_config,
            managed_commands::check_agent_config,
            managed_commands::create_managed_session,
            managed_commands::get_managed_session,
            managed_commands::start_managed_session,
            managed_commands::stop_managed_session,
            managed_commands::send_managed_prompt,
            managed_commands::cancel_managed_prompt,
            managed_commands::respond_managed_permission,
            managed_commands::resolve_feedback_delivery,
            show_ramble_console,
            hide_ramble_console,
            window::set_speech_overlay_layout,
            window::focus_speech_feedback,
            get_generic_mcp_configuration,
            restart_application,
            open_main_devtools,
            get_data_storage_settings,
            set_data_storage_path,
            list_host_profiles,
            detect_generic_mcp_hosts,
            install_generic_mcp_hosts,
            get_pi_package_status,
            install_pi_package,
            uninstall_pi_package,
            dsh_install::detect_dsh_host,
            dsh_install::install_dsh_package,
            set_pending_count,
            list_feedback_inbox,
            list_host_sessions,
            list_archived_host_sessions,
            rename_host_session,
            set_host_session_pinned,
            archive_host_session,
            unarchive_host_session,
            delete_host_session,
            delete_feedback_request,
            set_host_pinned,
            list_feedback_requests,
            get_feedback_workspace,
            read_published_feedback,
            save_feedback_draft,
            add_feedback_attachment,
            import_feedback_attachment_path,
            diagnostics::export_diagnostics,
            diagnostics::record_diagnostic_event,
            remove_feedback_attachment,
            reorder_feedback_attachments,
            read_feedback_attachment,
            read_request_attachment,
            open_attachment::open_feedback_attachment,
            open_attachment::reveal_feedback_attachment,
            open_attachment::reveal_feedback_package,
            open_attachment::reveal_path_in_folder,
            submit_feedback,
            approve_feedback_request,
            cancel_feedback_request,
            notification_sounds::import_notification_sound,
            notification_sounds::commit_notification_sound,
            notification_sounds::read_notification_sound,
            notification_sounds::remove_notification_sound,
            list_speech_models,
            download_speech_model,
            delete_speech_model,
            list_speech_input_devices,
            start_voice_ramble,
            stop_voice_ramble,
            macos_permissions::list_macos_permissions,
            macos_permissions::request_macos_permission,
            macos_permissions::open_macos_privacy_settings,
            clipboard_capture::capture_clipboard_once,
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
            shortcuts::get_shortcut_settings,
            shortcuts::set_shortcut_setting,
            shortcuts::reset_shortcut_settings,
            shortcuts::set_shortcut_capture_active,
            shortcuts::set_speech_review_shortcuts_active,
            web_access::get_web_access_status,
            web_access::start_web_access,
            web_access::stop_web_access,
            web_access::copy_web_access_token,
            web_access::open_web_access,
            log_frontend_error,
        ])
        .build(tauri::generate_context!());

    let app = match app {
        Ok(app) => app,
        Err(error) => {
            logging::show_fatal_startup_error(&error.to_string());
            return;
        }
    };

    app.run(|app_handle, event| {
        #[cfg(target_os = "macos")]
        if matches!(event, RunEvent::Reopen { .. }) {
            show_main_window(app_handle);
        }
        if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. })
            && let Some(state) = app_handle.try_state::<WorkbenchState>()
        {
            if tauri::async_runtime::block_on(state.sessions.shutdown()).is_err() {
                tracing::warn!("managed session shutdown completed with a cleanup error");
            }
            state.local_server.cancel();
            if let Ok(lifecycle) = state.web_access_lifecycle.try_lock() {
                lifecycle.cancel_active();
            }
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
    fn default_capability_url_scopes_do_not_use_invalid_recursive_globs() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json"))
                .expect("default capability JSON");
        let permissions = capability["permissions"]
            .as_array()
            .expect("default capability permissions");
        let mut invalid_urls = Vec::new();
        for permission in permissions {
            let Some(allow) = permission
                .get("allow")
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for scope in allow {
                let Some(url) = scope.get("url").and_then(serde_json::Value::as_str) else {
                    continue;
                };
                if url.contains("://**") || url.contains(":**") {
                    invalid_urls.push(url.to_owned());
                }
            }
        }

        assert!(
            invalid_urls.is_empty(),
            "URL scopes use invalid recursive glob patterns: {invalid_urls:?}"
        );
    }

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
