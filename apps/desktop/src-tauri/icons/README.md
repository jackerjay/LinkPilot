# Icons

Real artwork shipped in v0.1.

| File | Source | Purpose |
|------|--------|---------|
| `icon.png`, `icon.icns`, `icon.ico`, `32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, `Square*Logo.png`, `StoreLogo.png`, `android/`, `ios/` | `npx tauri icon ../../docs/brand/icon.png` | Bundle icons for every platform target (macOS, iOS, Android, Windows, Linux). |
| `tray.png`, `tray@2x.png`, `tray@3x.png` | `rsvg-convert` from `../../../docs/brand/tray-template.svg` | macOS menu-bar template image. `iconAsTemplate: true` in `tauri.conf.json` — render in black-on-transparent only; the system tints for active/inactive + light/dark. |

## Regenerating

```sh
# 1. Re-pad the master so the artwork is inside Apple's ~80% safe area
#    (skip if your source already has transparent padding around the art).
cargo run -p linkpilot-icon-padder --release -- \
  docs/brand/icon-raw.png docs/brand/icon.png

# 2. Full bundle matrix (uses the padded 1254×1254 brand mark):
cd apps/desktop
npx tauri icon ../../docs/brand/icon.png

# Menu-bar template (uses the simplified single-color SVG):
cd ../..
rsvg-convert -w 22 -h 22 docs/brand/tray-template.svg \
  -o apps/desktop/src-tauri/icons/tray.png
rsvg-convert -w 44 -h 44 docs/brand/tray-template.svg \
  -o apps/desktop/src-tauri/icons/tray@2x.png
rsvg-convert -w 66 -h 66 docs/brand/tray-template.svg \
  -o apps/desktop/src-tauri/icons/tray@3x.png
```

`rsvg-convert` ships with `librsvg` (`brew install librsvg`).

Edit `docs/brand/tray-template.svg` to tweak the menu-bar mark — keep it
**single-color (black) on transparent** so macOS templateImage tinting
works. The full brand mark in `docs/brand/icon.png` is colour and would
render as a solid blob in the menu bar if used directly.
