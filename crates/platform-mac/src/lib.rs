//! macOS backend for LinkPilot's platform traits.

#![cfg(target_os = "macos")]

mod autostart;
mod default_browser;
mod inventory;
mod launcher;
mod notifier;
mod opener;

use linkpilot_core::platform::{
    Autostart, BrowserInventory, DefaultBrowserController, Notifier, OpenerDetector,
    PlatformProvider, UrlLauncher,
};

/// Bundle identifier baked into `tauri.conf.json`. Used as the fallback when
/// the caller doesn't pass one explicitly (e.g. CLI invocations).
pub const DEFAULT_BUNDLE_ID: &str = "app.linkpilot.desktop";

pub struct MacProvider {
    default_browser: default_browser::MacDefaultBrowser,
    inventory: inventory::MacInventory,
    launcher: launcher::MacUrlLauncher,
    autostart: autostart::MacAutostart,
    notifier: notifier::MacNotifier,
    opener: opener::MacOpenerDetector,
}

impl MacProvider {
    /// Construct a provider rooted at the given bundle identifier (the value
    /// of `CFBundleIdentifier` in the running `.app`).
    pub fn new(bundle_id: impl Into<String>) -> Self {
        let bundle_id = bundle_id.into();
        Self {
            default_browser: default_browser::MacDefaultBrowser::new(bundle_id.clone()),
            inventory: inventory::MacInventory,
            launcher: launcher::MacUrlLauncher,
            autostart: autostart::MacAutostart::new(bundle_id),
            notifier: notifier::MacNotifier,
            opener: opener::MacOpenerDetector,
        }
    }
}

impl Default for MacProvider {
    fn default() -> Self {
        Self::new(DEFAULT_BUNDLE_ID)
    }
}

impl PlatformProvider for MacProvider {
    fn default_browser(&self) -> &dyn DefaultBrowserController {
        &self.default_browser
    }
    fn browser_inventory(&self) -> &dyn BrowserInventory {
        &self.inventory
    }
    fn url_launcher(&self) -> &dyn UrlLauncher {
        &self.launcher
    }
    fn autostart(&self) -> &dyn Autostart {
        &self.autostart
    }
    fn notifier(&self) -> &dyn Notifier {
        &self.notifier
    }
    fn opener_detector(&self) -> &dyn OpenerDetector {
        &self.opener
    }
}
