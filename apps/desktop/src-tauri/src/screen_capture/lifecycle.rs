use super::*;

pub(super) fn validated_selection(
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
