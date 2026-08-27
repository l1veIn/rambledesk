use super::*;

#[tauri::command]
pub async fn pin_screen_capture(
    input: CompleteCaptureInput,
    app: AppHandle,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<PinnedCaptureInfo, String> {
    let (png, image, restore, monitor) = {
        let session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        let Some(CaptureSession::Editing {
            monitor,
            restore,
            ..
        }) = session.as_ref()
        else {
            return Err("没有可固定的截图".to_owned());
        };
        let monitor = monitor.clone();
        let restore = *restore;
        let (png, image) = completed_capture_image(session.as_ref().expect("editing"), &input)?;
        (png, image, restore, monitor)
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
    restore_capture_windows(&app, restore);
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
