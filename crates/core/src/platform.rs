//! Platform abstraction traits. Concrete implementations live in
//! `linkpilot-platform-{mac,win,linux}` crates.
//!
//! v0.1 ships only mac as a real backend. Other platforms must still expose
//! a [`PlatformProvider`] (returning `NotSupported`) so the workspace builds
//! everywhere — this is the contract that keeps Windows / Linux ports cheap.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::browser::{BrowserId, BrowserProfile, BrowserTarget, InstalledBrowser};

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("not supported on this platform")]
    NotSupported,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, PlatformError>;

/// Single entry point each platform crate must provide.
pub trait PlatformProvider: Send + Sync {
    fn default_browser(&self) -> &dyn DefaultBrowserController;
    fn browser_inventory(&self) -> &dyn BrowserInventory;
    fn url_launcher(&self) -> &dyn UrlLauncher;
    fn autostart(&self) -> &dyn Autostart;
    fn notifier(&self) -> &dyn Notifier;
    fn opener_detector(&self) -> &dyn OpenerDetector;
}

/// Manage LinkPilot's "is this the system default browser?" state.
pub trait DefaultBrowserController: Send + Sync {
    fn current_default(&self) -> Result<Option<BrowserId>>;
    fn is_linkpilot_default(&self) -> Result<bool>;
    fn request_set_default(&self) -> Result<SetDefaultOutcome>;
}

/// Result of asking the OS to set LinkPilot as default.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SetDefaultOutcome {
    Done,
    /// Windows: opens Settings → Default apps. UI shows guidance.
    UserConsentRequired {
        instructions_url: Option<String>,
    },
    NotSupported,
}

/// Discover installed browsers and their profiles.
pub trait BrowserInventory: Send + Sync {
    fn installed_browsers(&self) -> Result<Vec<InstalledBrowser>>;
    fn profiles(&self, browser: &BrowserId) -> Result<Vec<BrowserProfile>>;
}

/// Launch a URL into a concrete browser + profile.
pub trait UrlLauncher: Send + Sync {
    fn open(&self, target: &BrowserTarget, url: &Url) -> Result<()>;
}

/// Manage the "open at login" setting.
pub trait Autostart: Send + Sync {
    fn is_enabled(&self) -> Result<bool>;
    fn set_enabled(&self, on: bool) -> Result<()>;
}

/// System notifications.
pub trait Notifier: Send + Sync {
    fn toast(&self, title: &str, body: &str) -> Result<()>;
}

/// Identify which app triggered an `open URL` event.
pub trait OpenerDetector: Send + Sync {
    fn detect(&self, hint: &OpenEventHint) -> Option<OpenerApp>;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenEventHint {
    pub bundle_id: Option<String>,
    pub pid: Option<i32>,
    pub timestamp_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenerApp {
    pub name: String,
    pub bundle_id: Option<String>,
    pub executable: Option<PathBuf>,
    pub pid: Option<i32>,
}

/// A stub provider used by the workspace on platforms where no real backend
/// has been written yet (Windows / Linux in v0.1). Every call returns
/// [`PlatformError::NotSupported`]; this lets `cargo check --workspace`
/// succeed everywhere.
pub struct StubProvider;

impl PlatformProvider for StubProvider {
    fn default_browser(&self) -> &dyn DefaultBrowserController {
        self
    }
    fn browser_inventory(&self) -> &dyn BrowserInventory {
        self
    }
    fn url_launcher(&self) -> &dyn UrlLauncher {
        self
    }
    fn autostart(&self) -> &dyn Autostart {
        self
    }
    fn notifier(&self) -> &dyn Notifier {
        self
    }
    fn opener_detector(&self) -> &dyn OpenerDetector {
        self
    }
}

impl DefaultBrowserController for StubProvider {
    fn current_default(&self) -> Result<Option<BrowserId>> {
        Ok(None)
    }
    fn is_linkpilot_default(&self) -> Result<bool> {
        Ok(false)
    }
    fn request_set_default(&self) -> Result<SetDefaultOutcome> {
        Ok(SetDefaultOutcome::NotSupported)
    }
}

impl BrowserInventory for StubProvider {
    fn installed_browsers(&self) -> Result<Vec<InstalledBrowser>> {
        Ok(Vec::new())
    }
    fn profiles(&self, _browser: &BrowserId) -> Result<Vec<BrowserProfile>> {
        Ok(Vec::new())
    }
}

impl UrlLauncher for StubProvider {
    fn open(&self, _target: &BrowserTarget, _url: &Url) -> Result<()> {
        Err(PlatformError::NotSupported)
    }
}

impl Autostart for StubProvider {
    fn is_enabled(&self) -> Result<bool> {
        Ok(false)
    }
    fn set_enabled(&self, _on: bool) -> Result<()> {
        Err(PlatformError::NotSupported)
    }
}

impl Notifier for StubProvider {
    fn toast(&self, _title: &str, _body: &str) -> Result<()> {
        Ok(())
    }
}

impl OpenerDetector for StubProvider {
    fn detect(&self, _hint: &OpenEventHint) -> Option<OpenerApp> {
        None
    }
}
