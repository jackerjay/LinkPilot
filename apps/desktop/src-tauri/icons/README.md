# Icons

1×1 transparent placeholders to satisfy `tauri-build`. **Real artwork:** the
LinkPilot P-compass icon. To install:

```sh
# Save the source PNG as ~/Downloads/icon.png (≥1024×1024), then:
cd apps/desktop
cargo tauri icon ~/Downloads/icon.png
```

`cargo tauri icon` regenerates the full matrix (`icon.png`, `icon.icns`,
`icon.ico`, `32x32.png`, `128x128.png`, `128x128@2x.png`, …) for every
platform. Drop a 22×22 alpha template at `tray.png` for the menu-bar icon
(macOS menu-bar wants single-channel template images).

Source artwork (SVG) lives at `../../../docs/brand/` once it exists.
