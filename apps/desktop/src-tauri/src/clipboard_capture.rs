use serde::Serialize;
use std::{
    collections::HashMap,
    io::Cursor,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::ipc::Response;

const MAX_TEXT_CHARS: usize = 50_000;
const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

#[derive(Default)]
pub struct ClipboardCaptureState {
    images: Mutex<HashMap<String, PendingImage>>,
}

struct PendingImage {
    contents: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClipboardCaptureEvent {
    Text {
        text: String,
        captured_at_ms: u64,
        truncated: bool,
    },
    Image {
        capture_id: String,
        file_name: String,
        captured_at_ms: u64,
        byte_length: usize,
    },
}

#[tauri::command]
pub fn capture_clipboard_once(
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
        let byte_length = contents.len();
        state
            .images
            .lock()
            .map_err(|_| "剪贴板图片状态锁已损坏".to_owned())?
            .insert(capture_id.clone(), PendingImage { contents });
        return Ok(ClipboardCaptureEvent::Image {
            capture_id,
            file_name,
            captured_at_ms,
            byte_length,
        });
    }

    if let Ok(text) = clipboard.get_text() {
        let (text, truncated) = truncate_text(text);
        if !text.trim().is_empty() {
            return Ok(ClipboardCaptureEvent::Text {
                text,
                captured_at_ms,
                truncated,
            });
        }
    }

    Err("剪贴板中没有可导入的文字或图片".to_owned())
}

#[tauri::command]
pub fn read_clipboard_capture_image(
    capture_id: String,
    state: tauri::State<'_, ClipboardCaptureState>,
) -> Result<Response, String> {
    let images = state
        .images
        .lock()
        .map_err(|_| "剪贴板图片状态锁已损坏".to_owned())?;
    let image = images
        .get(&capture_id)
        .ok_or_else(|| "剪贴板图片已过期或不存在".to_owned())?;
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

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
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

    #[test]
    fn pending_image_contains_only_client_local_bytes() {
        let image = PendingImage {
            contents: vec![9, 8, 7],
        };
        assert_eq!(image.contents, [9, 8, 7]);
    }
}
