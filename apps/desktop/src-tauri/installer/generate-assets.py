"""Rebuild NSIS Modern UI bitmaps and the DMG background from brand assets.

MUI2 copies the welcome sidebar into a 164x314 control and the header into
150x57. Larger sources are downsampled by NSIS first (nearest-neighbor), then
Windows stretches that tiny bitmap on HiDPI — so shipping 3x BMPs looks worse,
not better, and an oversized header can break the next installer page.

Paint at SCALE, then Lanczos-resample to the official MUI sizes. Keep type off
the bitmap; NSIS already draws the page copy with real fonts.

Run from anywhere:

    python apps/desktop/src-tauri/installer/generate-assets.py
"""

from __future__ import annotations

import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent
DESKTOP = ROOT.parent.parent
CUTOUT = ROOT / "rambelle-cutout.png"
ICON = ROOT / ".." / "icons" / "icon.png"
DMG_DIR = ROOT.parent / "dmg"

# Official MUI2 sizes at 96 DPI.
SIDEBAR = (164, 314)
HEADER = (150, 57)
# 3x covers 150%, 200%, and 300% without upscaling the source.
SCALE = 3

ICE_TOP = (247, 249, 252)
ICE_BOTTOM = (198, 221, 240)
NAVY = (32, 51, 75)
ICE = (111, 168, 220)
PRIMARY = (39, 117, 202)
CYAN = (87, 198, 192)
MUTED = (96, 115, 138)
WHITE = (255, 255, 255)


