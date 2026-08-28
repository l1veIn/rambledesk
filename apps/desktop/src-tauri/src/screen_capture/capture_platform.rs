use super::*;

#[cfg(target_os = "macos")]
pub(super) fn capture_monitor(
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
pub(super) fn capture_monitor(
    monitor: &xcap::Monitor,
    _excluded_window_ids: &[u32],
) -> Result<RgbaImage, String> {
    monitor
        .capture_image()
        .map_err(|error| format!("无法截取显示器画面：{error}"))
}

#[cfg(target_os = "macos")]
pub(super) fn ensure_screen_capture_permission() -> Result<(), String> {
    crate::macos_permissions::require_screen_capture_access()
}

#[cfg(target_os = "macos")]
fn native_window_id(window: &tauri::WebviewWindow) -> Option<u32> {
    use objc2::runtime::AnyObject;

    let ns_window = window.ns_window().ok()? as *mut AnyObject;
    Some(unsafe { objc2::msg_send![ns_window, windowNumber] })
}

#[cfg(target_os = "macos")]
pub(super) fn excluded_capture_window_ids(app: &AppHandle) -> Vec<u32> {
    [RAMBLE_CONSOLE_LABEL, OVERLAY_LABEL, SCROLL_LABEL]
        .into_iter()
        .filter_map(|label| app.get_webview_window(label))
        .filter_map(|window| native_window_id(&window))
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub(super) fn excluded_capture_window_ids(_app: &AppHandle) -> Vec<u32> {
    Vec::new()
}

#[cfg(not(target_os = "macos"))]
pub(super) fn ensure_screen_capture_permission() -> Result<(), String> {
    Ok(())
}

#[derive(Debug, Clone)]
pub(super) struct WindowSnapshot {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) app_name: String,
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) opaque: bool,
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
pub(super) fn window_snapshots() -> Vec<WindowSnapshot> {
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
pub(super) fn window_snapshots() -> Vec<WindowSnapshot> {
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
