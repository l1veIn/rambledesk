use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};

use rambledesk_core::{
    AddAttachmentInput, ApplicationError, ApplicationFeedbackRequestView,
    ApplicationFeedbackWorkspaceView, ApplicationHostProfileView, ApproveFeedbackInput,
    CancelFeedbackInput, DeleteFeedbackRequestInput, DraftView, FeedbackPackageView,
    FeedbackRequestSummary, FeedbackStatus, GetFeedbackInput, HostSessionInput, HostSessionSummary,
    ListFeedbackRequestsInput, ListFeedbackRequestsOutput, ListHostSessionsInput,
    MAX_ATTACHMENT_BYTES, ReadAttachmentInput, RemoveAttachmentInput, RenameHostSessionInput,
    ReorderAttachmentsInput, SaveDraftInput, SetHostPinnedInput, SetHostSessionPinnedInput,
    SubmitFeedbackInput,
};
use rambledesk_speech::{
    SpeechEvent, SpeechEventSink, SpeechProvider, SpeechSession, SpeechSessionConfig,
    ensure_vad_model, list_input_devices,
    model::{SpeechModelInfo, delete_model, download_model, list_models, model_dir, model_info},
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, ipc::Response};

use rambledesk_mcp::{McpHostView, McpInstallResult, detect_hosts, install_hosts};

use super::{
    TRAY_ID, WorkbenchState, clipboard_capture::ClipboardCaptureState, diagnostics,
    migrate_library, pending_tray_icon, pi_install, save_library_path,
    screen_capture::ScreenCaptureState,
};

#[derive(Debug, Deserialize)]
pub(super) struct StartVoiceRambleInput {
    request_id: String,
    input_device: Option<String>,
    model_id: String,
    vad_threshold: f32,
    vad_silence_ms: u32,
    hotwords: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct VoiceRambleSessionView {
    voice_session_id: String,
    provider: String,
    model_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct StorageMigrationProgress {
    copied: u64,
    total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SpeechModelProgress {
    model_id: String,
    downloaded: u64,
    total: u64,
}

#[derive(Debug, Serialize)]
pub(super) struct DataStorageView {
    active_path: String,
    selected_path: String,
    restart_required: bool,
}

#[tauri::command]
fn display_path(path: &std::path::Path) -> String {
    let value = path.to_string_lossy();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
}

#[tauri::command]
pub(super) fn restart_application(app: tauri::AppHandle) {
    app.restart();
}

#[tauri::command]
pub(super) fn open_main_devtools(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(debug_assertions)]
    {
        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "主窗口不可用".to_owned())?;
        window.open_devtools();
        Ok(())
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = app;
        Err("开发者工具仅在开发构建中可用".to_owned())
    }
}

#[tauri::command]
pub(super) fn get_data_storage_settings(
    state: tauri::State<'_, WorkbenchState>,
) -> DataStorageView {
    let active_root = state.library_root();
    let active_path = display_path(&active_root);
    DataStorageView {
        selected_path: active_path.clone(),
        active_path,
        restart_required: false,
    }
}

#[tauri::command]
pub(super) async fn set_data_storage_path(
    path: PathBuf,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<DataStorageView, String> {
    if !path.is_absolute() {
        return Err("数据存储位置必须是绝对路径".to_owned());
    }
    if !state
        .application
        .list_open_feedback_requests()
        .await
        .map_err(|error| error.to_string())?
        .is_empty()
    {
        return Err(
            "仍有未提交的反馈请求。请先提交或取消所有进行中的反馈，再迁移数据。".to_owned(),
        );
    }
    let source = state.library_root();
    let destination = path.clone();
    let event_app = app.clone();
    let migrated_bytes = tokio::task::spawn_blocking(move || {
        migrate_library(&source, &destination, &|copied, total| {
            let _ = event_app.emit(
                "storage-migration-progress",
                StorageMigrationProgress { copied, total },
            );
        })
    })
    .await
    .map_err(|error| format!("数据迁移任务异常退出：{error}"))??;
    let selected = save_library_path(&path)?;
    if migrated_bytes == 0 {
        state.activate_library_root(selected.clone());
        return Ok(DataStorageView {
            active_path: display_path(&selected),
            selected_path: display_path(&selected),
            restart_required: false,
        });
    }
    let active_root = state.library_root();
    Ok(DataStorageView {
        active_path: display_path(&active_root),
        selected_path: display_path(&selected),
        restart_required: selected != active_root,
    })
}

#[tauri::command]
pub(super) fn get_generic_mcp_configuration(state: tauri::State<'_, WorkbenchState>) -> String {
    state.generic_mcp_configuration.clone()
}

#[tauri::command]
pub(super) fn list_host_profiles(
    state: tauri::State<'_, WorkbenchState>,
) -> Vec<ApplicationHostProfileView> {
    state.application_commands.list_host_profiles()
}

#[tauri::command]
pub(super) fn detect_generic_mcp_hosts(app: tauri::AppHandle) -> Result<Vec<McpHostView>, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Could not resolve the user home directory: {error}"))?;
    Ok(detect_hosts(&home))
}

#[tauri::command]
pub(super) fn install_generic_mcp_hosts(
    host_ids: Vec<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<McpInstallResult>, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Could not resolve the user home directory: {error}"))?;
    install_hosts(&home, &host_ids, &state.generic_mcp_configuration)
}

#[tauri::command]
pub(super) async fn install_pi_package(
    app: tauri::AppHandle,
    checkout_root: Option<String>,
) -> Result<String, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("Could not resolve bundled application resources: {error}"))?;
    let package_dir =
        pi_install::resolve_package_dir(checkout_root.as_deref(), Some(&resource_dir)).ok_or_else(
            || {
                "Could not locate the bundled pi-rambledesk package. Reinstall RambleDesk or run `pi install npm:@rambledesk/pi` manually."
                    .to_owned()
            },
        )?;
    let home = app.path().home_dir().ok();
    let pi_bin = pi_install::resolve_pi_binary(home.as_deref()).ok_or_else(|| {
        "The `pi` CLI was not found. RambleDesk checked PATH and common macOS package-manager locations. Install Pi, set RAMBLEDESK_PI_BIN, or run `pi install npm:@rambledesk/pi` manually.".to_owned()
    })?;
    tauri::async_runtime::spawn_blocking(move || {
        pi_install::run_install(&pi_bin, &package_dir, home.as_deref())
    })
    .await
    .map_err(|error| format!("Installer task failed: {error}"))?
}

