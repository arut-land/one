#!/usr/bin/env python3
"""Convert Arut's imagegen master to native app icon formats.

Run with Pillow 12.1.1: python tools/generate-icons.py
The generated PNG is the artwork source; this script only sizes and masks it.
"""

import json
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
MASTER = ROOT / "product/brand/arut-master.png"
source = Image.open(MASTER).convert("RGB").resize((1024, 1024), Image.Resampling.LANCZOS)


def save(image, relative, size=None, **options):
    path = ROOT / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    if size:
        image = image.resize((size, size), Image.Resampling.LANCZOS)
    image.save(path, **options)


def write_json(relative, value):
    path = ROOT / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8", newline="\n")


# Desktop icons own their outer silhouette. Keep the tile inside the canvas.
desktop = Image.new("RGBA", (1024, 1024))
tile = source.resize((832, 832), Image.Resampling.LANCZOS).convert("RGBA")
mask = Image.new("L", tile.size)
ImageDraw.Draw(mask).rounded_rectangle((0, 0, 831, 831), radius=184, fill=255)
tile.putalpha(mask)
desktop.alpha_composite(tile, (96, 96))
save(desktop, "product/brand/arut-desktop.png")
save(desktop, "surfaces/windows/Assets/Arut.png", 256)
save(desktop, "surfaces/windows/Assets/Arut.ico", sizes=[(n, n) for n in (16, 20, 24, 32, 40, 48, 64, 128, 256)])
save(desktop, "product/brand/arut.icns")

catalog = "surfaces/apple/shared/app/Assets.xcassets"
write_json(f"{catalog}/Contents.json", {"info": {"author": "xcode", "version": 1}})
entries = []
for points in (16, 32, 128, 256, 512):
    for scale in (1, 2):
        name = f"mac-{points}@{scale}x.png"
        save(desktop, f"{catalog}/AppIcon.appiconset/{name}", points * scale)
        entries.append({"idiom": "mac", "size": f"{points}x{points}", "scale": f"{scale}x", "filename": name})
# Explicit slots also support the deployment targets preceding universal icons.
mobile_slots = {
    "iphone": ((20, (2, 3)), (29, (2, 3)), (40, (2, 3)), (60, (2, 3))),
    "ipad": ((20, (1, 2)), (29, (1, 2)), (40, (1, 2)), (76, (1, 2)), (83.5, (2,))),
}
for idiom, sizes in mobile_slots.items():
    for points, scales in sizes:
        for scale in scales:
            name = f"{idiom}-{points}@{scale}x.png"
            save(source, f"{catalog}/AppIcon.appiconset/{name}", int(points * scale))
            entries.append({"idiom": idiom, "size": f"{points}x{points}", "scale": f"{scale}x", "filename": name})
save(source, f"{catalog}/AppIcon.appiconset/ios-marketing.png")
entries.append({"idiom": "ios-marketing", "size": "1024x1024", "scale": "1x", "filename": "ios-marketing.png"})
write_json(f"{catalog}/AppIcon.appiconset/Contents.json", {"images": entries, "info": {"author": "xcode", "version": 1}})

for density, size in (("mdpi", 48), ("hdpi", 72), ("xhdpi", 96), ("xxhdpi", 144), ("xxxhdpi", 192)):
    save(desktop, f"surfaces/android/src/main/res/mipmap-{density}/ic_launcher.png", size)
# Separate the white mark into an alpha mask for adaptive parallax and tinting.
# The red channel separates the white foreground from the cobalt background.
alpha = source.getchannel("R").point(
    lambda value: max(0, min(255, round((value - 40) * 255 / 205)))
)
foreground = Image.new("RGBA", source.size, "white")
foreground.putalpha(alpha)
save(foreground, "product/brand/arut-mark.png")
save(foreground, "surfaces/android/src/main/res/drawable-nodpi/ic_launcher_foreground.png", 432)

save(desktop, "surfaces/web/public/favicon.ico", sizes=[(n, n) for n in (16, 32, 48)])
save(source, "surfaces/web/public/apple-touch-icon.png", 180)
for size in (192, 512):
    save(desktop, f"surfaces/web/public/icons/arut-{size}.png", size)
    save(source, f"surfaces/web/public/icons/arut-maskable-{size}.png", size)
for size in (16, 32, 48, 128):
    save(desktop, f"surfaces/chromium/public/icons/arut-{size}.png", size)
save(source, "surfaces/vscode/media/arut.png", 128)
for size in (16, 24, 32, 48, 64, 128, 256, 512):
    save(desktop, f"surfaces/linux/data/icons/hicolor/{size}x{size}/apps/dev.arut.Arut.png", size)

print("Generated Windows, Apple, Android, web, Chromium, VS Code, and Linux icons.")
