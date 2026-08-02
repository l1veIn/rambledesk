use tauri::image::Image;

use super::BASE_TRAY_ICON;

pub(super) fn pending_tray_icon(count: u32) -> Image<'static> {
    let mut rgba = BASE_TRAY_ICON.rgba().to_vec();
    if count == 0 {
        return Image::new_owned(rgba, BASE_TRAY_ICON.width(), BASE_TRAY_ICON.height());
    }
    let width = BASE_TRAY_ICON.width() as i32;
    let height = BASE_TRAY_ICON.height() as i32;
    let center_x = width - 8;
    let center_y = 8;
    for y in 0..height {
        for x in 0..width {
            let dx = x - center_x;
            let dy = y - center_y;
            if dx * dx + dy * dy <= 49 {
                set_icon_pixel(&mut rgba, width, x, y, [202, 58, 47, 255]);
            }
        }
    }
    let digit = count.min(9) as usize;
    const DIGITS: [[u8; 5]; 10] = [
        [0b111, 0b101, 0b101, 0b101, 0b111],
        [0b010, 0b110, 0b010, 0b010, 0b111],
        [0b111, 0b001, 0b111, 0b100, 0b111],
        [0b111, 0b001, 0b111, 0b001, 0b111],
        [0b101, 0b101, 0b111, 0b001, 0b001],
        [0b111, 0b100, 0b111, 0b001, 0b111],
        [0b111, 0b100, 0b111, 0b101, 0b111],
        [0b111, 0b001, 0b010, 0b010, 0b010],
        [0b111, 0b101, 0b111, 0b101, 0b111],
        [0b111, 0b101, 0b111, 0b001, 0b111],
    ];
    for (row, bits) in DIGITS[digit].iter().enumerate() {
        for column in 0..3 {
            if bits & (1 << (2 - column)) != 0 {
                set_icon_pixel(
                    &mut rgba,
                    width,
                    center_x - 1 + column,
                    center_y - 2 + row as i32,
                    [255, 255, 255, 255],
                );
            }
        }
    }
    Image::new_owned(rgba, BASE_TRAY_ICON.width(), BASE_TRAY_ICON.height())
}

fn set_icon_pixel(rgba: &mut [u8], width: i32, x: i32, y: i32, color: [u8; 4]) {
    let offset = ((y * width + x) * 4) as usize;
    rgba[offset..offset + 4].copy_from_slice(&color);
}
