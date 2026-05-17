//! Tauri shell that hosts the LinkPilot daemon.
//!
//! v0.1: the Rust side owns the router, the config store, route history, and
//! the fsnotify watcher; the web frontend talks to it via [`tauri::command`].
//! macOS URL events arrive through `tauri-plugin-deep-link`.

mod commands;
mod dispatch;
mod ipc_host;
mod nmh_supervisor;
mod picker;
mod state;
mod tray;
mod url_handler;

use std::sync::Arc;

use linkpilot_core::config::{default_config_path, ConfigStore};
use linkpilot_core::history::RouteHistory;
use linkpilot_core::platform::PlatformProvider;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_deep_link::DeepLinkExt;

pub use state::{AppState, DaemonMode};

fn probe_existing_daemon(endpoint: &linkpilot_core::endpoint::Endpoint) -> bool {
    use linkpilot_core::protocol::Request;
    let req = Request::StatePing {
        request_id: "gui-startup-probe".into(),
    };
    linkpilot_ipc::client::send(endpoint, req).is_ok()
}

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
            let config_path =
                default_config_path().map_err(|e| anyhow::anyhow!("resolve config path: {e}"))?;
            let (config_store, created) = ConfigStore::load_or_init(config_path.clone())
                .map_err(|e| anyhow::anyhow!("load config {}: {e}", config_path.display()))?;
            if created {
                tracing::info!(path = %config_path.display(), "wrote first-run config");
            }

            let history = Arc::new(RouteHistory::new());
            let bundle_id = app.config().identifier.clone();
            let platform = make_platform(bundle_id);

            // DaemonRuntime is the in-process daemon's state — same logic the
            // headless `linkpilot-daemon` binary uses. Shared via Arc so the
            // ipc_host handler (below) and AppState's Tauri commands both see
            // identical config/history snapshots.
            let runtime = Arc::new(linkpilot_core::daemon::DaemonRuntime::new(
                config_store.clone(),
                Arc::clone(&history),
                Arc::clone(&platform),
                env!("CARGO_PKG_VERSION"),
            ));

            let state = AppState::new(
                config_store.clone(),
                Arc::clone(&history),
                Arc::clone(&platform),
            );

            // fsnotify: rebroadcast every config change to the front-end.
            //
            // Both origins emit — frontends that mutate via IPC also
            // listen on `config-changed` to refresh views they don't
            // own (sidebar workspace dots, WorkspacePage, etc.). The
            // origin string distinguishes "I caused this" from "an
            // external editor caused this" should a consumer want to
            // skip its own echo, but for now everyone just refetches
            // — the read is cheap and the emit is debounced by the
            // watcher itself, so there's no risk of a feedback loop
            // (IPC reads don't write, so they don't re-fire the watch).
            let app_handle = app.handle().clone();
            let watcher = config_store
                .watch(move |origin| {
                    let label = match origin {
                        linkpilot_core::config::store::ChangeOrigin::External => "external",
                        linkpilot_core::config::store::ChangeOrigin::Echo => "echo",
                    };
                    let _ = app_handle.emit("config-changed", label);
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

            // v0.2 daemon coexistence:
            //   - If `linkpilot-daemon` is already running on the socket, the
            //     GUI runs in "client mode": skip the IPC server bind so we
            //     don't fight the daemon for the socket. The GUI's local
            //     ConfigStore + fsnotify still work — they read/write the
            //     same on-disk config file the daemon does, and the anti-echo
            //     token keeps both sides in sync.
            //   - Otherwise (no external daemon) we behave like v0.1: the
            //     GUI itself hosts the daemon (route_open from `lp`, ipc-host
            //     handler, etc.).
            // Inspector history is degraded in client mode (we don't see the
            // daemon's in-memory history); M3 wires `lp history` through to
            // the daemon and the GUI can call the same path.
            let endpoint = linkpilot_ipc::path::default_endpoint();
            let daemon_mode = if probe_existing_daemon(&endpoint) {
                tracing::info!(
                    endpoint = %endpoint.display(),
                    "external linkpilot-daemon detected; GUI running in client mode"
                );
                DaemonMode::External
            } else {
                let handler = std::sync::Arc::new(ipc_host::DaemonHandler::new(
                    Arc::clone(&runtime),
                    state.clone(),
                    app.handle().clone(),
                ));
                match linkpilot_ipc::server::serve(endpoint, handler) {
                    Ok(ipc) => {
                        state.attach_ipc(ipc);
                        DaemonMode::InProcess
                    }
                    Err(err) => {
                        tracing::warn!(?err, "ipc server failed to start; GUI still works");
                        DaemonMode::InProcess
                    }
                }
            };
            state.set_daemon_mode(daemon_mode);

            tray::install(&app.handle())?;
            app.manage(state);
            // Browser-picker state for the Ask UI (see picker.rs).
            app.manage(picker::PickerState::default());

            // Show the main window once on launch so the first-time user can
            // see the GUI without having to find the menu-bar icon. The tray
            // / Dock reopen handlers below keep it accessible after the user
            // closes it.
            tray::show_main_window(&app.handle());

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                // Main window: hide instead of destroy so the tray /
                // Dock reopen handlers can show it again.
                WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                // Picker window dismissed via the close button (rare —
                // it has no chrome). Resolve the in-flight ask as
                // cancelled so the dispatch thread doesn't time out.
                WindowEvent::CloseRequested { .. } if window.label() == "picker" => {
                    let state: tauri::State<picker::PickerState> = window.app_handle().state();
                    picker::picker_resolve(state, None);
                }
                // Tray popover: dismiss when the user clicks elsewhere.
                // Matches macOS menu-bar popover convention (Bartender,
                // Stats, the system Wi-Fi menu). Stamp the hide so the
                // tray-icon click handler doesn't immediately re-show
                // (focus-lost arrives before click on macOS).
                WindowEvent::Focused(false) if window.label() == "tray" => {
                    let _ = window.hide();
                    tray::note_popover_hidden(window.app_handle());
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::config_get,
            commands::config_replace,
            commands::rule_upsert,
            commands::rule_delete,
            commands::workspace_upsert,
            commands::workspace_delete,
            commands::workspace_set_enabled,
            commands::set_smart_routing,
            commands::doctor,
            commands::list_browsers,
            commands::add_custom_browser,
            commands::remove_custom_browser,
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
            commands::cli_install_status,
            commands::cli_install_to_path,
            picker::picker_session,
            picker::picker_resolve,
            tray::tray_open_main,
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
