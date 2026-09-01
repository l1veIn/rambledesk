use super::geometry::{is_meaningfully_visible, visible_rectangle_area};
use super::lifecycle::validated_selection;
use super::scroll::stitch_vertical;
use super::*;

fn patterned_frame(offset: u32, width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        let absolute_y = y + offset;
        image::Rgba([
            ((x * 17 + absolute_y * 3) % 251) as u8,
            ((x * 7 + absolute_y * 11) % 241) as u8,
            ((x * 13 + absolute_y * 5) % 239) as u8,
            255,
        ])
    })
}

#[test]
fn vertical_stitch_finds_overlap_and_appends_only_new_rows() {
    let first = patterned_frame(0, 120, 100);
    let second = patterned_frame(35, 120, 100);
    let result = stitch_vertical(&first, &first, &second).expect("stitch");
    assert!(result.matched);
    assert!(result.added_height.abs_diff(35) <= 2);
    assert_eq!(
        result.image.expect("stitched image").height(),
        100 + result.added_height
    );
}

#[test]
fn vertical_stitch_skips_duplicate_frames() {
    let frame = patterned_frame(0, 120, 100);
    let result = stitch_vertical(&frame, &frame, &frame).expect("stitch");
    assert!(result.matched);
    assert_eq!(result.added_height, 0);
    assert!(result.image.is_none());
}

#[test]
fn completed_capture_rejects_non_png_payloads() {
    let encoded = base64::engine::general_purpose::STANDARD.encode(b"not a png");
    assert!(decode_canonical_png(&encoded).is_err());
}

#[test]
fn selection_is_clamped_to_the_source_image() {
    assert_eq!(
        validated_selection(
            CaptureRectangle {
                x: 90,
                y: 40,
                width: 30,
                height: 30,
            },
            100,
            60,
        )
        .expect("selection")
        .width,
        10
    );
}

#[test]
fn fully_occluded_windows_are_not_meaningfully_visible() {
    let window = CaptureRectangle {
        x: 20,
        y: 20,
        width: 200,
        height: 120,
    };
    assert!(!is_meaningfully_visible(window, &[window]));
}

#[test]
fn partially_visible_windows_remain_selectable() {
    let window = CaptureRectangle {
        x: 0,
        y: 0,
        width: 200,
        height: 120,
    };
    let cover = CaptureRectangle {
        x: 0,
        y: 0,
        width: 100,
        height: 120,
    };
    assert_eq!(visible_rectangle_area(window, &[cover]), 12_000);
    assert!(is_meaningfully_visible(window, &[cover]));
}

#[test]
fn several_front_windows_can_jointly_hide_a_background_window() {
    let window = CaptureRectangle {
        x: 0,
        y: 0,
        width: 200,
        height: 120,
    };
    let covers = [
        CaptureRectangle {
            x: 0,
            y: 0,
            width: 100,
            height: 120,
        },
        CaptureRectangle {
            x: 100,
            y: 0,
            width: 100,
            height: 120,
        },
    ];
    assert_eq!(visible_rectangle_area(window, &covers), 0);
    assert!(!is_meaningfully_visible(window, &covers));
}

#[test]
fn crop_selection_uses_the_source_pixels() {
    let image = RgbaImage::from_fn(40, 30, |x, y| image::Rgba([x as u8, y as u8, 0, 255]));
    let cropped = crop_selection(
        &image,
        CaptureRectangle {
            x: 4,
            y: 2,
            width: 12,
            height: 10,
        },
    )
    .expect("crop");
    assert_eq!(cropped.dimensions(), (12, 10));
    assert_eq!(cropped.get_pixel(0, 0).0, [4, 2, 0, 255]);
}
