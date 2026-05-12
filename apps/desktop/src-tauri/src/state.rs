//! Daemon-level state owned by the Tauri app.

use std::sync::{Arc, Mutex};

use linkpilot_core::config::store::RecommendedWatcherHandle;
use linkpilot_core::config::ConfigStore;
use linkpilot_core::history::RouteHistory;
use linkpilot_core::platform::PlatformProvider;

/// State stored in `tauri::State`. Cheap to clone — every field is either an
/// `Arc` or itself clone-shareable.
#[derive(Clone)]
pub struct AppState {
    pub config: ConfigStore,
    pub history: Arc<RouteHistory>,
    pub platform: Arc<dyn PlatformProvider>,
    watcher: Arc<Mutex<Option<RecommendedWatcherHandle>>>,
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
            watcher: Arc::new(Mutex::new(None)),
        }
    }

    /// Park the watcher inside the state so it lives as long as the daemon.
    pub fn attach_watcher(&self, handle: RecommendedWatcherHandle) {
        let mut guard = self.watcher.lock().expect("watcher mutex poisoned");
        *guard = Some(handle);
    }
}
