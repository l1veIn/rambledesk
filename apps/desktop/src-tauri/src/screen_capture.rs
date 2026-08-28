use base64::Engine;
use image::{RgbaImage, imageops};
#[cfg(target_os = "macos")]
use objc2_core_foundation::{CFDictionary, CFNumber, CFNumberType, CFString, CGRect};
#[cfg(target_os = "macos")]
use objc2_core_graphics::{
    CGRectMakeWithDictionaryRepresentation, CGWindowListCopyWindowInfo, CGWindowListOption,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use std::ffi::c_void;
use std::{borrow::Cow, collections::HashMap, sync::Mutex};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
    WindowEvent, ipc::Response, window::Color,
};

const OVERLAY_LABEL: &str = "capture-overlay";
const SCROLL_LABEL: &str = "capture-scroll";
const RAMBLE_CONSOLE_LABEL: &str = "ramble-console";
const MIN_SELECTION_SIZE: u32 = 8;
const MAX_RESULT_BYTES: usize = 128 * 1024 * 1024;
const MAX_SCROLL_HEIGHT: u32 = 60_000;

#[derive(Default)]
pub struct ScreenCaptureState {
    session: Mutex<Option<CaptureSession>>,
    pinned: Mutex<HashMap<String, Vec<u8>>>,
}

