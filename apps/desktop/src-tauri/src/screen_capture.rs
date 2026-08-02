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
#[cfg(not(target_os = "macos"))]
use std::time::Duration;
use std::{borrow::Cow, collections::HashMap, sync::Mutex};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
    WindowEvent, ipc::Response, window::Color,
};

pub const SCREEN_CAPTURE_SHORTCUT: &str = "Ctrl+Shift+1";
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
    png_base64: String,
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

enum CaptureSession {
    Capturing {
        capture_session_id: String,
        restore_console: bool,
    },
    Editing {
        capture_session_id: String,
        image: RgbaImage,
        monitor: MonitorWindow,
        targets: Vec<CaptureTarget>,
        restore_console: bool,
        suggested_selection: Option<CaptureRectangle>,
    },
    Scrolling {
        capture_session_id: String,
        monitor: MonitorWindow,
        selection: CaptureRectangle,
        composite: RgbaImage,
        last_frame: RgbaImage,
        frame_count: usize,
        restore_console: bool,
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

    fn restore_console(&self) -> bool {
        match self {
            Self::Capturing {
                restore_console, ..
            }
            | Self::Editing {
                restore_console, ..
            }
            | Self::Scrolling {
                restore_console, ..
            } => *restore_console,
            Self::Ready { .. } => false,
        }
    }
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

fn hide_console(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(RAMBLE_CONSOLE_LABEL) {
        let _ = window.hide();
    }
}

fn restore_console(app: &AppHandle, should_restore: bool) {
    if should_restore && let Some(window) = app.get_webview_window(RAMBLE_CONSOLE_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn normalized_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn physical_monitor_size(monitor: &xcap::Monitor) -> Option<(u32, u32)> {
    let width = monitor.width().ok()?;
    let height = monitor.height().ok()?;
    #[cfg(target_os = "windows")]
    return Some((width, height));
    #[cfg(not(target_os = "windows"))]
    {
        let scale = monitor.scale_factor().unwrap_or(1.0).max(1.0);
        Some((
            (width as f32 * scale).round() as u32,
            (height as f32 * scale).round() as u32,
        ))
    }
}

fn choose_monitor(app: &AppHandle) -> Result<(xcap::Monitor, MonitorWindow), String> {
    let cursor = app
        .cursor_position()
        .map_err(|error| format!("无法读取鼠标位置：{error}"))?;
    let target = app
        .monitor_from_point(cursor.x, cursor.y)
        .map_err(|error| format!("无法定位鼠标所在显示器：{error}"))?
        .or_else(|| app.primary_monitor().ok().flatten())
        .ok_or_else(|| "未检测到显示器".to_owned())?;
    let monitors = xcap::Monitor::all().map_err(|error| format!("无法获取显示器列表：{error}"))?;
    if monitors.is_empty() {
        return Err("未检测到显示器".to_owned());
    }

    let target_name = target.name().map(|name| normalized_name(name));
    let target_size = (target.size().width, target.size().height);
    let by_name = target_name.as_ref().and_then(|name| {
        monitors.iter().find(|monitor| {
            monitor.name().ok().into_iter().any(|candidate| {
                let candidate = normalized_name(&candidate);
                !candidate.is_empty()
                    && (candidate == *name || candidate.contains(name) || name.contains(&candidate))
            })
        })
    });
    let by_size = monitors.iter().find(|monitor| {
        physical_monitor_size(monitor).is_some_and(|size| {
            size.0.abs_diff(target_size.0) <= 2 && size.1.abs_diff(target_size.1) <= 2
        })
    });
    let selected = by_name
        .or(by_size)
        .or_else(|| {
            if target.position().x == 0 && target.position().y == 0 {
                monitors
                    .iter()
                    .find(|monitor| monitor.is_primary().unwrap_or(false))
            } else {
                None
            }
        })
        .unwrap_or(&monitors[0])
        .clone();

    let selected_id = selected
        .id()
        .map_err(|error| format!("无法读取显示器标识：{error}"))?;
    let selected_x = selected
        .x()
        .map_err(|error| format!("无法读取显示器位置：{error}"))?;
    let selected_y = selected
        .y()
        .map_err(|error| format!("无法读取显示器位置：{error}"))?;
    let selected_width = selected
        .width()
        .map_err(|error| format!("无法读取显示器尺寸：{error}"))?;
    let selected_height = selected
        .height()
        .map_err(|error| format!("无法读取显示器尺寸：{error}"))?;
    let descriptor = MonitorWindow {
        monitor_id: selected_id,
        monitor_x: selected_x,
        monitor_y: selected_y,
        monitor_width: selected_width,
        monitor_height: selected_height,
        window_x: target.position().x,
        window_y: target.position().y,
        window_width: target.size().width,
        window_height: target.size().height,
        capture_width: 1,
        capture_height: 1,
        regions: vec![MonitorRegion {
            monitor_id: selected_id,
            x: selected_x,
            y: selected_y,
            width: selected_width,
            height: selected_height,
        }],
    };
    Ok((selected, descriptor))
}

fn choose_capture_monitors(app: &AppHandle) -> Result<(Vec<xcap::Monitor>, MonitorWindow), String> {
    let (active_monitor, single_descriptor) = choose_monitor(app)?;
    let monitors = xcap::Monitor::all().map_err(|error| format!("无法获取显示器列表：{error}"))?;
    if monitors.len() <= 1 {
        return Ok((vec![active_monitor], single_descriptor));
    }

    #[cfg(target_os = "macos")]
    {
        let first_scale = monitors[0].scale_factor().unwrap_or(1.0);
        if monitors.iter().any(|monitor| {
            (monitor.scale_factor().unwrap_or(first_scale) - first_scale).abs() > 0.01
        }) {
            return Ok((vec![active_monitor], single_descriptor));
        }
    }

    let tauri_monitors = app
        .available_monitors()
        .map_err(|error| format!("无法获取系统显示器布局：{error}"))?;
    if tauri_monitors.len() != monitors.len() {
        return Ok((vec![active_monitor], single_descriptor));
    }

    let mut used_tauri_monitors = Vec::<usize>::new();
    let mut physical_rectangles = Vec::with_capacity(monitors.len());
    for monitor in &monitors {
        let monitor_name = monitor.name().ok().map(|name| normalized_name(&name));
        let monitor_size = physical_monitor_size(monitor);
        let match_index = tauri_monitors
            .iter()
            .enumerate()
            .filter(|(index, _)| !used_tauri_monitors.contains(index))
            .find(|(_, candidate)| {
                let name_matches = monitor_name.as_ref().is_some_and(|name| {
                    candidate.name().is_some_and(|candidate_name| {
                        let candidate_name = normalized_name(candidate_name);
                        !candidate_name.is_empty()
                            && (candidate_name == *name
                                || candidate_name.contains(name)
                                || name.contains(&candidate_name))
                    })
                });
                let size_matches = monitor_size.is_some_and(|size| {
                    size.0.abs_diff(candidate.size().width) <= 2
                        && size.1.abs_diff(candidate.size().height) <= 2
                });
                name_matches || size_matches
            })
            .map(|(index, _)| index);
        let Some(match_index) = match_index else {
            return Ok((vec![active_monitor], single_descriptor));
        };
        used_tauri_monitors.push(match_index);
        let candidate = &tauri_monitors[match_index];
        physical_rectangles.push((
            candidate.position().x,
            candidate.position().y,
            candidate.size().width,
            candidate.size().height,
        ));
    }

    let logical_min_x = monitors
        .iter()
        .filter_map(|monitor| monitor.x().ok())
        .min()
        .unwrap_or(single_descriptor.monitor_x);
    let logical_min_y = monitors
        .iter()
        .filter_map(|monitor| monitor.y().ok())
        .min()
        .unwrap_or(single_descriptor.monitor_y);
    let logical_max_x = monitors
        .iter()
        .filter_map(|monitor| Some(monitor.x().ok()? + monitor.width().ok()? as i32))
        .max()
        .unwrap_or(single_descriptor.monitor_x + single_descriptor.monitor_width as i32);
    let logical_max_y = monitors
        .iter()
        .filter_map(|monitor| Some(monitor.y().ok()? + monitor.height().ok()? as i32))
        .max()
        .unwrap_or(single_descriptor.monitor_y + single_descriptor.monitor_height as i32);
    let physical_min_x = physical_rectangles
        .iter()
        .map(|rectangle| rectangle.0)
        .min()
        .unwrap_or(single_descriptor.window_x);
    let physical_min_y = physical_rectangles
        .iter()
        .map(|rectangle| rectangle.1)
        .min()
        .unwrap_or(single_descriptor.window_y);
    let physical_max_x = physical_rectangles
        .iter()
        .map(|rectangle| rectangle.0 + rectangle.2 as i32)
        .max()
        .unwrap_or(single_descriptor.window_x + single_descriptor.window_width as i32);
    let physical_max_y = physical_rectangles
        .iter()
        .map(|rectangle| rectangle.1 + rectangle.3 as i32)
        .max()
        .unwrap_or(single_descriptor.window_y + single_descriptor.window_height as i32);

    let regions = monitors
        .iter()
        .filter_map(|monitor| {
            Some(MonitorRegion {
                monitor_id: monitor.id().ok()?,
                x: monitor.x().ok()?,
                y: monitor.y().ok()?,
                width: monitor.width().ok()?,
                height: monitor.height().ok()?,
            })
        })
        .collect();
    Ok((
        monitors,
        MonitorWindow {
            monitor_id: active_monitor
                .id()
                .map_err(|error| format!("无法读取显示器标识：{error}"))?,
            monitor_x: logical_min_x,
            monitor_y: logical_min_y,
            monitor_width: (logical_max_x - logical_min_x).max(1) as u32,
            monitor_height: (logical_max_y - logical_min_y).max(1) as u32,
            window_x: physical_min_x,
            window_y: physical_min_y,
            window_width: (physical_max_x - physical_min_x).max(1) as u32,
            window_height: (physical_max_y - physical_min_y).max(1) as u32,
            capture_width: 1,
            capture_height: 1,
            regions,
        },
    ))
}

fn monitor_by_id(monitor_id: u32) -> Result<xcap::Monitor, String> {
    xcap::Monitor::all()
        .map_err(|error| format!("无法获取显示器列表：{error}"))?
        .into_iter()
        .find(|monitor| monitor.id().ok() == Some(monitor_id))
        .ok_or_else(|| "截图期间显示器配置发生变化，请重新截图".to_owned())
}

fn capture_monitors(
    monitors: &[xcap::Monitor],
    descriptor: &MonitorWindow,
    excluded_window_ids: &[u32],
) -> Result<RgbaImage, String> {
    if monitors.len() == 1 {
        return capture_monitor(&monitors[0], excluded_window_ids);
    }

    let captured = monitors
        .par_iter()
        .map(|monitor| {
            let image = capture_monitor(monitor, excluded_window_ids)?;
            let x = monitor
                .x()
                .map_err(|error| format!("无法读取显示器位置：{error}"))?;
            let y = monitor
                .y()
                .map_err(|error| format!("无法读取显示器位置：{error}"))?;
            let width = monitor
                .width()
                .map_err(|error| format!("无法读取显示器尺寸：{error}"))?;
            let height = monitor
                .height()
                .map_err(|error| format!("无法读取显示器尺寸：{error}"))?;
            Ok::<_, String>((image, x, y, width, height))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let scale_x = captured[0].0.width() as f64 / captured[0].3.max(1) as f64;
    let scale_y = captured[0].0.height() as f64 / captured[0].4.max(1) as f64;
    let image_width = (descriptor.monitor_width as f64 * scale_x).round().max(1.0) as u32;
    let image_height = (descriptor.monitor_height as f64 * scale_y)
        .round()
        .max(1.0) as u32;
    let mut composite = RgbaImage::new(image_width, image_height);
    for (image, x, y, _, _) in captured {
        let offset_x = ((x - descriptor.monitor_x) as f64 * scale_x).round() as i64;
        let offset_y = ((y - descriptor.monitor_y) as f64 * scale_y).round() as i64;
        imageops::replace(&mut composite, &image, offset_x, offset_y);
    }
    Ok(composite)
}

#[cfg(target_os = "macos")]
fn capture_monitor(
    monitor: &xcap::Monitor,
    excluded_window_ids: &[u32],
) -> Result<RgbaImage, String> {
    let monitor_id = monitor
        .id()
        .map_err(|error| format!("无法读取显示器标识：{error}"))?;
    let width = monitor
        .width()
        .map_err(|error| format!("无法读取显示器宽度：{error}"))?;
    let height = monitor
        .height()
        .map_err(|error| format!("无法读取显示器高度：{error}"))?;
    let target = scap::Target::Display(scap::Display {
        id: monitor_id,
        title: monitor.name().unwrap_or_default(),
        raw_handle: core_graphics_helmer_fork::display::CGDisplay::new(monitor_id),
    });
    let excluded_targets = (!excluded_window_ids.is_empty()).then(|| {
        excluded_window_ids
            .iter()
            .map(|window_id| {
                scap::Target::Window(scap::Window {
                    id: *window_id,
                    title: "RambleDesk capture surface".to_owned(),
                    raw_handle: *window_id,
                })
            })
            .collect()
    });
    let options = scap::capturer::Options {
        fps: 1,
        target: Some(target),
        show_cursor: false,
        show_highlight: true,
        excluded_targets,
        output_type: scap::frame::FrameType::BGRAFrame,
        output_resolution: scap::capturer::Resolution::Captured,
        crop_area: Some(scap::capturer::Area {
            origin: scap::capturer::Point { x: 0.0, y: 0.0 },
            size: scap::capturer::Size {
                width: width as f64,
                height: height as f64,
            },
        }),
    };
    let mut capturer = scap::capturer::Capturer::build(options)
        .map_err(|error| format!("无法启动 macOS ScreenCaptureKit：{error}"))?;
    capturer.start_capture();
    let frame = capturer
        .get_next_frame()
        .map_err(|error| format!("macOS 未能返回截图画面：{error}"))?;
    capturer.stop_capture();
    let scap::frame::Frame::BGRA(frame) = frame else {
        return Err("macOS 截图返回了不支持的像素格式".to_owned());
    };
    let frame_width = u32::try_from(frame.width).map_err(|_| "截图宽度无效".to_owned())?;
    let frame_height = u32::try_from(frame.height).map_err(|_| "截图高度无效".to_owned())?;
    let mut rgba = frame.data;
    rgba.par_chunks_exact_mut(4)
        .for_each(|pixel| pixel.swap(0, 2));
    RgbaImage::from_raw(frame_width, frame_height, rgba)
        .ok_or_else(|| "macOS 截图像素长度与画面尺寸不一致".to_owned())
}

#[cfg(not(target_os = "macos"))]
fn capture_monitor(
    monitor: &xcap::Monitor,
    _excluded_window_ids: &[u32],
) -> Result<RgbaImage, String> {
    monitor
        .capture_image()
        .map_err(|error| format!("无法截取显示器画面：{error}"))
}

#[cfg(target_os = "macos")]
fn ensure_screen_capture_permission() -> Result<(), String> {
    if scap::has_permission() {
        return Ok(());
    }
    let _ = scap::request_permission();
    if scap::has_permission() {
        return Ok(());
    }
    Err(
        "RambleDesk 需要“屏幕与系统音频录制”权限。请在系统设置 → 隐私与安全性中允许 RambleDesk，然后重新启动应用再截图。"
            .to_owned(),
    )
}

#[cfg(target_os = "macos")]
fn native_window_id(window: &tauri::WebviewWindow) -> Option<u32> {
    use objc2::runtime::AnyObject;

    let ns_window = window.ns_window().ok()? as *mut AnyObject;
    Some(unsafe { objc2::msg_send![ns_window, windowNumber] })
}

#[cfg(target_os = "macos")]
fn excluded_capture_window_ids(app: &AppHandle) -> Vec<u32> {
    [RAMBLE_CONSOLE_LABEL, OVERLAY_LABEL, SCROLL_LABEL]
        .into_iter()
        .filter_map(|label| app.get_webview_window(label))
        .filter_map(|window| native_window_id(&window))
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn excluded_capture_window_ids(_app: &AppHandle) -> Vec<u32> {
    Vec::new()
}

#[cfg(not(target_os = "macos"))]
fn ensure_screen_capture_permission() -> Result<(), String> {
    Ok(())
}

#[derive(Debug, Clone)]
struct WindowSnapshot {
    id: String,
    title: String,
    app_name: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    opaque: bool,
}

#[cfg(target_os = "macos")]
fn mac_dictionary_value(dictionary: &CFDictionary, key: &str) -> Option<*const c_void> {
    let key = CFString::from_str(key);
    let value = unsafe { dictionary.value((key.as_ref() as *const CFString).cast()) };
    (!value.is_null()).then_some(value)
}

#[cfg(target_os = "macos")]
fn mac_number(dictionary: &CFDictionary, key: &str) -> Option<f64> {
    let number = mac_dictionary_value(dictionary, key)? as *const CFNumber;
    let mut value = 0.0_f64;
    let success =
        unsafe { (*number).value(CFNumberType::DoubleType, (&mut value as *mut f64).cast()) };
    success.then_some(value)
}

#[cfg(target_os = "macos")]
fn mac_string(dictionary: &CFDictionary, key: &str) -> Option<String> {
    let value = mac_dictionary_value(dictionary, key)? as *const CFString;
    Some(unsafe { (*value).to_string() })
}

#[cfg(target_os = "macos")]
fn mac_bounds(dictionary: &CFDictionary) -> Option<CGRect> {
    let bounds = mac_dictionary_value(dictionary, "kCGWindowBounds")? as *const CFDictionary;
    let mut rectangle = CGRect::default();
    unsafe { CGRectMakeWithDictionaryRepresentation(Some(&*bounds), &mut rectangle) }
        .then_some(rectangle)
}

#[cfg(target_os = "macos")]
fn window_snapshots() -> Vec<WindowSnapshot> {
    let Some(windows) = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements,
        0,
    ) else {
        return Vec::new();
    };
    let mut snapshots = Vec::with_capacity(windows.count() as usize);
    for index in 0..windows.count() {
        let dictionary = unsafe { windows.value_at_index(index) } as *const CFDictionary;
        if dictionary.is_null() {
            continue;
        }
        let dictionary = unsafe { &*dictionary };
        if mac_number(dictionary, "kCGWindowLayer").unwrap_or(1.0) != 0.0 {
            continue;
        }
        let Some(bounds) = mac_bounds(dictionary) else {
            continue;
        };
        let width = bounds.size.width.max(0.0).round() as u32;
        let height = bounds.size.height.max(0.0).round() as u32;
        if width == 0 || height == 0 {
            continue;
        }
        let app_name = mac_string(dictionary, "kCGWindowOwnerName").unwrap_or_default();
        if app_name == "Window Server" {
            continue;
        }
        snapshots.push(WindowSnapshot {
            id: mac_number(dictionary, "kCGWindowNumber")
                .map(|value| (value as u32).to_string())
                .unwrap_or_else(|| format!("mac-window-{index}")),
            title: mac_string(dictionary, "kCGWindowName").unwrap_or_default(),
            app_name,
            x: bounds.origin.x.round() as i32,
            y: bounds.origin.y.round() as i32,
            width,
            height,
            opaque: mac_number(dictionary, "kCGWindowAlpha").unwrap_or(1.0) >= 0.98,
        });
    }
    snapshots
}

#[cfg(not(target_os = "macos"))]
fn window_snapshots() -> Vec<WindowSnapshot> {
    let Ok(windows) = xcap::Window::all() else {
        return Vec::new();
    };
    windows
        .into_iter()
        .filter(|window| !window.is_minimized().unwrap_or(false))
        .filter_map(|window| {
            Some(WindowSnapshot {
                id: window.id().ok()?.to_string(),
                title: window.title().unwrap_or_default(),
                app_name: window.app_name().unwrap_or_default(),
                x: window.x().ok()?,
                y: window.y().ok()?,
                width: window.width().ok()?,
                height: window.height().ok()?,
                opaque: true,
            })
        })
        .collect()
}

fn subtract_rectangle(source: CaptureRectangle, cover: CaptureRectangle) -> Vec<CaptureRectangle> {
    let left = source.x.max(cover.x);
    let top = source.y.max(cover.y);
    let right = (source.x + source.width).min(cover.x + cover.width);
    let bottom = (source.y + source.height).min(cover.y + cover.height);
    if left >= right || top >= bottom {
        return vec![source];
    }

    let mut remaining = Vec::with_capacity(4);
    if source.y < top {
        remaining.push(CaptureRectangle {
            x: source.x,
            y: source.y,
            width: source.width,
            height: top - source.y,
        });
    }
    let source_bottom = source.y + source.height;
    if bottom < source_bottom {
        remaining.push(CaptureRectangle {
            x: source.x,
            y: bottom,
            width: source.width,
            height: source_bottom - bottom,
        });
    }
    if source.x < left {
        remaining.push(CaptureRectangle {
            x: source.x,
            y: top,
            width: left - source.x,
            height: bottom - top,
        });
    }
    let source_right = source.x + source.width;
    if right < source_right {
        remaining.push(CaptureRectangle {
            x: right,
            y: top,
            width: source_right - right,
            height: bottom - top,
        });
    }
    remaining
}

fn visible_rectangle_area(rectangle: CaptureRectangle, occluders: &[CaptureRectangle]) -> u64 {
    let mut fragments = vec![rectangle];
    for occluder in occluders {
        fragments = fragments
            .into_iter()
            .flat_map(|fragment| subtract_rectangle(fragment, *occluder))
            .collect();
        if fragments.is_empty() {
            return 0;
        }
    }
    fragments
        .iter()
        .map(|fragment| u64::from(fragment.width) * u64::from(fragment.height))
        .sum()
}

fn is_meaningfully_visible(rectangle: CaptureRectangle, occluders: &[CaptureRectangle]) -> bool {
    let visible_area = visible_rectangle_area(rectangle, occluders);
    let total_area = u64::from(rectangle.width) * u64::from(rectangle.height);
    visible_area >= 48 * 36 && visible_area.saturating_mul(100) >= total_area * 8
}

fn collect_window_targets(
    monitor: &MonitorWindow,
    image: &RgbaImage,
    snapshots: Vec<WindowSnapshot>,
) -> Vec<CaptureTarget> {
    let scale_x = image.width() as f64 / monitor.monitor_width.max(1) as f64;
    let scale_y = image.height() as f64 / monitor.monitor_height.max(1) as f64;
    let mut targets = Vec::new();
    let mut occluders = Vec::new();

    for window in snapshots {
        let left = ((window.x - monitor.monitor_x) as f64 * scale_x).round() as i64;
        let top = ((window.y - monitor.monitor_y) as f64 * scale_y).round() as i64;
        let right = left + (window.width as f64 * scale_x).round() as i64;
        let bottom = top + (window.height as f64 * scale_y).round() as i64;
        let clipped_left = left.clamp(0, image.width() as i64);
        let clipped_top = top.clamp(0, image.height() as i64);
        let clipped_right = right.clamp(0, image.width() as i64);
        let clipped_bottom = bottom.clamp(0, image.height() as i64);
        if clipped_right <= clipped_left || clipped_bottom <= clipped_top {
            continue;
        }
        let rectangle = CaptureRectangle {
            x: clipped_left as u32,
            y: clipped_top as u32,
            width: (clipped_right - clipped_left) as u32,
            height: (clipped_bottom - clipped_top) as u32,
        };
        if is_meaningfully_visible(rectangle, &occluders)
            && rectangle.width >= 48
            && rectangle.height >= 36
            && (!window.title.is_empty() || !window.app_name.is_empty())
        {
            targets.push(CaptureTarget {
                id: window.id,
                title: window.title,
                app_name: window.app_name,
                x: rectangle.x,
                y: rectangle.y,
                width: rectangle.width,
                height: rectangle.height,
            });
        }
        if window.opaque {
            occluders.push(rectangle);
        }
    }
    targets
}

pub fn prepare_screen_capture_overlay(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(OVERLAY_LABEL).is_some() {
        return Ok(());
    }
    let overlay = WebviewWindowBuilder::new(
        app,
        OVERLAY_LABEL,
        WebviewUrl::App("index.html#capture".into()),
    )
    .title("RambleDesk 高级截图")
    .decorations(false)
    .resizable(false)
    .closable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .background_color(Color(0, 0, 0, 255))
    .inner_size(1.0, 1.0)
    .visible(false)
    .build()
    .map_err(|error| format!("无法创建截图编辑窗口：{error}"))?;

    let overlay_to_hide = overlay.clone();
    overlay.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = overlay_to_hide.hide();
        }
    });
    Ok(())
}

fn configure_capture_overlay(app: &AppHandle, monitor: &MonitorWindow) -> Result<(), String> {
    prepare_screen_capture_overlay(app)?;
    let overlay = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or_else(|| "截图编辑窗口尚未准备完成".to_owned())?;
    let _ = overlay.hide();
    overlay
        .set_position(PhysicalPosition::new(monitor.window_x, monitor.window_y))
        .and_then(|_| {
            overlay.set_size(PhysicalSize::new(
                monitor.window_width,
                monitor.window_height,
            ))
        })
        .map_err(|error| format!("无法准备截图编辑窗口：{error}"))
}

#[tauri::command]
pub fn show_screen_capture_overlay(app: AppHandle) -> Result<(), String> {
    let overlay = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or_else(|| "截图编辑窗口已经关闭".to_owned())?;
    overlay
        .show()
        .and_then(|_| overlay.set_focus())
        .map_err(|error| format!("无法显示截图编辑窗口：{error}"))
}

#[tauri::command]
pub async fn begin_screen_capture(
    app: AppHandle,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<(), String> {
    ensure_screen_capture_permission()?;
    let should_restore_console = console_was_visible(&app);
    let capture_session_id = uuid::Uuid::now_v7().to_string();
    {
        let mut session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        if session.is_some() || app.get_webview_window(SCROLL_LABEL).is_some() {
            return Err("已有截图正在进行，按 Esc 可取消".to_owned());
        }
        *session = Some(CaptureSession::Capturing {
            capture_session_id: capture_session_id.clone(),
            restore_console: should_restore_console,
        });
    }

    let result = async {
        let (monitors, mut descriptor) = choose_capture_monitors(&app)?;
        configure_capture_overlay(&app, &descriptor)?;
        let excluded_window_ids = excluded_capture_window_ids(&app);
        hide_console(&app);
        #[cfg(not(target_os = "macos"))]
        tokio::time::sleep(Duration::from_millis(90)).await;
        let (image, snapshots) = rayon::join(
            || capture_monitors(&monitors, &descriptor, &excluded_window_ids),
            window_snapshots,
        );
        let image = image?;
        descriptor.capture_width = image.width();
        descriptor.capture_height = image.height();
        let targets = collect_window_targets(&descriptor, &image, snapshots);
        {
            let mut session = state
                .session
                .lock()
                .map_err(|_| "截图状态锁已损坏".to_owned())?;
            match session.as_ref() {
                Some(CaptureSession::Capturing {
                    capture_session_id: active_id,
                    ..
                }) if active_id == &capture_session_id => {}
                _ => return Err("截图会话已变化，请重新截图".to_owned()),
            }
            *session = Some(CaptureSession::Editing {
                capture_session_id: capture_session_id.clone(),
                image,
                monitor: descriptor.clone(),
                targets,
                restore_console: should_restore_console,
                suggested_selection: None,
            });
        }
        app.emit_to(
            OVERLAY_LABEL,
            "screen-capture-session-ready",
            ScreenCaptureSessionReady {
                capture_session_id: capture_session_id.clone(),
            },
        )
        .map_err(|error| format!("无法唤醒截图编辑窗口：{error}"))
    }
    .await;

    if let Err(error) = result {
        if let Ok(mut session) = state.session.lock()
            && session
                .as_ref()
                .is_some_and(|active| active.capture_session_id() == capture_session_id)
        {
            *session = None;
        }
        restore_console(&app, should_restore_console);
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub fn get_active_capture_info(
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<ActiveCaptureInfo, String> {
    let session = state
        .session
        .lock()
        .map_err(|_| "截图状态锁已损坏".to_owned())?;
    match session.as_ref() {
        Some(CaptureSession::Editing {
            capture_session_id,
            image,
            targets,
            suggested_selection,
            ..
        }) => Ok(ActiveCaptureInfo {
            capture_session_id: capture_session_id.clone(),
            image_width: image.width(),
            image_height: image.height(),
            targets: targets.clone(),
            suggested_selection: *suggested_selection,
        }),
        Some(CaptureSession::Scrolling { .. }) => Err("滚动截图正在采集中".to_owned()),
        Some(CaptureSession::Capturing { .. }) => Err("屏幕画面仍在读取，请稍候".to_owned()),
        Some(CaptureSession::Ready { .. }) => Err("截图已经完成，正在写入文档".to_owned()),
        None => Err("没有活动的截图会话".to_owned()),
    }
}

#[tauri::command]
pub fn read_capture_rgba_bytes(
    capture_session_id: String,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<Response, String> {
    let session = state
        .session
        .lock()
        .map_err(|_| "截图状态锁已损坏".to_owned())?;
    match session.as_ref() {
        Some(CaptureSession::Editing {
            capture_session_id: active_id,
            image,
            ..
        }) if active_id == &capture_session_id => Ok(Response::new(image.as_raw().clone())),
        Some(_) => Err("截图会话已变化，请重新截图".to_owned()),
        None => Err("没有活动的截图会话".to_owned()),
    }
}

#[tauri::command]
pub async fn complete_screen_capture(
    input: CompleteCaptureInput,
    app: AppHandle,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<(), String> {
    let (png, image) = decode_canonical_png(&input.png_base64)?;
    if input.copy_to_clipboard {
        copy_image_to_clipboard(&image)?;
    }

    let restore = {
        let mut session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        let Some(active) = session.as_ref() else {
            return Err("没有活动的截图会话".to_owned());
        };
        if active.capture_session_id() != input.capture_session_id {
            return Err("截图会话已变化，请重新截图".to_owned());
        }
        let restore = active.restore_console();
        *session = Some(CaptureSession::Ready {
            capture_session_id: input.capture_session_id.clone(),
            png,
        });
        restore
    };

    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.hide();
    }
    restore_console(&app, restore);
    app.emit_to(
        "main",
        "screen-capture-ready",
        ScreenCaptureReady {
            capture_session_id: input.capture_session_id.clone(),
            file_name: format!("ramble-screenshot-{}.png", input.capture_session_id),
        },
    )
    .map_err(|error| format!("无法通知文档插入截图：{error}"))
}

#[tauri::command]
pub async fn pin_screen_capture(
    input: CompleteCaptureInput,
    app: AppHandle,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<PinnedCaptureInfo, String> {
    let (png, image) = decode_canonical_png(&input.png_base64)?;
    let (restore, monitor) = {
        let session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        let Some(CaptureSession::Editing {
            capture_session_id,
            monitor,
            restore_console,
            ..
        }) = session.as_ref()
        else {
            return Err("没有可固定的截图".to_owned());
        };
        if capture_session_id != &input.capture_session_id {
            return Err("截图会话已变化，请重新截图".to_owned());
        }
        (*restore_console, monitor.clone())
    };
    if input.copy_to_clipboard {
        copy_image_to_clipboard(&image)?;
    }

    let pin_id = uuid::Uuid::now_v7().to_string();
    state
        .pinned
        .lock()
        .map_err(|_| "固定截图状态锁已损坏".to_owned())?
        .insert(pin_id.clone(), png);
    if let Err(error) = create_pin_window(&app, &pin_id, image.width(), image.height(), &monitor) {
        if let Ok(mut pinned) = state.pinned.lock() {
            pinned.remove(&pin_id);
        }
        return Err(error);
    }

    let completed_session = {
        let mut session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        if session
            .as_ref()
            .is_some_and(|active| active.capture_session_id() == input.capture_session_id)
        {
            session.take();
            true
        } else {
            false
        }
    };
    if !completed_session {
        if let Ok(mut pinned) = state.pinned.lock() {
            pinned.remove(&pin_id);
        }
        if let Some(window) = app.get_webview_window(&format!("capture-pin-{pin_id}")) {
            let _ = window.close();
        }
        return Err("截图会话已变化，请重新截图".to_owned());
    }
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.hide();
    }
    restore_console(&app, restore);
    let _ = app.emit_to(
        "main",
        "screen-capture-finished",
        ScreenCaptureFinished {
            capture_session_id: Some(input.capture_session_id),
            outcome: "pinned",
        },
    );
    Ok(PinnedCaptureInfo {
        pin_id,
        width: image.width(),
        height: image.height(),
    })
}

fn create_pin_window(
    app: &AppHandle,
    pin_id: &str,
    image_width: u32,
    image_height: u32,
    monitor: &MonitorWindow,
) -> Result<(), String> {
    let max_width = (monitor.window_width as f64 * 0.55).max(240.0);
    let max_height = (monitor.window_height as f64 * 0.65).max(160.0);
    let scale = (max_width / image_width.max(1) as f64)
        .min(max_height / image_height.max(1) as f64)
        .min(1.0);
    let width = (image_width as f64 * scale).max(160.0).round() as u32;
    let height = (image_height as f64 * scale).max(100.0).round() as u32;
    let label = format!("capture-pin-{pin_id}");
    let window = WebviewWindowBuilder::new(
        app,
        &label,
        WebviewUrl::App(format!("index.html#capture-pin={pin_id}").into()),
    )
    .title("RambleDesk 固定截图")
    .decorations(false)
    .resizable(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(true)
    .visible_on_all_workspaces(true)
    .visible(false)
    .build()
    .map_err(|error| format!("无法创建固定截图窗口：{error}"))?;
    let x = monitor.window_x + (monitor.window_width.saturating_sub(width) / 2) as i32;
    let y = monitor.window_y + (monitor.window_height.saturating_sub(height) / 2) as i32;
    window
        .set_position(PhysicalPosition::new(x, y))
        .and_then(|_| window.set_size(PhysicalSize::new(width, height)))
        .and_then(|_| window.show())
        .and_then(|_| window.set_focus())
        .map_err(|error| format!("无法显示固定截图窗口：{error}"))
}

#[tauri::command]
pub fn read_pinned_screen_capture(
    pin_id: String,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<Response, String> {
    state
        .pinned
        .lock()
        .map_err(|_| "固定截图状态锁已损坏".to_owned())?
        .get(&pin_id)
        .cloned()
        .map(Response::new)
        .ok_or_else(|| "固定截图已关闭".to_owned())
}

#[tauri::command]
pub fn close_pinned_screen_capture(
    pin_id: String,
    app: AppHandle,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<(), String> {
    state
        .pinned
        .lock()
        .map_err(|_| "固定截图状态锁已损坏".to_owned())?
        .remove(&pin_id);
    if let Some(window) = app.get_webview_window(&format!("capture-pin-{pin_id}")) {
        let _ = window.close();
    }
    Ok(())
}

fn scrolling_monitor_selection(
    monitor: &MonitorWindow,
    image_width: u32,
    image_height: u32,
    selection: CaptureRectangle,
) -> Result<(u32, CaptureRectangle), String> {
    let scale_x = image_width as f64 / monitor.monitor_width.max(1) as f64;
    let scale_y = image_height as f64 / monitor.monitor_height.max(1) as f64;
    for region in &monitor.regions {
        let x = ((region.x - monitor.monitor_x) as f64 * scale_x).round() as i64;
        let y = ((region.y - monitor.monitor_y) as f64 * scale_y).round() as i64;
        let width = (region.width as f64 * scale_x).round().max(1.0) as u32;
        let height = (region.height as f64 * scale_y).round().max(1.0) as u32;
        let right = x + width as i64;
        let bottom = y + height as i64;
        let selection_right = selection.x as i64 + selection.width as i64;
        let selection_bottom = selection.y as i64 + selection.height as i64;
        if selection.x as i64 >= x
            && selection.y as i64 >= y
            && selection_right <= right
            && selection_bottom <= bottom
        {
            return Ok((
                region.monitor_id,
                CaptureRectangle {
                    x: (selection.x as i64 - x) as u32,
                    y: (selection.y as i64 - y) as u32,
                    width: selection.width,
                    height: selection.height,
                },
            ));
        }
    }
    Err("滚动截图区域不能跨越多个显示器，请在单个显示器内框选".to_owned())
}

#[tauri::command]
pub async fn begin_scrolling_capture(
    input: BeginScrollingInput,
    app: AppHandle,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<ScrollCaptureInfo, String> {
    let (monitor, selection, first_frame, should_restore_console) = {
        let mut session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        let active = session
            .take()
            .ok_or_else(|| "没有活动的截图会话".to_owned())?;
        let CaptureSession::Editing {
            capture_session_id,
            image,
            monitor,
            targets,
            restore_console,
            suggested_selection,
        } = active
        else {
            *session = Some(active);
            return Err("当前截图不能进入滚动采集".to_owned());
        };
        if capture_session_id != input.capture_session_id {
            *session = Some(CaptureSession::Editing {
                capture_session_id,
                image,
                monitor,
                targets,
                restore_console,
                suggested_selection,
            });
            return Err("截图会话已变化，请重新截图".to_owned());
        }
        let selection = match validated_selection(input.selection, image.width(), image.height()) {
            Ok(selection) => selection,
            Err(error) => {
                *session = Some(CaptureSession::Editing {
                    capture_session_id,
                    image,
                    monitor,
                    targets,
                    restore_console,
                    suggested_selection,
                });
                return Err(error);
            }
        };
        let (scroll_monitor_id, local_selection) =
            match scrolling_monitor_selection(&monitor, image.width(), image.height(), selection) {
                Ok(result) => result,
                Err(error) => {
                    *session = Some(CaptureSession::Editing {
                        capture_session_id,
                        image,
                        monitor,
                        targets,
                        restore_console,
                        suggested_selection,
                    });
                    return Err(error);
                }
            };
        let first_frame = imageops::crop_imm(
            &image,
            selection.x,
            selection.y,
            selection.width,
            selection.height,
        )
        .to_image();
        let mut scroll_monitor = monitor.clone();
        scroll_monitor.monitor_id = scroll_monitor_id;
        *session = Some(CaptureSession::Scrolling {
            capture_session_id: input.capture_session_id.clone(),
            monitor: scroll_monitor,
            selection: local_selection,
            composite: first_frame.clone(),
            last_frame: first_frame.clone(),
            frame_count: 1,
            restore_console,
        });
        (monitor, selection, first_frame, restore_console)
    };

    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.hide();
    }
    if let Err(error) = create_scroll_window(&app, &monitor, selection) {
        if let Ok(mut session) = state.session.lock() {
            *session = None;
        }
        restore_console(&app, should_restore_console);
        return Err(error);
    }
    Ok(ScrollCaptureInfo {
        capture_session_id: input.capture_session_id,
        frame_count: 1,
        width: first_frame.width(),
        height: first_frame.height(),
        added_height: first_frame.height(),
        matched: true,
    })
}

fn create_scroll_window(
    app: &AppHandle,
    monitor: &MonitorWindow,
    selection: CaptureRectangle,
) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window(SCROLL_LABEL) {
        let _ = existing.close();
    }
    let selection_right = (selection.x as f64 / monitor.capture_width.max(1) as f64
        * monitor.window_width as f64) as i32;
    let selection_bottom = ((selection.y + selection.height) as f64
        / monitor.capture_height.max(1) as f64
        * monitor.window_height as f64) as i32;
    let window = WebviewWindowBuilder::new(
        app,
        SCROLL_LABEL,
        WebviewUrl::App("index.html#capture-scroll".into()),
    )
    .title("RambleDesk 滚动截图")
    .decorations(false)
    .resizable(false)
    .closable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(true)
    .inner_size(340.0, 104.0)
    .visible(false)
    .build()
    .map_err(|error| format!("无法创建滚动截图控制器：{error}"))?;
    let controller_size = window
        .outer_size()
        .map_err(|error| format!("无法读取滚动截图控制器尺寸：{error}"))?;
    let mut x = monitor.window_x + selection_right + 12;
    let mut y = monitor.window_y + selection_bottom + 12;
    if x + controller_size.width as i32 > monitor.window_x + monitor.window_width as i32 {
        x = monitor.window_x + monitor.window_width as i32 - controller_size.width as i32 - 18;
    }
    if y + controller_size.height as i32 > monitor.window_y + monitor.window_height as i32 {
        y = monitor.window_y + 18;
    }
    window
        .set_position(PhysicalPosition::new(x, y))
        .and_then(|_| window.show())
        .and_then(|_| window.set_focus())
        .map_err(|error| format!("无法显示滚动截图控制器：{error}"))
}

#[tauri::command]
pub fn get_scrolling_capture_info(
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<ScrollCaptureInfo, String> {
    let session = state
        .session
        .lock()
        .map_err(|_| "截图状态锁已损坏".to_owned())?;
    match session.as_ref() {
        Some(CaptureSession::Scrolling {
            capture_session_id,
            frame_count,
            composite,
            ..
        }) => Ok(ScrollCaptureInfo {
            capture_session_id: capture_session_id.clone(),
            frame_count: *frame_count,
            width: composite.width(),
            height: composite.height(),
            added_height: 0,
            matched: true,
        }),
        _ => Err("没有活动的滚动截图".to_owned()),
    }
}

#[tauri::command]
pub async fn append_scrolling_capture_frame(
    capture_session_id: String,
    app: AppHandle,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<ScrollCaptureInfo, String> {
    let (monitor_id, selection) = {
        let session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        match session.as_ref() {
            Some(CaptureSession::Scrolling {
                capture_session_id: active_id,
                monitor,
                selection,
                ..
            }) if active_id == &capture_session_id => (monitor.monitor_id, *selection),
            Some(_) => return Err("滚动截图会话已变化".to_owned()),
            None => return Err("没有活动的滚动截图".to_owned()),
        }
    };

    let excluded_window_ids = excluded_capture_window_ids(&app);
    let capture_result = (|| {
        let monitor = monitor_by_id(monitor_id)?;
        let image = capture_monitor(&monitor, &excluded_window_ids)?;
        let selection = validated_selection(selection, image.width(), image.height())?;
        Ok::<RgbaImage, String>(
            imageops::crop_imm(
                &image,
                selection.x,
                selection.y,
                selection.width,
                selection.height,
            )
            .to_image(),
        )
    })();
    let frame = capture_result?;

    let mut session = state
        .session
        .lock()
        .map_err(|_| "截图状态锁已损坏".to_owned())?;
    let Some(CaptureSession::Scrolling {
        capture_session_id: active_id,
        composite,
        last_frame,
        frame_count,
        ..
    }) = session.as_mut()
    else {
        return Err("没有活动的滚动截图".to_owned());
    };
    if active_id != &capture_session_id {
        return Err("滚动截图会话已变化".to_owned());
    }
    let stitch = stitch_vertical(composite, last_frame, &frame)?;
    if let Some(next) = stitch.image {
        *composite = next;
        *last_frame = frame;
        *frame_count += 1;
    }
    Ok(ScrollCaptureInfo {
        capture_session_id,
        frame_count: *frame_count,
        width: composite.width(),
        height: composite.height(),
        added_height: stitch.added_height,
        matched: stitch.matched,
    })
}

#[tauri::command]
pub async fn finish_scrolling_capture(
    capture_session_id: String,
    app: AppHandle,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<(), String> {
    let monitor = {
        let mut session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        match session.as_ref() {
            Some(CaptureSession::Scrolling {
                capture_session_id: active_id,
                ..
            }) if active_id == &capture_session_id => {}
            Some(_) => return Err("滚动截图会话已变化".to_owned()),
            None => return Err("没有活动的滚动截图".to_owned()),
        }
        let active = session.take().expect("scrolling session was checked above");
        let CaptureSession::Scrolling {
            mut monitor,
            composite,
            restore_console,
            ..
        } = active
        else {
            *session = Some(active);
            return Err("当前截图不在滚动采集阶段".to_owned());
        };
        let full = CaptureRectangle {
            x: 0,
            y: 0,
            width: composite.width(),
            height: composite.height(),
        };
        monitor.capture_width = composite.width();
        monitor.capture_height = composite.height();
        *session = Some(CaptureSession::Editing {
            capture_session_id: capture_session_id.clone(),
            image: composite,
            monitor: monitor.clone(),
            targets: Vec::new(),
            restore_console,
            suggested_selection: Some(full),
        });
        monitor
    };
    if let Some(window) = app.get_webview_window(SCROLL_LABEL) {
        let _ = window.close();
    }
    configure_capture_overlay(&app, &monitor)?;
    app.emit_to(
        OVERLAY_LABEL,
        "screen-capture-session-ready",
        ScreenCaptureSessionReady { capture_session_id },
    )
    .map_err(|error| format!("无法重新打开截图编辑窗口：{error}"))
}

struct StitchResult {
    image: Option<RgbaImage>,
    added_height: u32,
    matched: bool,
}

fn stitch_vertical(
    composite: &RgbaImage,
    previous: &RgbaImage,
    next: &RgbaImage,
) -> Result<StitchResult, String> {
    if previous.dimensions() != next.dimensions() || composite.width() != next.width() {
        return Err("滚动截图区域尺寸发生变化，请保持目标窗口大小不变".to_owned());
    }
    let width = next.width();
    let height = next.height();
    if width < 8 || height < 16 {
        return Err("滚动截图区域过小".to_owned());
    }
    let duplicate_step = ((width.min(height) / 100).max(2)) as usize;
    let mut duplicate_total = 0_u64;
    let mut duplicate_samples = 0_u64;
    for y in (0..height).step_by(duplicate_step) {
        for x in (0..width).step_by(duplicate_step) {
            let first = previous.get_pixel(x, y).0;
            let second = next.get_pixel(x, y).0;
            duplicate_total += first[0].abs_diff(second[0]) as u64
                + first[1].abs_diff(second[1]) as u64
                + first[2].abs_diff(second[2]) as u64;
            duplicate_samples += 3;
        }
    }
    if duplicate_samples > 0 && duplicate_total as f64 / (duplicate_samples as f64) < 2.0 {
        return Ok(StitchResult {
            image: None,
            added_height: 0,
            matched: true,
        });
    }
    let min_overlap = (height / 8).max(8);
    let max_overlap = (height * 19 / 20).max(min_overlap);
    let sample_step = ((width.min(height) / 90).max(2)) as usize;
    let mut best: Option<(u32, f64)> = None;

    for overlap in (min_overlap..=max_overlap).step_by(2) {
        let mut total = 0_u64;
        let mut samples = 0_u64;
        let start_x = width / 12;
        let end_x = width.saturating_sub(width / 12);
        for offset_y in (0..overlap).step_by(sample_step) {
            let previous_y = height - overlap + offset_y;
            let next_y = offset_y;
            for x in (start_x..end_x).step_by(sample_step) {
                let first = previous.get_pixel(x, previous_y).0;
                let second = next.get_pixel(x, next_y).0;
                let difference = first[0].abs_diff(second[0]) as u64
                    + first[1].abs_diff(second[1]) as u64
                    + first[2].abs_diff(second[2]) as u64;
                total += difference.min(150);
                samples += 3;
            }
        }
        if samples == 0 {
            continue;
        }
        let score = total as f64 / samples as f64;
        if best.is_none_or(|(_, current)| score < current) {
            best = Some((overlap, score));
        }
    }

    let Some((overlap, score)) = best else {
        return Ok(StitchResult {
            image: None,
            added_height: 0,
            matched: false,
        });
    };
    if score > 27.0 {
        return Ok(StitchResult {
            image: None,
            added_height: 0,
            matched: false,
        });
    }
    let added_height = height.saturating_sub(overlap);
    if added_height <= 2 && score < 4.0 {
        return Ok(StitchResult {
            image: None,
            added_height: 0,
            matched: true,
        });
    }
    let output_height = composite.height().saturating_add(added_height);
    if output_height > MAX_SCROLL_HEIGHT {
        return Err("滚动截图已达到 60000 像素高度上限".to_owned());
    }
    let mut output = RgbaImage::new(width, output_height);
    imageops::overlay(&mut output, composite, 0, 0);
    let tail = imageops::crop_imm(next, 0, overlap, width, added_height).to_image();
    imageops::overlay(&mut output, &tail, 0, composite.height() as i64);
    Ok(StitchResult {
        image: Some(output),
        added_height,
        matched: true,
    })
}

fn validated_selection(
    selection: CaptureRectangle,
    image_width: u32,
    image_height: u32,
) -> Result<CaptureRectangle, String> {
    let x = selection.x.min(image_width);
    let y = selection.y.min(image_height);
    let width = selection.width.min(image_width.saturating_sub(x));
    let height = selection.height.min(image_height.saturating_sub(y));
    if width < MIN_SELECTION_SIZE || height < MIN_SELECTION_SIZE {
        return Err("截图区域过小，请重新选择".to_owned());
    }
    Ok(CaptureRectangle {
        x,
        y,
        width,
        height,
    })
}

#[tauri::command]
pub fn read_completed_screen_capture(
    capture_session_id: String,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<Response, String> {
    let session = state
        .session
        .lock()
        .map_err(|_| "截图状态锁已损坏".to_owned())?;
    match session.as_ref() {
        Some(CaptureSession::Ready {
            capture_session_id: active_id,
            png,
        }) if active_id == &capture_session_id => Ok(Response::new(png.clone())),
        Some(_) => Err("截图会话已变化，请重新截图".to_owned()),
        None => Err("没有已完成的截图".to_owned()),
    }
}

#[tauri::command]
pub fn discard_screen_capture(
    capture_session_id: String,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<(), String> {
    let mut session = state
        .session
        .lock()
        .map_err(|_| "截图状态锁已损坏".to_owned())?;
    if session
        .as_ref()
        .is_some_and(|active| active.capture_session_id() == capture_session_id)
    {
        *session = None;
    }
    Ok(())
}

#[tauri::command]
pub async fn cancel_screen_capture(
    app: AppHandle,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<(), String> {
    let (capture_session_id, restore) = state
        .session
        .lock()
        .map_err(|_| "截图状态锁已损坏".to_owned())?
        .take()
        .map(|session| {
            (
                Some(session.capture_session_id().to_owned()),
                session.restore_console(),
            )
        })
        .unwrap_or((None, false));
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.hide();
    }
    if let Some(scroll) = app.get_webview_window(SCROLL_LABEL) {
        let _ = scroll.close();
    }
    restore_console(&app, restore);
    app.emit_to(
        "main",
        "screen-capture-finished",
        ScreenCaptureFinished {
            capture_session_id,
            outcome: "cancelled",
        },
    )
    .map_err(|error| format!("无法通知截图取消：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patterned_frame(offset: u32, width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            let absolute_y = y + offset;
            image::Rgba([
                ((x * 17 + absolute_y * 3) % 251) as u8,
                ((x * 7 + absolute_y * 11) % 241) as u8,
                ((x * 13 + absolute_y * 5) % 239) as u8,
                255,
            ])
        })
    }

    #[test]
    fn vertical_stitch_finds_overlap_and_appends_only_new_rows() {
        let first = patterned_frame(0, 120, 100);
        let second = patterned_frame(35, 120, 100);
        let result = stitch_vertical(&first, &first, &second).expect("stitch");
        assert!(result.matched);
        assert!(result.added_height.abs_diff(35) <= 2);
        assert_eq!(
            result.image.expect("stitched image").height(),
            100 + result.added_height
        );
    }

    #[test]
    fn vertical_stitch_skips_duplicate_frames() {
        let frame = patterned_frame(0, 120, 100);
        let result = stitch_vertical(&frame, &frame, &frame).expect("stitch");
        assert!(result.matched);
        assert_eq!(result.added_height, 0);
        assert!(result.image.is_none());
    }

    #[test]
    fn completed_capture_rejects_non_png_payloads() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"not a png");
        assert!(decode_canonical_png(&encoded).is_err());
    }

    #[test]
    fn selection_is_clamped_to_the_source_image() {
        assert_eq!(
            validated_selection(
                CaptureRectangle {
                    x: 90,
                    y: 40,
                    width: 30,
                    height: 30,
                },
                100,
                60,
            )
            .expect("selection")
            .width,
            10
        );
    }

    #[test]
    fn fully_occluded_windows_are_not_meaningfully_visible() {
        let window = CaptureRectangle {
            x: 20,
            y: 20,
            width: 200,
            height: 120,
        };
        assert!(!is_meaningfully_visible(window, &[window]));
    }

    #[test]
    fn partially_visible_windows_remain_selectable() {
        let window = CaptureRectangle {
            x: 0,
            y: 0,
            width: 200,
            height: 120,
        };
        let cover = CaptureRectangle {
            x: 0,
            y: 0,
            width: 100,
            height: 120,
        };
        assert_eq!(visible_rectangle_area(window, &[cover]), 12_000);
        assert!(is_meaningfully_visible(window, &[cover]));
    }

    #[test]
    fn several_front_windows_can_jointly_hide_a_background_window() {
        let window = CaptureRectangle {
            x: 0,
            y: 0,
            width: 200,
            height: 120,
        };
        let covers = [
            CaptureRectangle {
                x: 0,
                y: 0,
                width: 100,
                height: 120,
            },
            CaptureRectangle {
                x: 100,
                y: 0,
                width: 100,
                height: 120,
            },
        ];
        assert_eq!(visible_rectangle_area(window, &covers), 0);
        assert!(!is_meaningfully_visible(window, &covers));
    }
}
