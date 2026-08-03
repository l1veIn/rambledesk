use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};

use rambledesk_core::{
    AddAttachmentInput, ApplicationError, ApproveFeedbackInput, CancelFeedbackInput, DraftView,
    FeedbackPackageContent, FeedbackRequestSummary, FeedbackRequestView, FeedbackStatus,
    FeedbackWorkspaceView, GetFeedbackInput, HostSessionSummary, ListFeedbackRequestsInput,
    ListFeedbackRequestsOutput, MAX_ATTACHMENT_BYTES, RemoveAttachmentInput,
    ReorderAttachmentsInput, SaveDraftInput, SubmitFeedbackInput,
};
use rambledesk_hosts::{HostProfile, known_host_profiles};
use rambledesk_speech::{
    SpeechEvent, SpeechEventSink, SpeechProvider, SpeechSession, SpeechSessionConfig,
    ensure_vad_model, list_input_devices,
    model::{SpeechModelInfo, delete_model, download_model, list_models, model_dir, model_info},
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use super::{
    TRAY_ID, WorkbenchState, continuation::deliver_continuation_after_terminal,
    generic_mcp_install, migrate_library, pending_tray_icon, pi_install, save_library_path,
};

#[derive(Debug, Deserialize)]
pub(super) struct StartVoiceRambleInput {
    request_id: String,
    input_device: Option<String>,
    model_id: String,
    vad_threshold: f32,
    vad_silence_ms: u32,
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
pub(super) fn get_data_storage_settings(
    state: tauri::State<'_, WorkbenchState>,
) -> DataStorageView {
    let active_path = display_path(&state.library_root);
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
    let source = state.library_root.clone();
    let destination = path.clone();
    let event_app = app.clone();
    tokio::task::spawn_blocking(move || {
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
    Ok(DataStorageView {
        active_path: display_path(&state.library_root),
        selected_path: display_path(&selected),
        restart_required: selected != state.library_root,
    })
}

#[tauri::command]
pub(super) fn get_generic_mcp_configuration(state: tauri::State<'_, WorkbenchState>) -> String {
    state.generic_mcp_configuration.clone()
}

#[tauri::command]
pub(super) fn list_host_profiles() -> Vec<HostProfile> {
    known_host_profiles()
}

#[tauri::command]
pub(super) fn detect_generic_mcp_hosts(
    app: tauri::AppHandle,
) -> Result<Vec<generic_mcp_install::McpHostView>, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Could not resolve the user home directory: {error}"))?;
    Ok(generic_mcp_install::detect_hosts(&home))
}

#[tauri::command]
pub(super) fn install_generic_mcp_hosts(
    host_ids: Vec<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<generic_mcp_install::McpInstallResult>, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Could not resolve the user home directory: {error}"))?;
    generic_mcp_install::install_hosts(&home, &host_ids, &state.generic_mcp_configuration)
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
    let pi_bin = pi_install::resolve_pi_binary().ok_or_else(|| {
        "The `pi` CLI was not found on PATH. Install Pi or set RAMBLEDESK_PI_BIN, then run `pi install npm:@rambledesk/pi` manually.".to_owned()
    })?;
    tauri::async_runtime::spawn_blocking(move || pi_install::run_install(&pi_bin, &package_dir))
        .await
        .map_err(|error| format!("Installer task failed: {error}"))?
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
    let application = state.application.clone();
    application.list_open_feedback_requests().await
}

#[tauri::command]
pub(super) async fn list_host_sessions(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Vec<HostSessionSummary>, ApplicationError> {
    state.application.list_host_sessions().await
}

#[tauri::command]
pub(super) async fn list_feedback_requests(
    input: ListFeedbackRequestsInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<ListFeedbackRequestsOutput, ApplicationError> {
    state.application.list_feedback_requests(input).await
}

#[tauri::command]
pub(super) async fn get_feedback_workspace(
    request_id: String,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackWorkspaceView, ApplicationError> {
    let application = state.application.clone();
    application.get_feedback_workspace(request_id).await
}

#[tauri::command]
pub(super) async fn read_published_feedback(
    request_id: String,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<Option<FeedbackPackageContent>, ApplicationError> {
    let application = state.application.clone();
    let request = application
        .get_feedback(GetFeedbackInput { request_id })
        .await?;
    application.read_feedback_package(&request).await
}

#[tauri::command]
pub(super) async fn save_feedback_draft(
    input: SaveDraftInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<DraftView, ApplicationError> {
    let application = state.application.clone();
    application.save_feedback_draft(input).await
}

#[tauri::command]
pub(super) async fn add_feedback_attachment(
    input: AddAttachmentInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackWorkspaceView, ApplicationError> {
    let application = state.application.clone();
    application.add_feedback_attachment(input).await
}

#[tauri::command]
pub(super) async fn import_feedback_attachment_path(
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
pub(super) async fn remove_feedback_attachment(
    input: RemoveAttachmentInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackWorkspaceView, ApplicationError> {
    let application = state.application.clone();
    application.remove_feedback_attachment(input).await
}

#[tauri::command]
pub(super) async fn reorder_feedback_attachments(
    input: ReorderAttachmentsInput,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackWorkspaceView, ApplicationError> {
    let application = state.application.clone();
    application.reorder_feedback_attachments(input).await
}

#[tauri::command]
pub(super) async fn read_feedback_attachment(
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
pub(super) async fn submit_feedback(
    input: SubmitFeedbackInput,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackRequestView, ApplicationError> {
    let application = state.application.clone();
    let result = application.submit_feedback(input.clone()).await?;
    deliver_continuation_after_terminal(
        &app,
        &state.continuation,
        &application,
        &input.request_id,
        result.status,
    )
    .await;
    Ok(result)
}

#[tauri::command]
pub(super) async fn approve_feedback_request(
    input: ApproveFeedbackInput,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackRequestView, ApplicationError> {
    let application = state.application.clone();
    let result = application.approve_feedback(input.clone()).await?;
    deliver_continuation_after_terminal(
        &app,
        &state.continuation,
        &application,
        &input.request_id,
        result.status,
    )
    .await;
    Ok(result)
}

#[tauri::command]
pub(super) async fn cancel_feedback_request(
    input: CancelFeedbackInput,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<FeedbackRequestView, ApplicationError> {
    let application = state.application.clone();
    let result = application.cancel_feedback(input.clone()).await?;
    deliver_continuation_after_terminal(
        &app,
        &state.continuation,
        &application,
        &input.request_id,
        result.status,
    )
    .await;
    Ok(result)
}

#[tauri::command]
pub(super) fn list_speech_models(state: tauri::State<'_, WorkbenchState>) -> Vec<SpeechModelInfo> {
    list_models(&state.library_root)
}

#[tauri::command]
pub(super) async fn download_speech_model(
    model_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<SpeechModelInfo, String> {
    let root = state.library_root.clone();
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
    model_info(&state.library_root, &model_id)
}

#[tauri::command]
pub(super) async fn delete_speech_model(
    model_id: String,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<SpeechModelInfo, String> {
    if state.speech_session.lock().await.is_some() {
        return Err("请先停止语音录入，再删除模型".to_owned());
    }
    let root = state.library_root.clone();
    let delete_model_id = model_id.clone();
    tokio::task::spawn_blocking(move || delete_model(&root, &delete_model_id))
        .await
        .map_err(|error| format!("模型删除任务异常退出：{error}"))??;
    model_info(&state.library_root, &model_id)
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

    let model = model_info(&state.library_root, &input.model_id)?;
    if !model.installed {
        return Err(format!(
            "语音模型 {} 尚未安装，请先在语音设置中下载",
            model.display_name
        ));
    }
    let voice_session_id = uuid::Uuid::now_v7().to_string();
    let provider =
        SpeechProvider::from_model_id(&input.model_id).map_err(|error| error.to_string())?;
    let model_path = model_dir(&state.library_root, &input.model_id)?;
    let vad_model_path =
        ensure_vad_model(&state.library_root).map_err(|error| error.to_string())?;
    let config = SpeechSessionConfig {
        request_id: input.request_id,
        voice_session_id: voice_session_id.clone(),
        provider,
        model_path: model_path.clone(),
        vad_model_path,
        vad_threshold: input.vad_threshold,
        vad_silence_ms: input.vad_silence_ms,
        input_device: input.input_device,
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
