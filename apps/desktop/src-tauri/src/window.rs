use std::fs;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{Manager, PhysicalPosition, PhysicalRect, PhysicalSize, WebviewWindow};

use super::RAMBLE_CONSOLE_EDGE_GAP;

const WINDOWS_EXTRA_EDGE_GAP: f64 = 16.0;

pub(super) struct SpeechOverlayVisibility {
    requested: AtomicBool,
    capture_hidden: AtomicBool,
    position: Mutex<Option<SavedSpeechPosition>>,
    layout_position: Mutex<Option<PhysicalPosition<i32>>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct SavedSpeechPosition {
    x: i32,
    bottom: i32,
}

impl Default for SpeechOverlayVisibility {
    fn default() -> Self {
        let position = speech_position_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|raw| serde_json::from_str(&raw).ok());
        Self {
            requested: AtomicBool::new(false),
            capture_hidden: AtomicBool::new(false),
            position: Mutex::new(position),
            layout_position: Mutex::new(None),
        }
    }
}

fn speech_position_path() -> Option<std::path::PathBuf> {
    rambledesk_storage::default_app_data_root()
        .ok()
        .map(|root| root.join("speech-overlay-position.json"))
}

pub(super) fn attach_speech_overlay_events(overlay: &WebviewWindow) {
    let handle = overlay.clone();
    overlay.on_window_event(move |event| match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = handle.hide();
        }
        tauri::WindowEvent::Moved(position) if handle.is_visible().unwrap_or(false) => {
            let state = handle.app_handle().state::<SpeechOverlayVisibility>();
            // Layout updates also emit Moved. Only persist actual user movement.
            if state
                .layout_position
                .lock()
                .ok()
                .is_some_and(|mut expected| expected.take() == Some(*position))
            {
                return;
            }
            let Ok(size) = handle.outer_size() else {
                return;
            };
            let saved = SavedSpeechPosition {
                x: position.x,
                bottom: clamp_i32(i64::from(position.y) + i64::from(size.height)),
            };
            if let Ok(mut value) = state.position.lock() {
                *value = Some(saved);
            }
            if let Some(path) = speech_position_path() {
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Ok(json) = serde_json::to_string(&saved)
                    && let Err(error) = fs::write(path, json)
                {
                    tracing::warn!(%error, "failed to save speech overlay position");
                }
            }
        }
        _ => {}
    });
}

fn speech_overlay_position(
    area: PhysicalRect<i32, u32>,
    size: PhysicalSize<u32>,
    scale: f64,
    saved: Option<SavedSpeechPosition>,
) -> PhysicalPosition<i32> {
    let position = saved.map_or_else(
        || {
            PhysicalPosition::new(
                clamp_i32(
                    i64::from(area.position.x)
                        + (i64::from(area.size.width) - i64::from(size.width)) / 2,
                ),
                clamp_i32(
                    i64::from(area.position.y) + i64::from(area.size.height)
                        - i64::from(size.height)
                        - (24.0 * scale).round() as i64,
                ),
            )
        },
        |saved| {
            PhysicalPosition::new(
                saved.x,
                clamp_i32(i64::from(saved.bottom) - i64::from(size.height)),
            )
        },
    );
    clamp_position_to_work_area(position, size, area)
}

fn speech_anchor_in_area(saved: SavedSpeechPosition, area: PhysicalRect<i32, u32>) -> bool {
    i64::from(saved.x) >= i64::from(area.position.x)
        && i64::from(saved.x) < i64::from(area.position.x) + i64::from(area.size.width)
        && i64::from(saved.bottom) > i64::from(area.position.y)
        && i64::from(saved.bottom) <= i64::from(area.position.y) + i64::from(area.size.height)
}

