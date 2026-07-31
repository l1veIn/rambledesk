use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Cursor,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, ipc::Response};

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const MAX_TEXT_CHARS: usize = 50_000;
const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

#[derive(Default)]
pub struct ClipboardCaptureState {
    monitor: Mutex<Option<ClipboardMonitor>>,
    images: Arc<Mutex<HashMap<String, PendingImage>>>,
}

struct ClipboardMonitor {
    running: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}

struct PendingImage {
    request_id: String,
    ramble_session_id: String,
    contents: Vec<u8>,
}

#[derive(Debug, Deserialize)]
pub struct StartClipboardCaptureInput {
    request_id: String,
    ramble_session_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClipboardCaptureEvent {
    Text {
        request_id: String,
        ramble_session_id: String,
        text: String,
        captured_at_ms: u64,
        truncated: bool,
    },
    Image {
        request_id: String,
        ramble_session_id: String,
        capture_id: String,
        file_name: String,
        captured_at_ms: u64,
    },
    Warning {
        request_id: String,
        ramble_session_id: String,
        message: String,
    },
}

#[tauri::command]
pub fn capture_clipboard_once(
    input: StartClipboardCaptureInput,
    state: tauri::State<'_, ClipboardCaptureState>,
) -> Result<ClipboardCaptureEvent, String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("无法访问系统剪贴板：{error}"))?;
    let captured_at_ms = unix_time_ms();

    if let Ok(image) = clipboard.get_image() {
        let contents = encode_clipboard_image(image)?;
        if contents.len() > MAX_IMAGE_BYTES {
            return Err("剪贴板图片超过 20 MiB，无法导入".to_owned());
        }
        let capture_id = uuid::Uuid::now_v7().to_string();
        let file_name = format!("ramble-clipboard-{capture_id}.png");
        state
            .images
            .lock()
            .map_err(|_| "剪贴板图片状态锁已损坏".to_owned())?
            .insert(
                capture_id.clone(),
                PendingImage {
                    request_id: input.request_id.clone(),
                    ramble_session_id: input.ramble_session_id.clone(),
                    contents,
                },
            );
        return Ok(ClipboardCaptureEvent::Image {
            request_id: input.request_id,
            ramble_session_id: input.ramble_session_id,
            capture_id,
            file_name,
            captured_at_ms,
        });
    }

    if let Ok(text) = clipboard.get_text() {
        let (text, truncated) = truncate_text(text);
        if !text.trim().is_empty() {
            return Ok(ClipboardCaptureEvent::Text {
                request_id: input.request_id,
                ramble_session_id: input.ramble_session_id,
                text,
                captured_at_ms,
                truncated,
            });
        }
    }

    Err("剪贴板中没有可导入的文字或图片".to_owned())
}

#[tauri::command]
pub async fn start_clipboard_capture(
    input: StartClipboardCaptureInput,
    app: AppHandle,
    state: tauri::State<'_, ClipboardCaptureState>,
) -> Result<(), String> {
    let clipboard =
        arboard::Clipboard::new().map_err(|error| format!("无法访问 Windows 剪贴板：{error}"))?;
    let mut monitor = state
        .monitor
        .lock()
        .map_err(|_| "剪贴板监听状态锁已损坏".to_owned())?;
    if monitor.is_some() {
        return Err("已有 Ramble 正在监听剪贴板".to_owned());
    }

    let running = Arc::new(AtomicBool::new(true));
    let worker_running = Arc::clone(&running);
    let images = Arc::clone(&state.images);
    let request_id = input.request_id;
    let ramble_session_id = input.ramble_session_id;
    let worker = thread::Builder::new()
        .name("rambledesk-clipboard".to_owned())
        .spawn(move || {
            monitor_clipboard(
                app,
                clipboard,
                request_id,
                ramble_session_id,
                worker_running,
                images,
            );
        })
        .map_err(|error| format!("无法启动剪贴板监听线程：{error}"))?;

    *monitor = Some(ClipboardMonitor { running, worker });
    Ok(())
}

#[tauri::command]
pub async fn stop_clipboard_capture(
    state: tauri::State<'_, ClipboardCaptureState>,
) -> Result<(), String> {
    let monitor = state
        .monitor
        .lock()
        .map_err(|_| "剪贴板监听状态锁已损坏".to_owned())?
        .take();
    let Some(monitor) = monitor else {
        return Ok(());
    };
    monitor.running.store(false, Ordering::Release);
    tauri::async_runtime::spawn_blocking(move || monitor.worker.join())
        .await
        .map_err(|error| format!("剪贴板监听停止任务异常退出：{error}"))?
        .map_err(|_| "剪贴板监听线程异常退出".to_owned())
}

