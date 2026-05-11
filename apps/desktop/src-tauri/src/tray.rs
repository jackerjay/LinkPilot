//! Menu bar / system tray. The visible status entry-point. Real menu items
//! (Pause routing, Recent routes, Quit) land alongside the menu-bar page.

use tauri::AppHandle;

pub fn install(_app: &AppHandle) -> tauri::Result<()> {
    // v0.1 step 1 scaffold: Tauri's default tray icon is created automatically
    // from `tauri.conf.json -> app.trayIcon`. Wiring menu items happens once
    // the menu-bar page is implemented.
    Ok(())
}