pub(super) fn suspend_speech_overlay(app: &tauri::AppHandle, hidden: bool) {
    let state = app.state::<SpeechOverlayVisibility>();
    state.capture_hidden.store(hidden, Ordering::SeqCst);
    if let Some(overlay) = app.get_webview_window("speech-overlay") {
        if hidden || !state.requested.load(Ordering::SeqCst) {
            let _ = overlay.hide();
        } else {
            let _ = overlay.show();
        }
    }
}

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

#[tauri::command]
pub(super) fn focus_speech_feedback(app: tauri::AppHandle) {
    show_main_window(&app);
}

#[tauri::command]
pub(super) fn set_speech_overlay_layout(
    app: tauri::AppHandle,
    visible: bool,
    height: f64,
) -> Result<(), String> {
    let visibility = app.state::<SpeechOverlayVisibility>();
    visibility.requested.store(visible, Ordering::SeqCst);
    let overlay = app
        .get_webview_window("speech-overlay")
        .ok_or_else(|| "Speech overlay window is not available".to_owned())?;
    if !visible {
        return overlay.hide().map_err(|error| error.to_string());
    }
    let saved = *visibility
        .position
        .lock()
        .map_err(|error| error.to_string())?;
    let saved_monitor = saved.and_then(|position| {
        app.available_monitors()
            .ok()?
            .into_iter()
            .find(|monitor| speech_anchor_in_area(position, *monitor.work_area()))
    });
    let saved = if saved_monitor.is_some() { saved } else { None };
    let monitor = saved_monitor
        .or_else(|| {
            app.get_webview_window("main")
                .and_then(|window| window.current_monitor().ok().flatten())
        })
        .or(overlay
            .primary_monitor()
            .map_err(|error| error.to_string())?);
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let width = 436.0_f64
            .min(f64::from(area.size.width) / scale - 24.0)
            .max(200.0);
        let height = if height.is_finite() { height } else { 140.0 }
            .clamp(80.0, 480.0)
            .min(f64::from(area.size.height) / scale - 48.0);
        let size = PhysicalSize::new(
            (width * scale).round() as u32,
            (height * scale).round() as u32,
        );
        let position = speech_overlay_position(*area, size, scale, saved);
        *visibility
            .layout_position
            .lock()
            .map_err(|error| error.to_string())? = Some(position);
        overlay.set_size(size).map_err(|error| error.to_string())?;
        overlay
            .set_position(position)
            .map_err(|error| error.to_string())?;
    }
    if visibility.capture_hidden.load(Ordering::SeqCst) {
        overlay.hide().map_err(|error| error.to_string())?;
    } else if !overlay.is_visible().unwrap_or(false) {
        overlay.show().map_err(|error| error.to_string())?;
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
    fn speech_overlay_expands_upward_from_the_dragged_bottom_anchor() {
        let saved = Some(SavedSpeechPosition {
            x: -1200,
            bottom: 850,
        });
        let monitor = area(-1920, 0, 1920, 1080);
        let short = speech_overlay_position(monitor, PhysicalSize::new(654, 210), 1.5, saved);
        let tall = speech_overlay_position(monitor, PhysicalSize::new(654, 600), 1.5, saved);
        assert_eq!(short, PhysicalPosition::new(-1200, 640));
        assert_eq!(tall, PhysicalPosition::new(-1200, 250));
    }

    #[test]
    fn speech_overlay_clamps_saved_position_and_detects_disconnected_monitors() {
        let monitor = area(0, 0, 1920, 1040);
        let saved = SavedSpeechPosition {
            x: 1800,
            bottom: 100,
        };
        assert!(speech_anchor_in_area(saved, monitor));
        assert_eq!(
            speech_overlay_position(monitor, PhysicalSize::new(436, 400), 1.0, Some(saved)),
            PhysicalPosition::new(1484, 0)
        );
        assert!(!speech_anchor_in_area(
            SavedSpeechPosition {
                x: -1200,
                bottom: 850
            },
            monitor
        ));
        assert_eq!(
            speech_overlay_position(monitor, PhysicalSize::new(436, 140), 1.0, None),
            PhysicalPosition::new(742, 876)
        );
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