impl ScreenCaptureState {
    pub fn take_completed_png(&self, capture_session_id: &str) -> Result<Vec<u8>, String> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        match session.take() {
            Some(CaptureSession::Ready {
                capture_session_id: active_id,
                png,
            }) if active_id == capture_session_id => Ok(png),
            other => {
                let message = if other.is_none() {
                    "没有已完成的截图"
                } else {
                    "截图会话已变化，请重新截图"
                };
                *session = other;
                Err(message.to_owned())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ScreenCaptureReady {
    capture_session_id: String,
    file_name: String,
}

#[derive(Debug, Clone, Serialize)]
struct ScreenCaptureFinished {
    capture_session_id: Option<String>,
    outcome: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct ScreenCaptureSessionReady {
    capture_session_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CaptureTarget {
    id: String,
    title: String,
    app_name: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(super) struct CaptureRectangle {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize)]
pub(super) struct ActiveCaptureInfo {
    capture_session_id: String,
    image_width: u32,
    image_height: u32,
    targets: Vec<CaptureTarget>,
    suggested_selection: Option<CaptureRectangle>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CompleteCaptureInput {
    capture_session_id: String,
    selection: CaptureRectangle,
    #[serde(default)]
    png_base64: Option<String>,
    #[serde(default)]
    copy_to_clipboard: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct BeginScrollingInput {
    capture_session_id: String,
    selection: CaptureRectangle,
}

#[derive(Debug, Serialize)]
pub(super) struct ScrollCaptureInfo {
    capture_session_id: String,
    frame_count: usize,
    width: u32,
    height: u32,
    added_height: u32,
    matched: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct PinnedCaptureInfo {
    pin_id: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
struct MonitorWindow {
    monitor_id: u32,
    monitor_x: i32,
    monitor_y: i32,
    monitor_width: u32,
    monitor_height: u32,
    window_x: i32,
    window_y: i32,
    window_width: u32,
    window_height: u32,
    capture_width: u32,
    capture_height: u32,
    regions: Vec<MonitorRegion>,
}

#[derive(Debug, Clone)]
struct MonitorRegion {
    monitor_id: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct RestoreWindows {
    console: bool,
}

enum CaptureSession {
    Capturing {
        capture_session_id: String,
        restore: RestoreWindows,
    },
    Editing {
        capture_session_id: String,
        image: RgbaImage,
        monitor: MonitorWindow,
        targets: Vec<CaptureTarget>,
        restore: RestoreWindows,
        suggested_selection: Option<CaptureRectangle>,
    },
    Scrolling {
        capture_session_id: String,
        monitor: MonitorWindow,
        selection: CaptureRectangle,
        composite: RgbaImage,
        last_frame: RgbaImage,
        frame_count: usize,
        restore: RestoreWindows,
    },
    Ready {
        capture_session_id: String,
        png: Vec<u8>,
    },
}

impl CaptureSession {
    fn capture_session_id(&self) -> &str {
        match self {
            Self::Capturing {
                capture_session_id, ..
            }
            | Self::Editing {
                capture_session_id, ..
            }
            | Self::Scrolling {
                capture_session_id, ..
            }
            | Self::Ready {
                capture_session_id, ..
            } => capture_session_id,
        }
    }

    fn windows_to_restore(&self) -> RestoreWindows {
        match self {
            Self::Capturing { restore, .. }
            | Self::Editing { restore, .. }
            | Self::Scrolling { restore, .. } => *restore,
            Self::Ready { .. } => RestoreWindows::default(),
        }
    }
}

fn crop_selection(image: &RgbaImage, selection: CaptureRectangle) -> Result<RgbaImage, String> {
    let rectangle = lifecycle::validated_selection(selection, image.width(), image.height())?;
    Ok(imageops::crop_imm(
        image,
        rectangle.x,
        rectangle.y,
        rectangle.width,
        rectangle.height,
    )
    .to_image())
}

fn completed_capture_image(
    session: &CaptureSession,
    input: &CompleteCaptureInput,
) -> Result<(Vec<u8>, RgbaImage), String> {
    let CaptureSession::Editing {
        capture_session_id,
        image,
        ..
    } = session
    else {
        return Err("没有可完成的截图会话".to_owned());
    };
    if capture_session_id != &input.capture_session_id {
        return Err("截图会话已变化，请重新截图".to_owned());
    }
    if let Some(encoded) = input
        .png_base64
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        return decode_canonical_png(encoded);
    }
    let cropped = crop_selection(image, input.selection)?;
    let png = encode_png(&cropped)?;
    Ok((png, cropped))
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|error| format!("无法编码截图：{error}"))?;
    Ok(cursor.into_inner())
}

fn decode_canonical_png(value: &str) -> Result<(Vec<u8>, RgbaImage), String> {
    let encoded = value
        .strip_prefix("data:image/png;base64,")
        .unwrap_or(value);
    let estimated_size = encoded.len().saturating_mul(3) / 4;
    if estimated_size > MAX_RESULT_BYTES {
        return Err("标注后的截图超过 128 MiB 限制".to_owned());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| format!("无法解码标注后的截图数据：{error}"))?;
    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|error| format!("标注结果不是有效的 PNG 图片：{error}"))?
        .to_rgba8();
    if image.width() < MIN_SELECTION_SIZE || image.height() < MIN_SELECTION_SIZE {
        return Err("截图区域过小，请重新选择".to_owned());
    }
    let canonical = encode_png(&image)?;
    Ok((canonical, image))
}

fn copy_image_to_clipboard(image: &RgbaImage) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("无法连接系统剪贴板：{error}"))?;
    clipboard
        .set_image(arboard::ImageData {
            width: image.width() as usize,
            height: image.height() as usize,
            bytes: Cow::Owned(image.clone().into_raw()),
        })
        .map_err(|error| format!("无法把截图复制到系统剪贴板：{error}"))
}

fn console_was_visible(app: &AppHandle) -> bool {
    app.get_webview_window(RAMBLE_CONSOLE_LABEL)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

/// Which RambleDesk windows should be hidden while a capture is in flight and
/// then restored afterwards. The main window deliberately stays untouched so a
/// visible window remains visible and a minimized window is never reopened by
/// the capture lifecycle.
fn capture_windows_restore(app: &AppHandle) -> RestoreWindows {
    let console = console_was_visible(app);
    RestoreWindows { console }
}

fn hide_capture_windows(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(RAMBLE_CONSOLE_LABEL) {
        let _ = window.hide();
    }
}

fn restore_capture_windows(app: &AppHandle, restore: RestoreWindows) {
    if restore.console
        && let Some(window) = app.get_webview_window(RAMBLE_CONSOLE_LABEL)
    {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

mod monitor;

mod capture_platform;

mod geometry;

pub(crate) mod overlay;

pub use overlay::prepare_screen_capture_overlay;

pub(crate) mod pin;

pub(crate) mod scroll;

pub(crate) mod lifecycle;

#[cfg(test)]
#[path = "screen_capture/tests.rs"]
mod tests;
