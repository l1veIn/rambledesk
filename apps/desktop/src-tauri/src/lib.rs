mod clipboard_capture;
mod mcp_setup;
mod screen_capture;

use rambledesk_adapters::{
    ResumePrompt, WakePayload, WakeReason, WakeResult, WakeupRouter,
};
use rambledesk_core::{
    AddAttachmentInput, ApplicationError, DraftView, FeedbackApplication, FeedbackRequestSummary,
    FeedbackRequestView, FeedbackStatus, FeedbackWorkspaceView, HealthSnapshot,
    ListFeedbackRequestsInput, ListFeedbackRequestsOutput, MAX_ATTACHMENT_BYTES,
    RemoveAttachmentInput, ReorderAttachmentsInput, SaveDraftInput, SubmitFeedbackInput,
};
use rambledesk_mcp::{AccessToken, ServerConfig, ServerHandle, default_token_path, start_server};
use rambledesk_speech::{
    SpeechEvent, SpeechEventSink, SpeechProvider, SpeechSession, SpeechSessionConfig,
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};
use tauri::{
    Emitter, Manager, PhysicalPosition, PhysicalRect, PhysicalSize, RunEvent, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const TRAY_ID: &str = "rambledesk-main";
const RAMBLE_CONSOLE_LABEL: &str = "ramble-console";
const RAMBLE_TOGGLE_SHORTCUT: &str = "Ctrl+Shift+R";
const RAMBLE_CONSOLE_WIDTH: f64 = 66.0;
const RAMBLE_CONSOLE_HEIGHT: f64 = 304.0;
const RAMBLE_CONSOLE_EDGE_GAP: f64 = 10.0;
const RESUME_PROMPT_EVENT: &str = "rambledesk://resume-prompt";
const BASE_TRAY_ICON: Image<'static> = tauri::include_image!("./icons/32x32.png");

struct WorkbenchState {
    handle: ServerHandle,
    application: FeedbackApplication,
    mcp_configuration: String,
    wakeup: WakeupRouter,
    pending_count: AtomicU32,
    speech_session: tokio::sync::Mutex<Option<SpeechSession>>,
}

fn right_center_position(
    work_area: PhysicalRect<i32, u32>,
    window_size: PhysicalSize<u32>,
    scale_factor: f64,
) -> PhysicalPosition<i32> {
    let gap = (RAMBLE_CONSOLE_EDGE_GAP * scale_factor).round() as i64;
    let x = i64::from(work_area.position.x) + i64::from(work_area.size.width)
        - i64::from(window_size.width)
        - gap;
    let y = i64::from(work_area.position.y)
        + (i64::from(work_area.size.height) - i64::from(window_size.height)) / 2;
    PhysicalPosition::new(
        x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    )
}

fn position_ramble_console(app: &tauri::AppHandle, console: &WebviewWindow) -> tauri::Result<()> {
    let monitor = app
        .get_webview_window("main")
        .and_then(|window| window.current_monitor().ok().flatten())
        .or(console.primary_monitor()?);
    let Some(monitor) = monitor else {
        return Ok(());
    };
    let position = right_center_position(
        *monitor.work_area(),
        console.outer_size()?,
        monitor.scale_factor(),
    );
    console.set_position(position)
}

#[derive(Debug, Deserialize)]
struct StartVoiceRambleInput {
    request_id: String,
}

#[derive(Debug, Serialize)]
struct VoiceRambleSessionView {
    session_id: String,
    provider: String,
    model_path: String,
}

#[tauri::command]
fn get_health() -> HealthSnapshot {
    rambledesk_storage::health_snapshot()
}

#[tauri::command]
fn get_mcp_endpoint(state: tauri::State<'_, WorkbenchState>) -> String {
    state.handle.endpoint().to_owned()
}

#[tauri::command]
fn get_mcp_configuration(state: tauri::State<'_, WorkbenchState>) -> String {
    state.mcp_configuration.clone()
}

#[tauri::command]
fn detect_mcp_clients(app: tauri::AppHandle) -> Result<Vec<mcp_setup::McpClientView>, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Could not resolve the user home directory: {error}"))?;
    Ok(mcp_setup::detect_clients(&home))
}

#[tauri::command]
fn install_mcp_clients(
    client_ids: Vec<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<mcp_setup::McpInstallResult>, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Could not resolve the user home directory: {error}"))?;
    mcp_setup::install_clients(&home, &client_ids, &state.mcp_configuration)
}

