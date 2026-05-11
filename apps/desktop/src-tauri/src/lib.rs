//! Tauri shell that hosts the LinkPilot daemon.
//!
//! In v0.1 this app is the daemon: the Rust side owns the router, the config
//! store, and (later) the IPC server; the web frontend is just a client over
//! `tauri::command`.

mod commands;
mod nmh_supervisor;
mod tray;
mod url_handler;

use tauri::Manager;

/// Construct the runtime [`PlatformProvider`] for the current target.
#[cfg(target_os = "macos")]
fn make_platform() -> Box<dyn linkpilot_core::platform::PlatformProvider> {
    Box::new(linkpilot_platform_mac::MacProvider::new())
}

#[cfg(not(target_os = "macos"))]
fn make_platform() -> Box<dyn linkpilot_core::platform::PlatformProvider> {
    Box::new(linkpilot_core::platform::StubProvider)
}

/// Daemon-level state stored in Tauri's managed state.
pub struct AppState {
    pub platform: Box<dyn linkpilot_core::platform::PlatformProvider>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let state = AppState {
                platform: make_platform(),
            };
            app.manage(state);
            tray::install(&app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::doctor,
            commands::list_browsers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running LinkPilot");
}
