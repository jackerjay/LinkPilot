use std::path::PathBuf;

use linkpilot_core::browser::{BrowserId, BrowserKind, BrowserProfile, InstalledBrowser};
use linkpilot_core::inventory::{parse_chromium_profiles, parse_firefox_profiles};
use linkpilot_core::platform::{BrowserInventory, PlatformError, Result};

pub struct MacInventory;

/// Static registry of browsers LinkPilot knows how to drive on macOS.
///
/// Adding a browser is a single row here plus (potentially) a launcher arm.
struct KnownBrowser {
    id: &'static str,
    display_name: &'static str,
    kind: BrowserKind,
    bundle_id: &'static str,
    /// Bundle directory name under `/Applications` or `~/Applications`.
    app_dir_name: &'static str,
    /// Executable name inside `Contents/MacOS/`.
    binary: &'static str,
    /// Profile root, relative to `~/Library/Application Support/`.
    /// `None` for Safari (no profile concept until macOS 14 introduced its
    /// limited form, which has no usable external API for v0.1).
    profile_root_rel: Option<&'static str>,
}

const REGISTRY: &[KnownBrowser] = &[
    KnownBrowser {
        id: "chrome",
        display_name: "Google Chrome",
        kind: BrowserKind::Chromium,
        bundle_id: "com.google.Chrome",
        app_dir_name: "Google Chrome.app",
        binary: "Google Chrome",
        profile_root_rel: Some("Google/Chrome"),
    },
    KnownBrowser {
        id: "edge",
        display_name: "Microsoft Edge",
        kind: BrowserKind::Chromium,
        bundle_id: "com.microsoft.edgemac",
        app_dir_name: "Microsoft Edge.app",
        binary: "Microsoft Edge",
        profile_root_rel: Some("Microsoft Edge"),
    },
    KnownBrowser {
        id: "brave",
        display_name: "Brave Browser",
        kind: BrowserKind::Chromium,
        bundle_id: "com.brave.Browser",
        app_dir_name: "Brave Browser.app",
        binary: "Brave Browser",
        profile_root_rel: Some("BraveSoftware/Brave-Browser"),
    },
    KnownBrowser {
        id: "arc",
        display_name: "Arc",
        kind: BrowserKind::Arc,
        bundle_id: "company.thebrowser.Browser",
        app_dir_name: "Arc.app",
        binary: "Arc",
        profile_root_rel: Some("Arc/User Data"),
    },
    KnownBrowser {
        id: "firefox",
        display_name: "Firefox",
        kind: BrowserKind::Firefox,
        bundle_id: "org.mozilla.firefox",
        app_dir_name: "Firefox.app",
        binary: "firefox",
        profile_root_rel: Some("Firefox"),
    },
    KnownBrowser {
        id: "safari",
        display_name: "Safari",
        kind: BrowserKind::Safari,
        bundle_id: "com.apple.Safari",
        app_dir_name: "Safari.app",
        binary: "Safari",
        profile_root_rel: None,
    },
];

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join("Applications"));
    }
    dirs
}

fn application_support_root() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support"))
}

fn find_app_path(app_dir: &str) -> Option<PathBuf> {
    search_dirs()
        .into_iter()
        .map(|base| base.join(app_dir))
        .find(|p| p.exists())
}

fn lookup(id: &BrowserId) -> Option<&'static KnownBrowser> {
    REGISTRY.iter().find(|e| e.id == id.0.as_str())
}

impl BrowserInventory for MacInventory {
    fn installed_browsers(&self) -> Result<Vec<InstalledBrowser>> {
        let asroot = application_support_root();
        let mut out = Vec::new();
        for entry in REGISTRY {
            let Some(app_path) = find_app_path(entry.app_dir_name) else {
                continue;
            };
            let exe = app_path.join("Contents").join("MacOS").join(entry.binary);
            let profile_root = entry
                .profile_root_rel
                .and_then(|rel| asroot.as_ref().map(|r| r.join(rel)));
            out.push(InstalledBrowser {
                id: BrowserId::new(entry.id),
                display_name: entry.display_name.to_string(),
                kind: entry.kind,
                executable: exe,
                platform_app_id: Some(entry.bundle_id.to_string()),
                profile_root,
            });
        }
        Ok(out)
    }

    fn profiles(&self, browser: &BrowserId) -> Result<Vec<BrowserProfile>> {
        let Some(entry) = lookup(browser) else {
            return Ok(Vec::new());
        };
        let Some(rel) = entry.profile_root_rel else {
            return Ok(Vec::new());
        };
        let Some(asroot) = application_support_root() else {
            return Ok(Vec::new());
        };
        let root = asroot.join(rel);
        if !root.exists() {
            return Ok(Vec::new());
        }
        match entry.kind {
            BrowserKind::Chromium | BrowserKind::Arc => {
                parse_chromium_profiles(&root).map_err(PlatformError::Io)
            }
            BrowserKind::Firefox => {
                let ini = root.join("profiles.ini");
                if !ini.exists() {
                    return Ok(Vec::new());
                }
                parse_firefox_profiles(&ini).map_err(PlatformError::Io)
            }
            BrowserKind::Safari | BrowserKind::Unknown => Ok(Vec::new()),
        }
    }
}
