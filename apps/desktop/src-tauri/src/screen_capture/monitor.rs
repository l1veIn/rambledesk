use super::capture_platform::capture_monitor;
use super::*;

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

pub(super) fn choose_capture_monitors(
    app: &AppHandle,
) -> Result<(Vec<xcap::Monitor>, MonitorWindow), String> {
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

pub(super) fn monitor_by_id(monitor_id: u32) -> Result<xcap::Monitor, String> {
    xcap::Monitor::all()
        .map_err(|error| format!("无法获取显示器列表：{error}"))?
        .into_iter()
        .find(|monitor| monitor.id().ok() == Some(monitor_id))
        .ok_or_else(|| "截图期间显示器配置发生变化，请重新截图".to_owned())
}

pub(super) fn capture_monitors(
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
