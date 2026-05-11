//! macOS backend for LinkPilot's platform traits.
//!
//! v0.1 step 1 wires up the module layout so other crates can already depend
//! on `MacProvider`. Real Cocoa / `LaunchServices` calls land in steps 5–6.

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

pub struct MacProvider {
    default_browser: default_browser::MacDefaultBrowser,
    inventory: inventory::MacInventory,
    launcher: launcher::MacUrlLauncher,
    autostart: autostart::MacAutostart,
    notifier: notifier::MacNotifier,
    opener: opener::MacOpenerDetector,
}

impl MacProvider {
    pub fn new() -> Self {
        Self {
            default_browser: default_browser::MacDefaultBrowser,
            inventory: inventory::MacInventory,
            launcher: launcher::MacUrlLauncher,
            autostart: autostart::MacAutostart,
            notifier: notifier::MacNotifier,
            opener: opener::MacOpenerDetector,
        }
    }
}

impl Default for MacProvider {
    fn default() -> Self {
        Self::new()
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