#[tauri::command]
pub(super) fn get_pi_package_status(
    app: tauri::AppHandle,
    checkout_root: Option<String>,
) -> Result<pi_install::PiPackageStatus, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Could not resolve the user home directory: {error}"))?;
    let resource_dir = app.path().resource_dir().ok();
    let package_dir =
        pi_install::resolve_package_dir(checkout_root.as_deref(), resource_dir.as_deref());
    pi_install::package_status(&home, package_dir.as_deref())
}

#[tauri::command]
pub(super) async fn uninstall_pi_package(
    app: tauri::AppHandle,
    checkout_root: Option<String>,
) -> Result<String, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Could not resolve the user home directory: {error}"))?;
    let resource_dir = app.path().resource_dir().ok();
    let package_dir =
        pi_install::resolve_package_dir(checkout_root.as_deref(), resource_dir.as_deref());
    let pi_bin = pi_install::resolve_pi_binary(Some(&home)).ok_or_else(|| {
        "The `pi` CLI was not found. RambleDesk checked PATH and common package-manager locations. Reinstall Pi before removing its RambleDesk adapter."
            .to_owned()
    })?;
    tauri::async_runtime::spawn_blocking(move || {
        pi_install::run_uninstall(&pi_bin, &home, package_dir.as_deref())
    })
    .await
    .map_err(|error| format!("Uninstaller task failed: {error}"))?
}

