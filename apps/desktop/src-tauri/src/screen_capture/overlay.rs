use std::time::Duration;

use super::*;

use super::capture_platform::{
    ensure_screen_capture_permission, excluded_capture_window_ids, window_snapshots,
};
use super::geometry::collect_window_targets;
use super::monitor::{capture_monitors, choose_capture_monitors};

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

pub(super) fn configure_capture_overlay(
    app: &AppHandle,
    monitor: &MonitorWindow,
) -> Result<(), String> {
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

async fn leave_main_fullscreen(app: &AppHandle) {
    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    let fullscreen = main.is_fullscreen().unwrap_or(false);
    if !fullscreen {
        return;
    }
    if let Err(error) = main.set_fullscreen(false) {
        tracing::warn!(%error, "failed to leave fullscreen before screen capture");
        return;
    }
    tracing::info!("left main-window fullscreen before screen capture");
    tokio::time::sleep(Duration::from_millis(500)).await;
}

#[tauri::command]
pub fn show_screen_capture_overlay(app: AppHandle) -> Result<(), String> {
    let overlay = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or_else(|| "截图编辑窗口已经关闭".to_owned())?;
    overlay
        .show()
        .and_then(|_| overlay.set_focus())
        .map_err(|error| format!("无法显示截图编辑窗口：{error}"))?;
    tracing::info!("showed screen capture overlay");
    Ok(())
}

#[tauri::command]
pub async fn begin_screen_capture(
    app: AppHandle,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    tracing::info!("begin_screen_capture: requesting permission");
    ensure_screen_capture_permission()?;
    leave_main_fullscreen(&app).await;
    let restore = capture_windows_restore(&app);
    let capture_session_id = uuid::Uuid::now_v7().to_string();
    tracing::info!(%capture_session_id, "begin_screen_capture: session created");
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
            restore,
        });
    }
    // Hide RambleDesk's own windows before pixels are read: clicking the
    // console raises the app (and its main window) over the user's target on
    // some platforms, which would otherwise block the view and end up inside
    // the captured image.
    hide_capture_windows(&app);

    let result = async {
        let (monitors, mut descriptor) = choose_capture_monitors(&app)?;
        configure_capture_overlay(&app, &descriptor)?;
        let excluded_window_ids = excluded_capture_window_ids(&app);
        #[cfg(not(target_os = "macos"))]
        tokio::time::sleep(Duration::from_millis(90)).await;
        let (image, snapshots) = rayon::join(
            || capture_monitors(&monitors, &descriptor, &excluded_window_ids),
            window_snapshots,
        );
        let image = image?;
        descriptor.capture_width = image.width();
        descriptor.capture_height = image.height();
        tracing::info!(
            width = image.width(),
            height = image.height(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "begin_screen_capture: pixels ready"
        );
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
                restore,
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
        restore_capture_windows(&app, restore);
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
    let started = std::time::Instant::now();
    let annotated = input
        .png_base64
        .as_deref()
        .is_some_and(|value| !value.is_empty());
    tracing::info!(
        capture_session_id = %input.capture_session_id,
        selection_w = input.selection.width,
        selection_h = input.selection.height,
        annotated,
        "complete_screen_capture: start"
    );
    let (png, image, restore) = {
        let session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        let active = session.as_ref().ok_or("没有活动的截图会话")?;
        let restore = active.windows_to_restore();
        let (png, image) = completed_capture_image(active, &input)?;
        (png, image, restore)
    };
    let png_len = png.len();
    if input.copy_to_clipboard {
        tracing::info!(
            png_bytes = png_len,
            "complete_screen_capture: copying clipboard"
        );
        copy_image_to_clipboard(&image)?;
    }
    {
        let mut session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        if session
            .as_ref()
            .is_none_or(|active| active.capture_session_id() != input.capture_session_id)
        {
            return Err("截图会话已变化，请重新截图".to_owned());
        }
        *session = Some(CaptureSession::Ready {
            capture_session_id: input.capture_session_id.clone(),
            png,
        });
    }

    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.hide();
    }
    restore_capture_windows(&app, restore);
    crate::diagnostics::record_event(
        "screen_capture_completed",
        Some(&input.capture_session_id),
        None,
        Some("ok"),
        None,
        None,
    );
    tracing::info!(
        capture_session_id = %input.capture_session_id,
        png_bytes = png_len,
        elapsed_ms = started.elapsed().as_millis() as u64,
        annotated,
        "complete_screen_capture: overlay hidden, notifying main"
    );
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
