use tauri::{Manager, PhysicalPosition, PhysicalRect, PhysicalSize, WebviewWindow};

use super::RAMBLE_CONSOLE_EDGE_GAP;

pub(super) fn right_center_position(
    work_area: PhysicalRect<i32, u32>,
    window_size: PhysicalSize<u32>,
    scale_factor: f64,
) -> PhysicalPosition<i32> {
    let gap = (RAMBLE_CONSOLE_EDGE_GAP * scale_factor).round() as i64;
    let x = i64::from(work_area.position.x) + i64::from(work_area.size.width)
        - i64::from(window_size.width)
        - gap;
    let y = i64::from(work_area.position.y)
        + (i64::from(work_area.size.height) - i64::from(window_size.height)) / 2;
    PhysicalPosition::new(
        x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    )
}

pub(super) fn position_ramble_console(
    app: &tauri::AppHandle,
    console: &WebviewWindow,
) -> tauri::Result<()> {
    let monitor = app
        .get_webview_window("main")
        .and_then(|window| window.current_monitor().ok().flatten())
        .or(console.primary_monitor()?);
    let Some(monitor) = monitor else {
        return Ok(());
    };
    let position = right_center_position(
        *monitor.work_area(),
        console.outer_size()?,
        monitor.scale_factor(),
    );
    console.set_position(position)
}

pub(super) fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
