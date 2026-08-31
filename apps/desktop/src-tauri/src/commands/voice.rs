use std::sync::Arc;

use rambledesk_core::{FeedbackStatus, kernel::FeedbackRequestStatus as V3FeedbackStatus};
use rambledesk_speech::{
    SpeechEvent, SpeechEventSink, SpeechProvider, SpeechSession, SpeechSessionConfig,
    ensure_vad_model, list_input_devices,
    model::{SpeechModelInfo, delete_model, download_model, list_models, model_dir, model_info},
};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::{AcpWorkbenchState, WorkbenchState};

#[derive(Debug, Deserialize)]
pub(crate) struct StartVoiceRambleInput {
    request_id: String,
    origin: WorkbenchRequestOrigin,
    input_device: Option<String>,
    model_id: String,
    vad_threshold: f32,
    vad_silence_ms: u32,
    hotwords: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WorkbenchRequestOrigin {
    Adapter,
    ManagedAcp,
}

#[derive(Debug, Serialize)]
pub(crate) struct VoiceRambleSessionView {
    voice_session_id: String,
    provider: String,
    model_path: String,
}

#[derive(Debug, Clone, Serialize)]
struct SpeechModelProgress {
    model_id: String,
    downloaded: u64,
    total: u64,
}

#[tauri::command]
pub(crate) fn list_speech_models(state: tauri::State<'_, WorkbenchState>) -> Vec<SpeechModelInfo> {
    list_models(&state.library_root())
}

#[tauri::command]
pub(crate) async fn download_speech_model(
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
pub(crate) async fn delete_speech_model(
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
pub(crate) async fn list_speech_input_devices() -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(list_input_devices)
        .await
        .map_err(|error| format!("麦克风枚举任务异常退出：{error}"))?
        .map_err(|error| error.to_string())
}

async fn validate_voice_ramble_request<V3Lookup, V3Future, AdapterLookup, AdapterFuture>(
    origin: WorkbenchRequestOrigin,
    v3_lookup: V3Lookup,
    adapter_lookup: AdapterLookup,
) -> Result<(), String>
where
    V3Lookup: FnOnce() -> V3Future,
    V3Future: std::future::Future<Output = Result<Option<V3FeedbackStatus>, String>>,
    AdapterLookup: FnOnce() -> AdapterFuture,
    AdapterFuture: std::future::Future<Output = Result<FeedbackStatus, String>>,
{
    match origin {
        WorkbenchRequestOrigin::ManagedAcp => {
            let Some(status) = v3_lookup().await? else {
                return Err("找不到由 ACP 管理的反馈请求".to_owned());
            };
            if matches!(
                status,
                V3FeedbackStatus::Submitted | V3FeedbackStatus::Cancelled
            ) {
                return Err("已结束的反馈请求不能继续录入语音".to_owned());
            }
        }
        WorkbenchRequestOrigin::Adapter => {
            let status = adapter_lookup().await?;
            if matches!(
                status,
                FeedbackStatus::Completed | FeedbackStatus::Cancelled
            ) {
                return Err("已结束的反馈请求不能继续录入语音".to_owned());
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn start_voice_ramble(
    input: StartVoiceRambleInput,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
    acp_state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<VoiceRambleSessionView, String> {
    let v3_request_id = input.request_id.clone();
    let adapter_request_id = input.request_id.clone();
    let adapter_application = state.application.clone();
    validate_voice_ramble_request(
        input.origin,
        || async move {
            acp_state
                .voice_feedback_status(&v3_request_id)
                .await
                .map_err(|error| error.message)
        },
        || async move {
            adapter_application
                .get_feedback_workspace(adapter_request_id)
                .await
                .map(|workspace| workspace.request.status)
                .map_err(|error| error.to_string())
        },
    )
    .await?;

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
pub(crate) async fn stop_voice_ramble(
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

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;

    #[tokio::test]
    async fn managed_acp_feedback_uses_only_the_v3_owner() {
        let adapter_called = Arc::new(AtomicBool::new(false));
        let called = adapter_called.clone();
        let result = validate_voice_ramble_request(
            WorkbenchRequestOrigin::ManagedAcp,
            || async { Ok(Some(V3FeedbackStatus::Waiting)) },
            || async move {
                called.store(true, Ordering::SeqCst);
                Err("feedback request was not found".to_owned())
            },
        )
        .await;

        assert_eq!(result, Ok(()));
        assert!(!adapter_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn adapter_feedback_uses_only_the_adapter_owner() {
        let v3_called = Arc::new(AtomicBool::new(false));
        let called = v3_called.clone();
        let result = validate_voice_ramble_request(
            WorkbenchRequestOrigin::Adapter,
            || async move {
                called.store(true, Ordering::SeqCst);
                Ok(None)
            },
            || async { Ok(FeedbackStatus::Waiting) },
        )
        .await;

        assert_eq!(result, Ok(()));
        assert!(!v3_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn terminal_managed_acp_feedback_is_rejected_without_consulting_the_adapter() {
        let adapter_called = Arc::new(AtomicBool::new(false));
        let called = adapter_called.clone();
        let result = validate_voice_ramble_request(
            WorkbenchRequestOrigin::ManagedAcp,
            || async { Ok(Some(V3FeedbackStatus::Submitted)) },
            || async move {
                called.store(true, Ordering::SeqCst);
                Ok(FeedbackStatus::Waiting)
            },
        )
        .await;

        assert_eq!(result, Err("已结束的反馈请求不能继续录入语音".to_owned()));
        assert!(!adapter_called.load(Ordering::SeqCst));
    }
}