def load_font(
    size: int, bold: bool = True, *, cjk: bool = False
) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    size = max(8, int(size))
    if cjk:
        candidates = [
            "C:/Windows/Fonts/msyhbd.ttc" if bold else "C:/Windows/Fonts/msyh.ttc",
            "C:/Windows/Fonts/msyhbd.ttf" if bold else "C:/Windows/Fonts/msyh.ttf",
            "C:/Windows/Fonts/NotoSansSC-VF.ttf",
            "/System/Library/Fonts/PingFang.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ]
    else:
        candidates = [
            "C:/Windows/Fonts/segoeuib.ttf" if bold else "C:/Windows/Fonts/segoeui.ttf",
            "C:/Windows/Fonts/arialbd.ttf" if bold else "C:/Windows/Fonts/arial.ttf",
            "/System/Library/Fonts/SFNS.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
            if bold
            else "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]
    for path in candidates:
        if Path(path).exists():
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()


def ice_gradient(size: tuple[int, int]) -> Image.Image:
    width, height = size
    image = Image.new("RGB", size, ICE_TOP)
    pixels = image.load()
    for y in range(height):
        t = y / max(1, height - 1)
        color = tuple(int(a + (b - a) * t) for a, b in zip(ICE_TOP, ICE_BOTTOM))
        for x in range(width):
            pixels[x, y] = color
    return image


def add_hex_grid(image: Image.Image, alpha: int = 48) -> Image.Image:
    overlay = Image.new("RGBA", image.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(overlay)
    radius = 22 * SCALE
    height = math.sqrt(3) * radius
    width = 1.5 * radius
    stroke = max(1, round(1.1 * SCALE))
    color = (*ICE, alpha)
    rows = int(image.height / height) + 3
    cols = int(image.width / width) + 3
    for row in range(-1, rows):
        for col in range(-1, cols):
            cx = col * width
            cy = row * height + (height / 2 if col % 2 else 0)
            points = [
                (
                    cx + radius * math.cos(math.radians(angle)),
                    cy + radius * math.sin(math.radians(angle)),
                )
                for angle in range(0, 360, 60)
            ]
            draw.polygon(points, outline=color, width=stroke)
    composed = Image.alpha_composite(image.convert("RGBA"), overlay)
    return composed.convert("RGB")


def opaque_bbox(image: Image.Image, threshold: int = 12) -> tuple[int, int, int, int]:
    alpha = image.getchannel("A")
    return alpha.point(lambda value: 255 if value > threshold else 0).getbbox() or image.getbbox()


def fit_cutout(dest_size: tuple[int, int]) -> Image.Image:
    source = Image.open(CUTOUT).convert("RGBA")
    left, top, right, bottom = opaque_bbox(source)
    cropped = source.crop((left, top, right, bottom))
    dest_w, dest_h = dest_size
    # Prefer the head and torso in the narrow sidebar: crop the source to the
    # destination aspect from the top of the hairline, not a letterboxed full body.
    target_aspect = dest_w / dest_h
    src_w, src_h = cropped.size
    crop_h = min(src_h, int(src_w / target_aspect))
    cropped = cropped.crop((0, 0, src_w, crop_h))
    fitted = cropped.resize(dest_size, Image.Resampling.LANCZOS)
    return fitted


def save_bmp(image: Image.Image, path: Path) -> None:
    # Classic 24-bit BITMAPINFOHEADER. NSIS MUI cannot load BMP v4/v5 reliably.
    image.convert("RGB").save(path, "BMP")


def downsample(image: Image.Image, size: tuple[int, int]) -> Image.Image:
    return image.resize(size, Image.Resampling.LANCZOS)


def build_sidebar() -> None:
    width, height = SIDEBAR[0] * SCALE, SIDEBAR[1] * SCALE
    image = add_hex_grid(ice_gradient((width, height)))

    # Fill the panel with a head-and-torso crop. Small baked-in type is what
    # looked mosaicked after NSIS/Windows stretched the bitmap.
    dest = (int(6 * SCALE), int(18 * SCALE), int(158 * SCALE), int(306 * SCALE))
    cutout = fit_cutout((dest[2] - dest[0], dest[3] - dest[1]))
    glow = Image.new("RGBA", image.size, (0, 0, 0, 0))
    gdraw = ImageDraw.Draw(glow)
    gdraw.ellipse(
        (dest[0] + 4 * SCALE, dest[1] + 36 * SCALE, dest[2] - 4 * SCALE, dest[3] + 4 * SCALE),
        fill=(255, 255, 255, 150),
    )
    glow = glow.filter(ImageFilter.GaussianBlur(12 * SCALE))
    image = Image.alpha_composite(image.convert("RGBA"), glow)
    image.paste(cutout, (dest[0], dest[1]), cutout)
    save_bmp(downsample(image.convert("RGB"), SIDEBAR), ROOT / "sidebar.bmp")


def build_header() -> None:
    width, height = HEADER[0] * SCALE, HEADER[1] * SCALE
    image = add_hex_grid(ice_gradient((width, height)), alpha=36)
    icon = Image.open(ICON).convert("RGBA").resize((49 * SCALE, 49 * SCALE), Image.Resampling.LANCZOS)
    image = image.convert("RGBA")
    image.paste(icon, (4 * SCALE, 4 * SCALE), icon)
    draw = ImageDraw.Draw(image)
    draw.text((58 * SCALE, 16 * SCALE), "RambleDesk", font=load_font(13 * SCALE), fill=NAVY)
    save_bmp(downsample(image.convert("RGB"), HEADER), ROOT / "header.bmp")


def build_dmg() -> None:
    # 2x the Finder window so the background stays sharp on retina.
    width, height = 1440, 920
    image = add_hex_grid(ice_gradient((width, height)), alpha=32)
    draw = ImageDraw.Draw(image)

    title = load_font(44, cjk=True)
    subtitle = load_font(24, bold=False, cjk=True)
    body = load_font(22, bold=False, cjk=True)
    strong = load_font(26, cjk=True)
    mono = load_font(18, bold=False, cjk=True)

    draw.text((width // 2, 72), "Install RambleDesk  ·  安装 RambleDesk", font=title, fill=NAVY, anchor="mt")
    draw.text(
        (width // 2, 128),
        "Drag RambleDesk into Applications  /  将 RambleDesk 拖入“应用程序”",
        font=subtitle,
        fill=MUTED,
        anchor="mt",
    )

    # Arrow pill between the app icon and Applications.
    pill = (560, 250, 880, 386)
    draw.rounded_rectangle(pill, radius=68, fill=WHITE)
    draw.line((620, 318, 780, 318), fill=PRIMARY, width=14)
    draw.polygon([(760, 286), (820, 318), (760, 350)], fill=CYAN)
    draw.text((width // 2, 430), "DRAG TO INSTALL", font=load_font(20), fill=MUTED, anchor="mt")

    card = (56, 560, width - 56, height - 48)
    draw.rounded_rectangle(card, radius=28, fill=WHITE)
    draw.rectangle((56, 560, 68, height - 48), fill=PRIMARY)
    x = 88
    draw.text((x, 590), "First launch  /  首次启动", font=strong, fill=NAVY)
    draw.text(
        (x, 640),
        "1. In Applications, right-click RambleDesk → Open  /  在“应用程序”中右键 → 打开",
        font=body,
        fill=MUTED,
    )
    draw.text(
        (x, 680),
        "2. Still blocked? System Settings → Privacy & Security → Open Anyway",
        font=body,
        fill=MUTED,
    )
    draw.text((x, 716), "仍被阻止：系统设置 → 隐私与安全性 → 仍要打开", font=body, fill=MUTED)
    draw.text(
        (x, 768),
        "“App is damaged / 已损坏”?  xattr -dr com.apple.quarantine /Applications/RambleDesk.app",
        font=mono,
        fill=PRIMARY,
    )

    DMG_DIR.mkdir(parents=True, exist_ok=True)
    image.save(DMG_DIR / "background.png", "PNG")


def main() -> None:
    if not CUTOUT.exists():
        raise SystemExit(f"missing cutout: {CUTOUT}")
    if not ICON.exists():
        raise SystemExit(f"missing icon: {ICON}")
    build_sidebar()
    build_header()
    build_dmg()
    print(f"Generated NSIS {SCALE}x bitmaps and DMG background in {ROOT} and {DMG_DIR}")


if __name__ == "__main__":
    main()
