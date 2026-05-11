//! Tauri commands exposed to the web frontend. These are the Rust ↔ JS
//! boundary; everything below them is plain Rust the daemon owns.

use linkpilot_core::browser::InstalledBrowser;
use linkpilot_core::protocol::DoctorReport;
use tauri::State;

use crate::AppState;

#[tauri::command]
pub fn doctor(state: State<'_, AppState>) -> Result<DoctorReport, String> {
    let inventory = state.platform.browser_inventory();
    let installed = inventory
        .installed_browsers()
        .map_err(|e| e.to_string())?
        .len();
    let is_default = state
        .platform
        .default_browser()
        .is_linkpilot_default()
        .unwrap_or(false);
    Ok(DoctorReport {
        daemon_version: env!("CARGO_PKG_VERSION").to_string(),
        is_default_browser: is_default,
        config_path: None,
        installed_browser_count: installed,
        ipc_socket_path: None,
    })
}

#[tauri::command]
pub fn list_browsers(state: State<'_, AppState>) -> Result<Vec<InstalledBrowser>, String> {
    state
        .platform
        .browser_inventory()
        .installed_browsers()
        .map_err(|e| e.to_string())
}
