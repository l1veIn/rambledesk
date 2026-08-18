use std::fs;

use serde::{Deserialize, Serialize};
use tauri::{Manager, PhysicalPosition, PhysicalRect, PhysicalSize, WebviewWindow};

use super::RAMBLE_CONSOLE_EDGE_GAP;

const WINDOWS_EXTRA_EDGE_GAP: f64 = 16.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
struct SavedConsolePosition {
    x: i32,
    y: i32,
}

#[cfg(test)]
pub(super) fn right_center_position(
    work_area: PhysicalRect<i32, u32>,
    window_size: PhysicalSize<u32>,
    scale_factor: f64,
) -> PhysicalPosition<i32> {
    right_center_position_with_gap(
        work_area,
        window_size,
        scale_factor,
        RAMBLE_CONSOLE_EDGE_GAP,
    )
}

fn right_center_position_with_gap(
    work_area: PhysicalRect<i32, u32>,
    window_size: PhysicalSize<u32>,
    scale_factor: f64,
    edge_gap: f64,
) -> PhysicalPosition<i32> {
    let gap = (edge_gap * scale_factor).round() as i64;
    let x = i64::from(work_area.position.x) + i64::from(work_area.size.width)
        - i64::from(window_size.width)
        - gap;
    let y = i64::from(work_area.position.y)
        + (i64::from(work_area.size.height) - i64::from(window_size.height)) / 2;
    clamp_position_to_work_area(
        PhysicalPosition::new(clamp_i32(x), clamp_i32(y)),
        window_size,
        work_area,
    )
}

fn platform_edge_gap() -> f64 {
    if cfg!(windows) {
        RAMBLE_CONSOLE_EDGE_GAP + WINDOWS_EXTRA_EDGE_GAP
    } else {
        RAMBLE_CONSOLE_EDGE_GAP
    }
}

pub(super) fn expected_console_size(scale_factor: f64) -> PhysicalSize<u32> {
    PhysicalSize::new(
        (super::RAMBLE_CONSOLE_WIDTH * scale_factor).round() as u32,
        (super::RAMBLE_CONSOLE_HEIGHT * scale_factor).round() as u32,
    )
}

pub(super) fn effective_console_size(
    reported: PhysicalSize<u32>,
    scale_factor: f64,
) -> PhysicalSize<u32> {
    let expected = expected_console_size(scale_factor);
    PhysicalSize::new(
        reported.width.max(expected.width),
        reported.height.max(expected.height),
    )
}

pub(super) fn clamp_position_to_work_area(
    position: PhysicalPosition<i32>,
    window_size: PhysicalSize<u32>,
    work_area: PhysicalRect<i32, u32>,
) -> PhysicalPosition<i32> {
    let min_x = i64::from(work_area.position.x);
    let min_y = i64::from(work_area.position.y);
    let max_x = min_x + i64::from(work_area.size.width) - i64::from(window_size.width);
    let max_y = min_y + i64::from(work_area.size.height) - i64::from(window_size.height);
    PhysicalPosition::new(
        clamp_i32(i64::from(position.x).clamp(min_x, max_x.max(min_x))),
        clamp_i32(i64::from(position.y).clamp(min_y, max_y.max(min_y))),
    )
}

fn overlap_area(
    position: PhysicalPosition<i32>,
    window_size: PhysicalSize<u32>,
    work_area: PhysicalRect<i32, u32>,
) -> u64 {
    let left = i64::from(position.x).max(i64::from(work_area.position.x));
    let top = i64::from(position.y).max(i64::from(work_area.position.y));
    let right = (i64::from(position.x) + i64::from(window_size.width))
        .min(i64::from(work_area.position.x) + i64::from(work_area.size.width));
    let bottom = (i64::from(position.y) + i64::from(window_size.height))
        .min(i64::from(work_area.position.y) + i64::from(work_area.size.height));
    u64::try_from((right - left).max(0) * (bottom - top).max(0)).unwrap_or(0)
}

pub(super) fn resolve_console_position(
    work_areas: &[PhysicalRect<i32, u32>],
    preferred_work_area: PhysicalRect<i32, u32>,
    reported_size: PhysicalSize<u32>,
    scale_factor: f64,
    saved: Option<PhysicalPosition<i32>>,
) -> PhysicalPosition<i32> {
    let size = effective_console_size(reported_size, scale_factor);
    if let Some(saved) = saved
        && let Some(work_area) = work_areas
            .iter()
            .copied()
            .max_by_key(|area| overlap_area(saved, size, *area))
            .filter(|area| overlap_area(saved, size, *area) > 0)
    {
        return clamp_position_to_work_area(saved, size, work_area);
    }
    right_center_position_with_gap(preferred_work_area, size, scale_factor, platform_edge_gap())
}

fn clamp_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn console_position_path() -> Option<std::path::PathBuf> {
    rambledesk_storage::default_app_data_root()
        .ok()
        .map(|root| root.join("ramble-console-position.json"))
}

fn load_saved_console_position() -> Option<PhysicalPosition<i32>> {
    let path = console_position_path()?;
    let contents = fs::read_to_string(path).ok()?;
    let saved = serde_json::from_str::<SavedConsolePosition>(&contents).ok()?;
    Some(PhysicalPosition::new(saved.x, saved.y))
}