#[tauri::command]
fn set_pending_count(
    count: u32,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<(), String> {
    if state.pending_count.swap(count, Ordering::Relaxed) == count {
        return Ok(());
    }
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "RambleDesk tray icon is unavailable".to_owned())?;
    tray.set_tooltip(Some(if count == 0 {
        "RambleDesk · 没有待处理反馈".to_owned()
    } else {
        format!("RambleDesk · {count} 个待处理反馈")
    }))
    .map_err(|error| error.to_string())?;
    tray.set_icon(Some(pending_tray_icon(count)))
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn list_feedback_inbox(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<FeedbackRequestSummary>, ApplicationError> {
    let application = state.application.clone();
    application.list_open_feedback_requests().await
}

#[tauri::command]
async fn list_feedback_history(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ListFeedbackRequestsOutput, ApplicationError> {
    let application = state.application.clone();
    application
        .list_feedback_requests(ListFeedbackRequestsInput {
            status: Some(vec![
                FeedbackStatus::Waiting,
                FeedbackStatus::InProgress,
                FeedbackStatus::Completed,
                FeedbackStatus::Cancelled,
            ]),
            limit: Some(100),
            ..Default::default()
        })
        .await
}

#[tauri::command]
async fn get_feedback_workspace(
    request_id: String,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackWorkspaceView, ApplicationError> {
    let application = state.application.clone();
    application.get_feedback_workspace(request_id).await
}

#[tauri::command]
async fn save_feedback_draft(
    input: SaveDraftInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<DraftView, ApplicationError> {
    let application = state.application.clone();
    application.save_feedback_draft(input).await
}

#[tauri::command]
async fn add_feedback_attachment(
    input: AddAttachmentInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackWorkspaceView, ApplicationError> {
    let application = state.application.clone();
    application.add_feedback_attachment(input).await
}

#[tauri::command]
async fn import_feedback_attachment_path(
    request_id: String,
    path: PathBuf,
    expected_revision: u64,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackWorkspaceView, ApplicationError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            ApplicationError::invalid_argument("attachment path has no UTF-8 file name")
        })?
        .to_owned();
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|_| ApplicationError::invalid_argument("attachment path could not be read"))?;
    if metadata.len() > MAX_ATTACHMENT_BYTES as u64 {
        return Err(ApplicationError::invalid_argument(format!(
            "attachment exceeds the {} MiB limit",
            MAX_ATTACHMENT_BYTES / 1024 / 1024
        )));
    }
    let contents = tokio::fs::read(path)
        .await
        .map_err(|_| ApplicationError::invalid_argument("attachment path could not be read"))?;
    let application = state.application.clone();
    application
        .add_feedback_attachment(AddAttachmentInput {
            request_id,
            file_name,
            contents,
            expected_revision,
        })
        .await
}

#[tauri::command]
async fn remove_feedback_attachment(
    input: RemoveAttachmentInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackWorkspaceView, ApplicationError> {
    let application = state.application.clone();
    application.remove_feedback_attachment(input).await
}

#[tauri::command]
async fn reorder_feedback_attachments(
    input: ReorderAttachmentsInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackWorkspaceView, ApplicationError> {
    let application = state.application.clone();
    application.reorder_feedback_attachments(input).await
}

#[tauri::command]
async fn read_feedback_attachment(
    request_id: String,
    attachment_id: String,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<u8>, ApplicationError> {
    let application = state.application.clone();
    application
        .read_feedback_attachment(request_id, attachment_id)
        .await
}

#[tauri::command]
async fn submit_feedback(
    input: SubmitFeedbackInput,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackRequestView, ApplicationError> {
    let application = state.application.clone();
    let result = application.submit_feedback(input.clone()).await?;
    deliver_wakeup_after_terminal(
        &app,
        &state.wakeup,
        &application,
        &input.request_id,
        result.status,
    )
    .await;
    Ok(result)
}

async fn deliver_wakeup_after_terminal(
    app: &tauri::AppHandle,
    router: &WakeupRouter,
    application: &FeedbackApplication,
    request_id: &str,
    status: FeedbackStatus,
) {
    let Some(reason) = WakeReason::from_status(status) else {
        return;
    };
    let (host_id, session_id) =
        match application.get_feedback_workspace(request_id.to_owned()).await {
            Ok(workspace) => (workspace.request.agent, workspace.request.session_id),
            Err(error) => {
                tracing::warn!(%request_id, %error, "wakeup: workspace lookup failed; using empty host");
                (String::new(), String::new())
            }
        };

    let payload = WakePayload {
        request_id: request_id.to_owned(),
        host_id: host_id.clone(),
        agent: host_id,
        session_id,
        reason,
    };
    match router.wake(&payload) {
        WakeResult::HostDelivered {
            adapter_id,
            host_id,
        } => {
            tracing::info!(%request_id, %adapter_id, %host_id, "host wakeup delivered");
        }
        WakeResult::UserPrompt { adapter_id, prompt } => {
            tracing::info!(
                %request_id,
                %adapter_id,
                host = %prompt.host_id,
                "generic wakeup prompt ready"
            );
            present_resume_prompt(app, &prompt);
        }
    }
}

fn present_resume_prompt(app: &tauri::AppHandle, prompt: &ResumePrompt) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
    if let Err(error) = app.emit(RESUME_PROMPT_EVENT, prompt) {
        tracing::warn!(%error, "failed to emit resume prompt event");
    }
}

#[tauri::command]
async fn start_voice_ramble(
    input: StartVoiceRambleInput,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<VoiceRambleSessionView, String> {
    let workspace = state
        .application
        .clone()
        .get_feedback_workspace(input.request_id.clone())
        .await
        .map_err(|error| error.to_string())?;
    if matches!(
        workspace.request.status,
        FeedbackStatus::Completed | FeedbackStatus::Cancelled
    ) {
        return Err("已结束的反馈请求不能继续录入语音".to_owned());
    }

    let mut active = state.speech_session.lock().await;
    if active.is_some() {
        return Err("已有语音 Ramble 正在进行，请先停止当前录音".to_owned());
    }

    let session_id = uuid::Uuid::now_v7().to_string();
    let provider = SpeechProvider::SherpaOnline;
    let model_path = configured_speech_model_path(&app)?;
    let config = SpeechSessionConfig {
        request_id: input.request_id,
        session_id: session_id.clone(),
        model_path: model_path.clone(),
    };
    let event_app = app.clone();
    let sink: SpeechEventSink = Arc::new(move |event: SpeechEvent| {
        if let Err(error) = event_app.emit("voice-ramble-event", event) {
            tracing::warn!(%error, "failed to emit voice ramble event");
        }
    });
    let session = tokio::task::spawn_blocking(move || SpeechSession::start(config, sink))
        .await
        .map_err(|error| format!("语音识别启动任务异常退出：{error}"))?
        .map_err(|error| error.to_string())?;
    *active = Some(session);

    Ok(VoiceRambleSessionView {
        session_id,
        provider: provider.id().to_owned(),
        model_path: model_path.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
async fn stop_voice_ramble(state: tauri::State<'_, WorkbenchState>) -> Result<(), String> {
    let session = state.speech_session.lock().await.take();
    let Some(session) = session else {
        return Ok(());
    };
    tokio::task::spawn_blocking(move || session.stop())
        .await
        .map_err(|error| format!("语音识别停止任务异常退出：{error}"))?
        .map_err(|error| error.to_string())
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

fn configured_path(
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

fn configured_database_path() -> Result<PathBuf, String> {
    configured_path("RAMBLEDESK_DATABASE_FILE", || {
        rambledesk_storage::default_database_path().map_err(|error| error.to_string())
    })
}

fn configured_token_path() -> Result<PathBuf, String> {
    configured_path("RAMBLEDESK_TOKEN_FILE", || {
        default_token_path().map_err(|error| error.to_string())
    })
}

fn configured_speech_model_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    configured_path("RAMBLEDESK_SHERPA_MODEL_DIR", || {
        app.path()
            .app_local_data_dir()
            .map(|directory| directory.join("models").join("sherpa-x-asr"))
            .map_err(|error| format!("无法确定 Sherpa 模型目录：{error}"))
    })
}

fn mcp_configuration(endpoint: &str, token: &AccessToken) -> String {
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

fn pending_tray_icon(count: u32) -> Image<'static> {
    let mut rgba = BASE_TRAY_ICON.rgba().to_vec();
    if count == 0 {
        return Image::new_owned(rgba, BASE_TRAY_ICON.width(), BASE_TRAY_ICON.height());
    }
    let width = BASE_TRAY_ICON.width() as i32;
    let height = BASE_TRAY_ICON.height() as i32;
    let center_x = width - 8;
    let center_y = 8;
    for y in 0..height {
        for x in 0..width {
            let dx = x - center_x;
            let dy = y - center_y;
            if dx * dx + dy * dy <= 49 {
                set_icon_pixel(&mut rgba, width, x, y, [202, 58, 47, 255]);
            }
        }
    }
    let digit = count.min(9) as usize;
    const DIGITS: [[u8; 5]; 10] = [
        [0b111, 0b101, 0b101, 0b101, 0b111],
        [0b010, 0b110, 0b010, 0b010, 0b111],
        [0b111, 0b001, 0b111, 0b100, 0b111],
        [0b111, 0b001, 0b111, 0b001, 0b111],
        [0b101, 0b101, 0b111, 0b001, 0b001],
        [0b111, 0b100, 0b111, 0b001, 0b111],
        [0b111, 0b100, 0b111, 0b101, 0b111],
        [0b111, 0b001, 0b010, 0b010, 0b010],
        [0b111, 0b101, 0b111, 0b101, 0b111],
        [0b111, 0b101, 0b111, 0b001, 0b111],
    ];
    for (row, bits) in DIGITS[digit].iter().enumerate() {
        for column in 0..3 {
            if bits & (1 << (2 - column)) != 0 {
                set_icon_pixel(
                    &mut rgba,
                    width,
                    center_x - 1 + column,
                    center_y - 2 + row as i32,
                    [255, 255, 255, 255],
                );
            }
        }
    }
    Image::new_owned(rgba, BASE_TRAY_ICON.width(), BASE_TRAY_ICON.height())
}

fn set_icon_pixel(rgba: &mut [u8], width: i32, x: i32, y: i32, color: [u8; 4]) {
    let offset = ((y * width + x) * 4) as usize;
    rgba[offset..offset + 4].copy_from_slice(&color);
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
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
            .shadow(true)
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
            let store = tauri::async_runtime::block_on(
                rambledesk_storage::SqliteFeedbackStore::connect(&database_path),
            )?;
            let application = store.into_application();
            let config = ServerConfig::new(token.clone()).with_port(configured_port()?);
            let handle = tauri::async_runtime::block_on(start_server(config, application.clone()))?;
            let configuration = mcp_configuration(handle.endpoint(), &token);
            let open_item = MenuItem::with_id(app, "open", "打开 RambleDesk", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &quit_item])?;
            TrayIconBuilder::with_id(TRAY_ID)
                .icon(pending_tray_icon(0))
                .tooltip("RambleDesk · 没有待处理反馈")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main_window(app),
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
                handle,
                application,
                mcp_configuration: configuration,
                // Specific host adapters register here later; unmatched hosts use generic UI.
                wakeup: WakeupRouter::default(),
                pending_count: AtomicU32::new(0),
                speech_session: tokio::sync::Mutex::new(None),
            });
            app.manage(screen_capture::ScreenCaptureState::default());
            app.manage(clipboard_capture::ClipboardCaptureState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_health,
            get_mcp_endpoint,
            get_mcp_configuration,
            detect_mcp_clients,
            install_mcp_clients,
            set_pending_count,
            list_feedback_inbox,
            list_feedback_history,
            get_feedback_workspace,
            save_feedback_draft,
            add_feedback_attachment,
            import_feedback_attachment_path,
            remove_feedback_attachment,
            reorder_feedback_attachments,
            read_feedback_attachment,
            submit_feedback,
            start_voice_ramble,
            stop_voice_ramble,
            clipboard_capture::capture_clipboard_once,
            clipboard_capture::start_clipboard_capture,
            clipboard_capture::stop_clipboard_capture,
            clipboard_capture::read_clipboard_capture_image,
            clipboard_capture::discard_clipboard_capture_image,
            screen_capture::begin_screen_capture,
            screen_capture::get_screen_capture_view,
            screen_capture::read_screen_capture_preview,
            screen_capture::complete_screen_capture,
            screen_capture::read_completed_screen_capture,
            screen_capture::discard_screen_capture,
            screen_capture::cancel_screen_capture,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build RambleDesk desktop app");

    app.run(|app_handle, event| {
        if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. })
            && let Some(state) = app_handle.try_state::<WorkbenchState>()
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

    #[test]
    fn configured_paths_default_when_overrides_are_absent() {
        if std::env::var_os("RAMBLEDESK_DATABASE_FILE").is_none() {
            assert_eq!(
                configured_database_path().expect("default database"),
                rambledesk_storage::default_database_path().expect("storage default")
            );
        }
        if std::env::var_os("RAMBLEDESK_TOKEN_FILE").is_none() {
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
    fn copied_mcp_configuration_contains_http_endpoint_and_bearer_token() {
        let token =
            AccessToken::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .expect("token");
        let configuration = mcp_configuration("http://127.0.0.1:37642/mcp", &token);
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
