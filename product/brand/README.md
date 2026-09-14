# Arut app icon

The white open loop suggests an `a` and a conversation. The cobalt tile provides contrast in light and dark app launchers. There is no existing brand palette in the product documents. This icon introduces cobalt as app artwork; it does not override native UI accent colors.

`arut-master.png` is the selected 1254 × 1254 imagegen output, preserved unchanged. It is raster artwork, not an SVG. `arut-desktop.png` adds transparent outer padding and a rounded tile for desktop launchers. `arut-mark.png` separates the white mark for Android's adaptive and monochrome layers. `arut.icns` contains the macOS raster representations; Xcode uses the app icon asset catalog instead.

## Regenerate

The conversion script requires Python 3 and Pillow 12.1.1. A disposable environment keeps this optional asset tool outside normal application builds:

```sh
python -m venv target/icon-tools
# Windows: target/icon-tools/Scripts/python.exe
# macOS/Linux: target/icon-tools/bin/python
target/icon-tools/Scripts/python.exe -m pip install Pillow==12.1.1
target/icon-tools/Scripts/python.exe tools/generate-icons.py
```

The script resizes the master with Lanczos sampling. It applies desktop padding and corner masks, and separates the white foreground into an alpha channel for Android. It does not call an image model. Generated app assets are checked in so application builds do not need Python.

## Integration

| App | Asset and integration |
| --- | --- |
| Windows | `surfaces/windows/Assets/Arut.ico`, with 16, 20, 24, 32, 40, 48, 64, 128 and 256 pixel frames. The project embeds it in the executable and the window loads it for the taskbar. `Arut.png` supplies the titlebar. |
| macOS and iOS | `surfaces/apple/shared/app/Assets.xcassets/AppIcon.appiconset`, selected by `ASSETCATALOG_COMPILER_APPICON_NAME` in `project.yml`. macOS has padded RGBA images; iOS has opaque square RGB images and applies its own mask. Regenerate the Xcode project after changing the catalog setting. |
| Android | Density-specific legacy mipmaps, API 26 adaptive foreground/background layers, and API 33 monochrome support. `AndroidManifest.xml` selects `@mipmap/ic_launcher`. |
| Web | Favicon, Apple touch icon, and 192/512 pixel ordinary and maskable icons, referenced by `index.html` and `manifest.webmanifest`. The manifest supplies app metadata; offline support requires a separate service worker. |
| Chromium | 16/32/48/128 pixel icons declared in the extension manifest, plus toolbar icons. |
| VS Code | `surfaces/vscode/media/arut.png`, selected by the extension's `icon` field. |
| Linux | Hicolor PNGs and `dev.arut.Arut.desktop` under `surfaces/linux/data`. The crate's `build.rs` also compiles the PNGs into its GResource, so the window and the About dialog show the icon with nothing installed; the launcher still needs the theme copy. The surface's own symbolic icons are Lucide SVGs under `surfaces/linux/data/icons/lucide`, a Linux-only choice; Windows and Apple keep their native symbol sets. Run `sh surfaces/linux/install-icons.sh` to install into the user XDG data directory, or pass a staging data directory as its first argument. The existing GTK icon name matches the desktop entry. Install `arut-linux` on PATH separately. |

The [Android adaptive icon requirements](https://developer.android.com/develop/ui/compose/system/icon_design_adaptive), [Apple app icon catalog format](https://developer.apple.com/library/archive/documentation/Xcode/Reference/xcode_ref-Asset_Catalog_Format/AppIconType.html), and [Chromium icon manifest](https://developer.chrome.com/docs/extensions/reference/manifest/icons) define the platform packaging.

## Artwork provenance

Created with the built-in imagegen tool on 2026-09-13. The first result had glow and was rejected. The selected result used this edit prompt:

> Edit this icon into flat production app artwork. Preserve the simple open lowercase-a loop silhouette, but REMOVE ALL blue glow, shadow, texture, gradients, lighting and dimensional effects. The whole 1024x1024 square canvas must be ONE uniform opaque cobalt blue #155EEF, all the way to all four edges. The mark must be ONE uniform solid white #FFFFFF shape with sharp antialiased edges and no outline. Scale the mark down so it occupies only the central 58% of the square and center it both horizontally and vertically. Exactly two flat colors total, blue background and white shape. No transparent background, no dark/black background, no rounded outer tile corners. Output only one finished app icon.

The generated file is 1254 pixels square and contains small raster color variations. The prompt records the design intent, not a claim that the model produced exact color values. Inspect the icon at native small sizes when changing the artwork. PNG decoding, ICO frames, ICNS representations and asset catalog references were checked on Windows. Apple, Android and Linux packaging still require their respective platform builds.