pub(super) fn remember_ramble_console_position(position: PhysicalPosition<i32>) {
    let Some(path) = console_position_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let payload = SavedConsolePosition {
        x: position.x,
        y: position.y,
    };
    if let Ok(json) = serde_json::to_string(&payload)
        && let Err(error) = fs::write(&path, json)
    {
        tracing::warn!(%error, path = %path.display(), "failed to save Ramble console position");
    }
}

pub(super) fn attach_ramble_console_events(console: &WebviewWindow) {
    let handle = console.clone();
    console.on_window_event(move |event| match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            if let Ok(position) = handle.outer_position() {
                remember_ramble_console_position(position);
            }
            let _ = handle.hide();
        }
        tauri::WindowEvent::Moved(position) => {
            if handle.is_visible().unwrap_or(false) {
                remember_ramble_console_position(*position);
            }
        }
        _ => {}
    });
}

pub(super) fn position_ramble_console(
    app: &tauri::AppHandle,
    console: &WebviewWindow,
) -> tauri::Result<()> {
    let preferred_monitor = app
        .get_webview_window("main")
        .and_then(|window| window.current_monitor().ok().flatten())
        .or(console.primary_monitor()?);
    let Some(preferred_monitor) = preferred_monitor else {
        return Ok(());
    };
    let work_areas = app.available_monitors().ok().map_or_else(
        || vec![*preferred_monitor.work_area()],
        |monitors| {
            let areas = monitors
                .iter()
                .map(|monitor| *monitor.work_area())
                .collect::<Vec<_>>();
            if areas.is_empty() {
                vec![*preferred_monitor.work_area()]
            } else {
                areas
            }
        },
    );
    let reported = console
        .outer_size()
        .unwrap_or_else(|_| PhysicalSize::new(0, 0));
    let position = resolve_console_position(
        &work_areas,
        *preferred_monitor.work_area(),
        reported,
        preferred_monitor.scale_factor(),
        load_saved_console_position(),
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

#[tauri::command]
pub(super) fn show_ramble_console(app: tauri::AppHandle) -> Result<(), String> {
    let console = app
        .get_webview_window(super::RAMBLE_CONSOLE_LABEL)
        .ok_or_else(|| "Ramble console window is not available".to_owned())?;
    if let Err(error) = position_ramble_console(&app, &console) {
        tracing::warn!(%error, "failed to position the Ramble console");
    }
    console
        .show()
        .map_err(|error| format!("failed to show the Ramble console: {error}"))?;
    // Windows often reports a usable outer size only after the window is shown.
    if let Err(error) = position_ramble_console(&app, &console) {
        tracing::warn!(%error, "failed to refine the Ramble console position");
    }
    let _ = console.set_focus();
    tracing::info!("opened Ramble console");
    Ok(())
}

#[tauri::command]
pub(super) fn hide_ramble_console(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(console) = app.get_webview_window(super::RAMBLE_CONSOLE_LABEL) {
        if let Ok(position) = console.outer_position() {
            remember_ramble_console_position(position);
        }
        console
            .hide()
            .map_err(|error| format!("failed to hide the Ramble console: {error}"))?;
        tracing::info!("hid Ramble console");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: i32, y: i32, width: u32, height: u32) -> PhysicalRect<i32, u32> {
        PhysicalRect {
            position: PhysicalPosition::new(x, y),
            size: PhysicalSize::new(width, height),
        }
    }

    #[test]
    fn default_right_center_uses_a_logical_ten_pixel_gap() {
        let position = right_center_position(
            area(-1_920, 40, 1_920, 1_040),
            PhysicalSize::new(132, 608),
            2.0,
        );
        assert_eq!(position, PhysicalPosition::new(-152, 256));
    }

    #[test]
    fn reported_zero_size_falls_back_to_the_designed_console_size() {
        assert_eq!(
            effective_console_size(PhysicalSize::new(0, 0), 2.0),
            PhysicalSize::new(116, 608)
        );
    }

    #[test]
    fn clamp_keeps_the_console_inside_the_work_area() {
        let position = clamp_position_to_work_area(
            PhysicalPosition::new(1_900, 1_000),
            PhysicalSize::new(116, 608),
            area(0, 0, 1_920, 1_080),
        );
        assert_eq!(position, PhysicalPosition::new(1_804, 472));
    }

    #[test]
    fn saved_position_is_restored_when_it_still_overlaps_a_monitor() {
        let saved = PhysicalPosition::new(200, 180);
        let position = resolve_console_position(
            &[area(0, 0, 1_920, 1_080)],
            area(0, 0, 1_920, 1_080),
            PhysicalSize::new(116, 608),
            2.0,
            Some(saved),
        );
        assert_eq!(position, saved);
    }

    #[test]
    fn disconnected_saved_position_falls_back_to_the_default_edge() {
        let position = resolve_console_position(
            &[area(0, 0, 1_920, 1_080)],
            area(0, 0, 1_920, 1_080),
            PhysicalSize::new(116, 608),
            1.0,
            Some(PhysicalPosition::new(8_000, 8_000)),
        );
        let expected = right_center_position_with_gap(
            area(0, 0, 1_920, 1_080),
            PhysicalSize::new(116, 608),
            1.0,
            platform_edge_gap(),
        );
        assert_eq!(position, expected);
    }
}
