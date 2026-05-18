//! Daemon-level state owned by the Tauri app.

use std::sync::{Arc, Mutex};

use linkpilot_core::config::store::RecommendedWatcherHandle;
use linkpilot_core::config::ConfigStore;
use linkpilot_core::history::RouteHistory;
use linkpilot_core::platform::PlatformProvider;
use linkpilot_ipc::server::ServerHandle;

/// Where the daemon work actually lives in this process. v0.2 splits the
/// daemon out of the Tauri shell — if `linkpilot-daemon` is already
/// running when the GUI launches, the GUI becomes a client of it
/// (`External`) and skips its own IPC server bind. Otherwise the GUI
/// hosts the daemon itself (`InProcess`), matching v0.1 behaviour.
///
/// Surfaced to the frontend via `daemon_service_status` (Settings
/// "Background service" card) — see commands/mod.rs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DaemonMode {
    #[default]
    InProcess,
    External,
}

/// State stored in `tauri::State`. Cheap to clone — every field is either an
/// `Arc` or itself clone-shareable.
#[derive(Clone)]
pub struct AppState {
    pub config: ConfigStore,
    pub history: Arc<RouteHistory>,
    pub platform: Arc<dyn PlatformProvider>,
    /// Long-lived background handles parked here so they outlive setup.
    handles: Arc<Mutex<Handles>>,
}

#[derive(Default)]
struct Handles {
    watcher: Option<RecommendedWatcherHandle>,
    ipc: Option<ServerHandle>,
    daemon_mode: DaemonMode,
}

impl AppState {
    pub fn new(
        config: ConfigStore,
        history: Arc<RouteHistory>,
        platform: Arc<dyn PlatformProvider>,
    ) -> Self {
        Self {
            config,
            history,
            platform,
            handles: Arc::new(Mutex::new(Handles::default())),
        }
    }

    /// Park the fsnotify watcher so it lives as long as the daemon.
    pub fn attach_watcher(&self, handle: RecommendedWatcherHandle) {
        let mut guard = self.handles.lock().expect("handles mutex poisoned");
        guard.watcher = Some(handle);
    }

    /// Park the IPC server handle.
    pub fn attach_ipc(&self, handle: ServerHandle) {
        let mut guard = self.handles.lock().expect("handles mutex poisoned");
        guard.ipc = Some(handle);
    }

    pub fn set_daemon_mode(&self, mode: DaemonMode) {
        let mut guard = self.handles.lock().expect("handles mutex poisoned");
        guard.daemon_mode = mode;
    }

    pub fn daemon_mode(&self) -> DaemonMode {
        self.handles
            .lock()
            .map(|g| g.daemon_mode)
            .unwrap_or(DaemonMode::InProcess)
    }
}
