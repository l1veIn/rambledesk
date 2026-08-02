use super::capture_platform::WindowSnapshot;
use super::*;

fn subtract_rectangle(source: CaptureRectangle, cover: CaptureRectangle) -> Vec<CaptureRectangle> {
    let left = source.x.max(cover.x);
    let top = source.y.max(cover.y);
    let right = (source.x + source.width).min(cover.x + cover.width);
    let bottom = (source.y + source.height).min(cover.y + cover.height);
    if left >= right || top >= bottom {
        return vec![source];
    }

    let mut remaining = Vec::with_capacity(4);
    if source.y < top {
        remaining.push(CaptureRectangle {
            x: source.x,
            y: source.y,
            width: source.width,
            height: top - source.y,
        });
    }
    let source_bottom = source.y + source.height;
    if bottom < source_bottom {
        remaining.push(CaptureRectangle {
            x: source.x,
            y: bottom,
            width: source.width,
            height: source_bottom - bottom,
        });
    }
    if source.x < left {
        remaining.push(CaptureRectangle {
            x: source.x,
            y: top,
            width: left - source.x,
            height: bottom - top,
        });
    }
    let source_right = source.x + source.width;
    if right < source_right {
        remaining.push(CaptureRectangle {
            x: right,
            y: top,
            width: source_right - right,
            height: bottom - top,
        });
    }
    remaining
}

pub(super) fn visible_rectangle_area(
    rectangle: CaptureRectangle,
    occluders: &[CaptureRectangle],
) -> u64 {
    let mut fragments = vec![rectangle];
    for occluder in occluders {
        fragments = fragments
            .into_iter()
            .flat_map(|fragment| subtract_rectangle(fragment, *occluder))
            .collect();
        if fragments.is_empty() {
            return 0;
        }
    }
    fragments
        .iter()
        .map(|fragment| u64::from(fragment.width) * u64::from(fragment.height))
        .sum()
}

pub(super) fn is_meaningfully_visible(
    rectangle: CaptureRectangle,
    occluders: &[CaptureRectangle],
) -> bool {
    let visible_area = visible_rectangle_area(rectangle, occluders);
    let total_area = u64::from(rectangle.width) * u64::from(rectangle.height);
    visible_area >= 48 * 36 && visible_area.saturating_mul(100) >= total_area * 8
}

pub(super) fn collect_window_targets(
    monitor: &MonitorWindow,
    image: &RgbaImage,
    snapshots: Vec<WindowSnapshot>,
) -> Vec<CaptureTarget> {
    let scale_x = image.width() as f64 / monitor.monitor_width.max(1) as f64;
    let scale_y = image.height() as f64 / monitor.monitor_height.max(1) as f64;
    let mut targets = Vec::new();
    let mut occluders = Vec::new();

    for window in snapshots {
        let left = ((window.x - monitor.monitor_x) as f64 * scale_x).round() as i64;
        let top = ((window.y - monitor.monitor_y) as f64 * scale_y).round() as i64;
        let right = left + (window.width as f64 * scale_x).round() as i64;
        let bottom = top + (window.height as f64 * scale_y).round() as i64;
        let clipped_left = left.clamp(0, image.width() as i64);
        let clipped_top = top.clamp(0, image.height() as i64);
        let clipped_right = right.clamp(0, image.width() as i64);
        let clipped_bottom = bottom.clamp(0, image.height() as i64);
        if clipped_right <= clipped_left || clipped_bottom <= clipped_top {
            continue;
        }
        let rectangle = CaptureRectangle {
            x: clipped_left as u32,
            y: clipped_top as u32,
            width: (clipped_right - clipped_left) as u32,
            height: (clipped_bottom - clipped_top) as u32,
        };
        if is_meaningfully_visible(rectangle, &occluders)
            && rectangle.width >= 48
            && rectangle.height >= 36
            && (!window.title.is_empty() || !window.app_name.is_empty())
        {
            targets.push(CaptureTarget {
                id: window.id,
                title: window.title,
                app_name: window.app_name,
                x: rectangle.x,
                y: rectangle.y,
                width: rectangle.width,
                height: rectangle.height,
            });
        }
        if window.opaque {
            occluders.push(rectangle);
        }
    }
    targets
}
