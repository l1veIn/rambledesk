use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, ipc::Response};
#[cfg(target_os = "windows")]
use tauri::{WebviewUrl, WebviewWindowBuilder};

pub const SCREEN_CAPTURE_SHORTCUT: &str = "Ctrl+Shift+1";
const OVERLAY_LABEL: &str = "capture-overlay";

#[derive(Default)]
pub struct ScreenCaptureState {
    session: Mutex<Option<CaptureSession>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScreenCaptureView {
    pub session_id: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
struct ScreenCaptureReady {
    session_id: String,
    file_name: String,
}

#[derive(Debug, Clone, Serialize)]
struct ScreenCaptureCancelled {
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub struct CompleteScreenCaptureInput {
    session_id: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
enum CaptureSession {
    Selecting {
        session_id: String,
        #[cfg(target_os = "windows")]
        image: image::RgbaImage,
    },
    Ready {
        session_id: String,
        png: Vec<u8>,
    },
}

impl CaptureSession {
    fn session_id(&self) -> &str {
        match self {
            Self::Selecting { session_id, .. } | Self::Ready { session_id, .. } => session_id,
        }
    }
}

#[cfg(target_os = "windows")]
fn encode_png(image: &image::RgbaImage) -> Result<Vec<u8>, String> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|error| format!("无法编码截图：{error}"))?;
    Ok(cursor.into_inner())
}

#[cfg(target_os = "windows")]
fn cursor_position() -> Result<(i32, i32), String> {
    use windows::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};

    let mut point = POINT::default();
    // SAFETY: GetCursorPos writes one POINT to the valid mutable pointer supplied here.
    unsafe { GetCursorPos(&mut point) }.map_err(|error| format!("无法读取鼠标位置：{error}"))?;
    Ok((point.x, point.y))
}

#[tauri::command]
pub async fn begin_screen_capture(
    app: AppHandle,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, state);
        Err("内置区域截图目前只在 Windows 开发环境启用".to_owned())
    }

    #[cfg(target_os = "windows")]
    {
        use xcap::Monitor;

        let mut session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        if session.is_some() || app.get_webview_window(OVERLAY_LABEL).is_some() {
            return Err("已有截图正在进行，按 Esc 可取消".to_owned());
        }

        let (cursor_x, cursor_y) = cursor_position()?;
        let monitor = Monitor::from_point(cursor_x, cursor_y)
            .map_err(|error| format!("无法定位鼠标所在显示器：{error}"))?;
        let monitor_x = monitor
            .x()
            .map_err(|error| format!("无法读取显示器位置：{error}"))?;
        let monitor_y = monitor
            .y()
            .map_err(|error| format!("无法读取显示器位置：{error}"))?;
        let image = monitor
            .capture_image()
            .map_err(|error| format!("无法截取显示器画面：{error}"))?;
        let width = image.width();
        let height = image.height();
        let session_id = uuid::Uuid::now_v7().to_string();
        *session = Some(CaptureSession::Selecting { session_id, image });
        drop(session);

        let overlay = WebviewWindowBuilder::new(
            &app,
            OVERLAY_LABEL,
            WebviewUrl::App("index.html#capture".into()),
        )
        .title("RambleDesk 区域截图")
        .decorations(false)
        .resizable(false)
        .closable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .visible(false)
        .build()
        .map_err(|error| {
            if let Ok(mut session) = state.session.lock() {
                *session = None;
            }
            format!("无法创建截图框选层：{error}")
        })?;

        if let Err(error) = overlay
            .set_position(tauri::PhysicalPosition::new(monitor_x, monitor_y))
            .and_then(|_| overlay.set_size(tauri::PhysicalSize::new(width, height)))
            .and_then(|_| overlay.show())
            .and_then(|_| overlay.set_focus())
        {
            let _ = overlay.close();
            if let Ok(mut session) = state.session.lock() {
                *session = None;
            }
            return Err(format!("无法显示截图框选层：{error}"));
        }
        Ok(())
    }
}

