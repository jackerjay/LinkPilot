//! Tauri shell that hosts the LinkPilot daemon.
//!
//! v0.1: the Rust side owns the router, the config store, route history, and
//! the fsnotify watcher; the web frontend talks to it via [`tauri::command`].
//! macOS URL events arrive through `tauri-plugin-deep-link`.

mod commands;
mod dispatch;
mod ipc_host;
mod nmh_supervisor;
mod state;
mod tray;
mod url_handler;

use std::sync::Arc;

use linkpilot_core::config::{default_config_path, ConfigStore};
use linkpilot_core::history::RouteHistory;
use linkpilot_core::platform::PlatformProvider;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_deep_link::DeepLinkExt;

pub use state::AppState;

#[cfg(target_os = "macos")]
fn make_platform(bundle_id: String) -> Arc<dyn PlatformProvider> {
    Arc::new(linkpilot_platform_mac::MacProvider::new(bundle_id))
}

#[cfg(not(target_os = "macos"))]
fn make_platform(_bundle_id: String) -> Arc<dyn PlatformProvider> {
    Arc::new(linkpilot_core::platform::StubProvider)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,linkpilot=debug")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_path = default_config_path()
                .map_err(|e| anyhow::anyhow!("resolve config path: {e}"))?;
            let (config_store, created) = ConfigStore::load_or_init(config_path.clone())
                .map_err(|e| anyhow::anyhow!("load config {}: {e}", config_path.display()))?;
            if created {
                tracing::info!(path = %config_path.display(), "wrote first-run config");
            }

            let history = Arc::new(RouteHistory::new());
            let bundle_id = app.config().identifier.clone();
            let platform = make_platform(bundle_id);

            let state = AppState::new(config_store.clone(), Arc::clone(&history), platform);

            // fsnotify: rebroadcast external edits to the front-end.
            let app_handle = app.handle().clone();
            let watcher = config_store
                .watch(move |origin| match origin {
                    linkpilot_core::config::store::ChangeOrigin::External => {
                        let _ = app_handle.emit("config-changed", "external");
                    }
                    linkpilot_core::config::store::ChangeOrigin::Echo => {}
                })
                .map_err(|e| anyhow::anyhow!("watch config: {e}"))?;
            state.attach_watcher(watcher);

            // macOS / Linux: incoming http(s) URLs from `open` events.
            let url_state = state.clone();
            let url_handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    url_handler::dispatch_system_url(&url_state, &url_handle, url.to_string());
                }
            });

            // IPC server: `lp` and (future) Native Host attach here.
            let handler = std::sync::Arc::new(ipc_host::DaemonHandler::new(
                state.clone(),
                app.handle().clone(),
            ));
            match linkpilot_ipc::server::serve(linkpilot_ipc::path::default_endpoint(), handler) {
                Ok(ipc) => state.attach_ipc(ipc),
                Err(err) => tracing::warn!(?err, "ipc server failed to start; GUI still works"),
            }

            tray::install(&app.handle())?;
            app.manage(state);

            // Show the main window once on launch so the first-time user can
            // see the GUI without having to find the menu-bar icon. The tray
            // / Dock reopen handlers below keep it accessible after the user
            // closes it.
            tray::show_main_window(&app.handle());

            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window must hide it, not destroy it — the
            // tray + Dock reopen handlers need a live window to show.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::config_get,
            commands::config_replace,
            commands::rule_upsert,
            commands::rule_delete,
            commands::doctor,
            commands::list_browsers,
            commands::list_profiles,
            commands::route_open,
            commands::route_evaluate,
            commands::route_history,
            commands::is_default_browser,
            commands::request_set_default_browser,
            commands::import_config,
            commands::export_config,
            commands::app_icon,
            commands::pick_app,
        ])
        .build(tauri::generate_context!())
        .expect("error while building LinkPilot")
        .run(|app, event| {
            // macOS: clicking the Dock icon after the window is hidden fires
            // Reopen — bring the main window back instead of being silent.
            #[cfg(target_os = "macos")]
            if let RunEvent::Reopen { .. } = event {
                tray::show_main_window(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