#[tauri::command]
pub(super) fn set_pending_count(
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
pub(super) async fn list_feedback_inbox(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<FeedbackRequestSummary>, ApplicationError> {
    state.application_commands.list_feedback_inbox().await
}

#[tauri::command]
pub(super) async fn list_host_sessions(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<HostSessionSummary>, ApplicationError> {
    state.application_commands.list_host_sessions().await
}

#[tauri::command]
pub(super) async fn list_archived_host_sessions(
    input: ListHostSessionsInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<HostSessionSummary>, ApplicationError> {
    state
        .application_commands
        .list_archived_host_sessions(input)
        .await
}

#[tauri::command]
pub(super) async fn rename_host_session(
    input: RenameHostSessionInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<HostSessionSummary, ApplicationError> {
    state.application_commands.rename_host_session(input).await
}

#[tauri::command]
pub(super) async fn set_host_session_pinned(
    input: SetHostSessionPinnedInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<HostSessionSummary, ApplicationError> {
    state
        .application_commands
        .set_host_session_pinned(input)
        .await
}

#[tauri::command]
pub(super) async fn archive_host_session(
    input: HostSessionInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<HostSessionSummary, ApplicationError> {
    state.application_commands.archive_host_session(input).await
}

#[tauri::command]
pub(super) async fn unarchive_host_session(
    input: HostSessionInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<HostSessionSummary, ApplicationError> {
    state
        .application_commands
        .unarchive_host_session(input)
        .await
}

#[tauri::command]
pub(super) async fn delete_host_session(
    input: HostSessionInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<(), ApplicationError> {
    state.application_commands.delete_host_session(input).await
}

#[tauri::command]
pub(super) async fn delete_feedback_request(
    input: DeleteFeedbackRequestInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<(), ApplicationError> {
    state
        .application_commands
        .delete_feedback_request(input)
        .await
}

#[tauri::command]
pub(super) async fn set_host_pinned(
    input: SetHostPinnedInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<HostSessionSummary>, ApplicationError> {
    state.application_commands.set_host_pinned(input).await
}

#[tauri::command]
pub(super) async fn list_feedback_requests(
    input: ListFeedbackRequestsInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ListFeedbackRequestsOutput, ApplicationError> {
    state
        .application_commands
        .list_feedback_requests(input)
        .await
}

#[tauri::command]
pub(super) async fn get_feedback_workspace(
    input: GetFeedbackInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ApplicationFeedbackWorkspaceView, ApplicationError> {
    state
        .application_commands
        .get_feedback_workspace(input)
        .await
}

#[tauri::command]
pub(super) async fn read_published_feedback(
    input: GetFeedbackInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Option<FeedbackPackageView>, ApplicationError> {
    state
        .application_commands
        .read_published_feedback(input)
        .await
}

#[tauri::command]
pub(super) async fn save_feedback_draft(
    input: SaveDraftInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<DraftView, ApplicationError> {
    state.application_commands.save_feedback_draft(input).await
}

#[tauri::command]
pub(super) async fn add_feedback_attachment(
    input: AddAttachmentInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ApplicationFeedbackWorkspaceView, ApplicationError> {
    state
        .application_commands
        .add_feedback_attachment(input)
        .await
}

#[tauri::command]
pub(super) async fn add_completed_screen_capture(
    request_id: String,
    capture_session_id: String,
    expected_revision: u64,
    capture_state: tauri::State<'_, ScreenCaptureState>,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ApplicationFeedbackWorkspaceView, ApplicationError> {
    let started = std::time::Instant::now();
    tracing::info!(%request_id, %capture_session_id, "add_completed_screen_capture: start");
    let contents = capture_state
        .take_completed_png(&capture_session_id)
        .map_err(ApplicationError::invalid_argument)?;
    tracing::info!(
        png_bytes = contents.len(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "add_completed_screen_capture: png taken"
    );
    if contents.len() > MAX_ATTACHMENT_BYTES {
        return Err(ApplicationError::invalid_argument(format!(
            "attachment exceeds the {} MiB limit",
            MAX_ATTACHMENT_BYTES / 1024 / 1024
        )));
    }
    let application = state.application.clone();
    diagnostics::record_event(
        "screen_capture_imported",
        Some(&request_id),
        None,
        Some("ok"),
        None,
        None,
    );
    let result = application
        .add_feedback_attachment(AddAttachmentInput {
            request_id,
            file_name: format!("ramble-screenshot-{capture_session_id}.png"),
            contents,
            expected_revision,
        })
        .await;
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis() as u64,
        ok = result.is_ok(),
        "add_completed_screen_capture: attachment saved"
    );
    result.map(Into::into)
}

#[tauri::command]
pub(super) async fn add_completed_clipboard_capture(
    request_id: String,
    capture_id: String,
    ramble_context_id: String,
    file_name: String,
    expected_revision: u64,
    clipboard: tauri::State<'_, ClipboardCaptureState>,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ApplicationFeedbackWorkspaceView, ApplicationError> {
    let contents = clipboard
        .take_image(&capture_id, &request_id, &ramble_context_id)
        .map_err(ApplicationError::invalid_argument)?;
    if contents.len() > MAX_ATTACHMENT_BYTES {
        return Err(ApplicationError::invalid_argument(format!(
            "attachment exceeds the {} MiB limit",
            MAX_ATTACHMENT_BYTES / 1024 / 1024
        )));
    }
    let safe_name = if file_name.starts_with("ramble-clipboard-") && file_name.ends_with(".png") {
        file_name
    } else {
        format!("ramble-clipboard-{capture_id}.png")
    };
    diagnostics::record_event(
        "clipboard_image_imported",
        Some(&request_id),
        None,
        Some("ok"),
        None,
        None,
    );
    let application = state.application.clone();
    application
        .add_feedback_attachment(AddAttachmentInput {
            request_id,
            file_name: safe_name,
            contents,
            expected_revision,
        })
        .await
        .map(Into::into)
}

#[tauri::command]
pub(super) async fn import_feedback_attachment_path(
    request_id: String,
    path: PathBuf,
    expected_revision: u64,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ApplicationFeedbackWorkspaceView, ApplicationError> {
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
        .map(Into::into)
}

#[tauri::command]
pub(super) async fn remove_feedback_attachment(
    input: RemoveAttachmentInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ApplicationFeedbackWorkspaceView, ApplicationError> {
    state
        .application_commands
        .remove_feedback_attachment(input)
        .await
}

#[tauri::command]
pub(super) async fn reorder_feedback_attachments(
    input: ReorderAttachmentsInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ApplicationFeedbackWorkspaceView, ApplicationError> {
    state
        .application_commands
        .reorder_feedback_attachments(input)
        .await
}

#[tauri::command]
pub(super) async fn read_feedback_attachment(
    input: ReadAttachmentInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Response, ApplicationError> {
    state
        .application_commands
        .read_feedback_attachment(input)
        .await
        .map(Response::new)
}

#[tauri::command]
pub(super) async fn read_request_attachment(
    input: ReadAttachmentInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Response, ApplicationError> {
    state
        .application_commands
        .read_request_attachment(input)
        .await
        .map(Response::new)
}

#[tauri::command]
pub(super) async fn submit_feedback(
    input: SubmitFeedbackInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ApplicationFeedbackRequestView, ApplicationError> {
    state.application_commands.submit_feedback(input).await
}

#[tauri::command]
pub(super) async fn approve_feedback_request(
    input: ApproveFeedbackInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ApplicationFeedbackRequestView, ApplicationError> {
    state
        .application_commands
        .approve_feedback_request(input)
        .await
}

#[tauri::command]
pub(super) async fn cancel_feedback_request(
    input: CancelFeedbackInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ApplicationFeedbackRequestView, ApplicationError> {
    state
        .application_commands
        .cancel_feedback_request(input)
        .await
}

#[tauri::command]
pub(super) fn list_speech_models(state: tauri::State<'_, WorkbenchState>) -> Vec<SpeechModelInfo> {
    list_models(&state.library_root())
}

#[tauri::command]
pub(super) async fn download_speech_model(
    model_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<SpeechModelInfo, String> {
    let root = state.library_root();
    let event_app = app.clone();
    let progress_model_id = model_id.clone();
    let download_model_id = model_id.clone();
    tokio::task::spawn_blocking(move || {
        download_model(&root, &download_model_id, &|downloaded, total| {
            let _ = event_app.emit(
                "speech-model-progress",
                SpeechModelProgress {
                    model_id: progress_model_id.clone(),
                    downloaded,
                    total,
                },
            );
        })
    })
    .await
    .map_err(|error| format!("模型下载任务异常退出：{error}"))??;
    model_info(&state.library_root(), &model_id)
}

#[tauri::command]
pub(super) async fn delete_speech_model(
    model_id: String,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<SpeechModelInfo, String> {
    if state.speech_session.lock().await.is_some() {
        return Err("请先停止语音录入，再删除模型".to_owned());
    }
    let root = state.library_root();
    let delete_model_id = model_id.clone();
    tokio::task::spawn_blocking(move || delete_model(&root, &delete_model_id))
        .await
        .map_err(|error| format!("模型删除任务异常退出：{error}"))??;
    model_info(&state.library_root(), &model_id)
}

#[tauri::command]
pub(super) async fn list_speech_input_devices() -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(list_input_devices)
        .await
        .map_err(|error| format!("麦克风枚举任务异常退出：{error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) async fn start_voice_ramble(
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

    let library_root = state.library_root();
    let model = model_info(&library_root, &input.model_id)?;
    if !model.installed {
        return Err(format!(
            "语音模型 {} 尚未安装，请先在语音设置中下载",
            model.display_name
        ));
    }
    tracing::info!(
        request_id = %input.request_id,
        model_id = %input.model_id,
        "start_voice_ramble: starting"
    );
    let voice_session_id = uuid::Uuid::now_v7().to_string();
    let provider =
        SpeechProvider::from_model_id(&input.model_id).map_err(|error| error.to_string())?;
    let model_path = model_dir(&library_root, &input.model_id)?;
    let vad_model_path = ensure_vad_model(&library_root).map_err(|error| error.to_string())?;
    let config = SpeechSessionConfig {
        request_id: input.request_id,
        voice_session_id: voice_session_id.clone(),
        provider,
        model_path: model_path.clone(),
        vad_model_path,
        vad_threshold: input.vad_threshold,
        vad_silence_ms: input.vad_silence_ms,
        input_device: input.input_device,
        hotwords: input.hotwords,
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
        voice_session_id,
        provider: provider.id().to_owned(),
        model_path: model_path.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub(super) async fn stop_voice_ramble(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<(), String> {
    let session = state.speech_session.lock().await.take();
    let Some(session) = session else {
        return Ok(());
    };
    tokio::task::spawn_blocking(move || session.stop())
        .await
        .map_err(|error| format!("语音识别停止任务异常退出：{error}"))?
        .map_err(|error| error.to_string())
}