#[tauri::command]
pub fn get_screen_capture_view(
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<ScreenCaptureView, String> {
    let session = state
        .session
        .lock()
        .map_err(|_| "截图状态锁已损坏".to_owned())?;
    match session.as_ref() {
        #[cfg(target_os = "windows")]
        Some(CaptureSession::Selecting { session_id, image }) => Ok(ScreenCaptureView {
            session_id: session_id.clone(),
            width: image.width(),
            height: image.height(),
        }),
        Some(CaptureSession::Ready { .. }) => Err("截图已经完成，正在写入文档".to_owned()),
        None => Err("没有活动的截图会话".to_owned()),
        #[cfg(not(target_os = "windows"))]
        Some(CaptureSession::Selecting { .. }) => Err("当前平台不支持区域截图".to_owned()),
    }
}

#[tauri::command]
pub fn read_screen_capture_preview(
    session_id: String,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<Response, String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (session_id, state);
        Err("当前平台不支持区域截图".to_owned())
    }

    #[cfg(target_os = "windows")]
    {
        let session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        match session.as_ref() {
            Some(CaptureSession::Selecting {
                session_id: active_id,
                image,
            }) if active_id == &session_id => encode_png(image).map(Response::new),
            Some(_) => Err("截图会话已变化，请重新截图".to_owned()),
            None => Err("没有活动的截图会话".to_owned()),
        }
    }
}

#[tauri::command]
pub async fn complete_screen_capture(
    input: CompleteScreenCaptureInput,
    app: AppHandle,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (input, app, state);
        Err("当前平台不支持区域截图".to_owned())
    }

    #[cfg(target_os = "windows")]
    {
        let mut session = state
            .session
            .lock()
            .map_err(|_| "截图状态锁已损坏".to_owned())?;
        let selecting = session
            .take()
            .ok_or_else(|| "没有活动的截图会话".to_owned())?;
        let CaptureSession::Selecting { session_id, image } = selecting else {
            *session = Some(selecting);
            return Err("截图已经完成，正在写入文档".to_owned());
        };
        if session_id != input.session_id {
            *session = Some(CaptureSession::Selecting { session_id, image });
            return Err("截图会话已变化，请重新截图".to_owned());
        }

        let x = input.x.min(image.width());
        let y = input.y.min(image.height());
        let width = input.width.min(image.width().saturating_sub(x));
        let height = input.height.min(image.height().saturating_sub(y));
        if width < 4 || height < 4 {
            *session = Some(CaptureSession::Selecting { session_id, image });
            return Err("截图区域过小，请重新拖动框选".to_owned());
        }
        let cropped = image::imageops::crop_imm(&image, x, y, width, height).to_image();
        let png = encode_png(&cropped)?;
        let file_name = format!("ramble-screenshot-{session_id}.png");
        let ready = ScreenCaptureReady {
            session_id: session_id.clone(),
            file_name: file_name.clone(),
        };
        *session = Some(CaptureSession::Ready { session_id, png });
        drop(session);

        if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
            let _ = overlay.close();
        }
        app.emit_to("main", "screen-capture-ready", ready)
            .map_err(|error| format!("无法通知文档插入截图：{error}"))
    }
}

#[tauri::command]
pub fn read_completed_screen_capture(
    session_id: String,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<Response, String> {
    let session = state
        .session
        .lock()
        .map_err(|_| "截图状态锁已损坏".to_owned())?;
    match session.as_ref() {
        Some(CaptureSession::Ready {
            session_id: active_id,
            png,
            ..
        }) if active_id == &session_id => Ok(Response::new(png.clone())),
        Some(_) => Err("截图会话已变化，请重新截图".to_owned()),
        None => Err("没有已完成的截图".to_owned()),
    }
}

#[tauri::command]
pub fn discard_screen_capture(
    session_id: String,
    state: tauri::State<'_, ScreenCaptureState>,
) -> Result<(), String> {
    let mut session = state
        .session
        .lock()
        .map_err(|_| "截图状态锁已损坏".to_owned())?;
    if session
        .as_ref()
        .is_some_and(|active| active.session_id() == session_id)
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
    let session_id = state
        .session
        .lock()
        .map_err(|_| "截图状态锁已损坏".to_owned())?
        .take()
        .map(|session| session.session_id().to_owned());
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.close();
    }
    app.emit_to(
        "main",
        "screen-capture-cancelled",
        ScreenCaptureCancelled { session_id },
    )
    .map_err(|error| format!("无法通知截图取消：{error}"))
}