#[tauri::command]
pub fn read_clipboard_capture_image(
    capture_id: String,
    request_id: String,
    ramble_session_id: String,
    state: tauri::State<'_, ClipboardCaptureState>,
) -> Result<Response, String> {
    let images = state
        .images
        .lock()
        .map_err(|_| "剪贴板图片状态锁已损坏".to_owned())?;
    let image = images
        .get(&capture_id)
        .ok_or_else(|| "剪贴板图片已过期或不存在".to_owned())?;
    if image.request_id != request_id || image.ramble_session_id != ramble_session_id {
        return Err("剪贴板图片不属于当前 Ramble".to_owned());
    }
    Ok(Response::new(image.contents.clone()))
}

#[tauri::command]
pub fn discard_clipboard_capture_image(
    capture_id: String,
    state: tauri::State<'_, ClipboardCaptureState>,
) -> Result<(), String> {
    state
        .images
        .lock()
        .map_err(|_| "剪贴板图片状态锁已损坏".to_owned())?
        .remove(&capture_id);
    Ok(())
}

fn monitor_clipboard(
    app: AppHandle,
    mut clipboard: arboard::Clipboard,
    request_id: String,
    ramble_session_id: String,
    running: Arc<AtomicBool>,
    images: Arc<Mutex<HashMap<String, PendingImage>>>,
) {
    let mut sequence = clipboard_sequence_number();
    while running.load(Ordering::Acquire) {
        thread::sleep(POLL_INTERVAL);
        let next_sequence = clipboard_sequence_number();
        if next_sequence == 0 || next_sequence == sequence {
            continue;
        }
        sequence = next_sequence;

        if let Ok(image) = clipboard.get_image() {
            match encode_clipboard_image(image) {
                Ok(contents) if contents.len() <= MAX_IMAGE_BYTES => {
                    let capture_id = uuid::Uuid::now_v7().to_string();
                    let file_name = format!("ramble-clipboard-{capture_id}.png");
                    if let Ok(mut pending) = images.lock() {
                        pending.insert(
                            capture_id.clone(),
                            PendingImage {
                                request_id: request_id.clone(),
                                ramble_session_id: ramble_session_id.clone(),
                                contents,
                            },
                        );
                        emit_event(
                            &app,
                            ClipboardCaptureEvent::Image {
                                request_id: request_id.clone(),
                                ramble_session_id: ramble_session_id.clone(),
                                capture_id,
                                file_name,
                                captured_at_ms: unix_time_ms(),
                            },
                        );
                    }
                }
                Ok(_) => emit_warning(
                    &app,
                    &request_id,
                    &ramble_session_id,
                    "剪贴板图片超过 20 MiB，已忽略",
                ),
                Err(error) => emit_warning(
                    &app,
                    &request_id,
                    &ramble_session_id,
                    &format!("剪贴板图片编码失败：{error}"),
                ),
            }
            continue;
        }

        if let Ok(text) = clipboard.get_text() {
            let (text, truncated) = truncate_text(text);
            if !text.trim().is_empty() {
                emit_event(
                    &app,
                    ClipboardCaptureEvent::Text {
                        request_id: request_id.clone(),
                        ramble_session_id: ramble_session_id.clone(),
                        text,
                        captured_at_ms: unix_time_ms(),
                        truncated,
                    },
                );
            }
        }
    }
}

fn encode_clipboard_image(image: arboard::ImageData<'_>) -> Result<Vec<u8>, String> {
    let width = u32::try_from(image.width).map_err(|_| "剪贴板图片宽度无效".to_owned())?;
    let height = u32::try_from(image.height).map_err(|_| "剪贴板图片高度无效".to_owned())?;
    let rgba = image::RgbaImage::from_raw(width, height, image.bytes.into_owned())
        .ok_or_else(|| "剪贴板图片像素数据无效".to_owned())?;
    let mut output = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(output.into_inner())
}

fn truncate_text(text: String) -> (String, bool) {
    let mut chars = text.chars();
    let truncated: String = chars.by_ref().take(MAX_TEXT_CHARS).collect();
    let was_truncated = chars.next().is_some();
    (truncated, was_truncated)
}

fn emit_warning(app: &AppHandle, request_id: &str, ramble_session_id: &str, message: &str) {
    emit_event(
        app,
        ClipboardCaptureEvent::Warning {
            request_id: request_id.to_owned(),
            ramble_session_id: ramble_session_id.to_owned(),
            message: message.to_owned(),
        },
    );
}

fn emit_event(app: &AppHandle, event: ClipboardCaptureEvent) {
    if let Err(error) = app.emit_to("main", "clipboard-capture-event", event) {
        tracing::warn!(%error, "failed to emit clipboard capture event");
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(target_os = "windows")]
fn clipboard_sequence_number() -> u32 {
    unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() }
}

#[cfg(not(target_os = "windows"))]
fn clipboard_sequence_number() -> u32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_limit_preserves_utf8_boundaries() {
        let source = "中".repeat(MAX_TEXT_CHARS + 1);
        let (text, truncated) = truncate_text(source);
        assert_eq!(text.chars().count(), MAX_TEXT_CHARS);
        assert!(truncated);
    }

    #[test]
    fn text_under_limit_is_unchanged() {
        let source = "copied context".to_owned();
        assert_eq!(truncate_text(source.clone()), (source, false));
    }
}
