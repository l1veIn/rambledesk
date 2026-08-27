use super::capture_platform::{capture_monitor, excluded_capture_window_ids};
use super::lifecycle::validated_selection;
use super::monitor::monitor_by_id;
use super::overlay::configure_capture_overlay;
use super::*;

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
    let (monitor, selection, first_frame, restore) = {
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
            restore,
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
                restore,
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
                    restore,
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
                        restore,
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
            restore,
        });
        (monitor, selection, first_frame, restore)
    };

    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.hide();
    }
    if let Err(error) = create_scroll_window(&app, &monitor, selection) {
        if let Ok(mut session) = state.session.lock() {
            *session = None;
        }
        restore_capture_windows(&app, restore);
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
            restore,
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
            restore,
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

pub(super) struct StitchResult {
    pub(super) image: Option<RgbaImage>,
    pub(super) added_height: u32,
    pub(super) matched: bool,
}

pub(super) fn stitch_vertical(
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
