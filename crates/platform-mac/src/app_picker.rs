//! Native macOS application picker for "Add manually" in the Browsers
//! page.
//!
//! Implementation note: we deliberately use AppleScript's `choose file`
//! (Finder-style file browser filtered to `.app` bundles) rather than
//! `choose application`. The latter only enumerates apps in
//! LaunchServices' application registry — niche browsers, dev builds,
//! sideloaded apps, and apps that LS hasn't reindexed yet are
//! invisible from there. `choose file` lets the user pick any `.app`
//! bundle anywhere on disk (including network volumes), which is what
//! "Browse..." in the system app picker does too.
//!
//! Bundle metadata (id + display name) is read out of the bundle's
//! Info.plist after the user picks. Using `defaults read` keeps us
//! AppKit-free; an alternative would be CoreFoundation via objc2 but
//! the plumbing isn't worth it for two scalar reads.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickedApp {
    pub name: String,
    pub bundle_id: String,
    /// POSIX path to the `.app` bundle (e.g. `/Applications/Foo.app`).
    /// Empty when the chooser couldn't resolve a path — caller falls
    /// back to `open -a <name>`.
    #[serde(default)]
    pub app_path: String,
}

// `choose file` opens the Finder-shaped file browser; `com.apple
// .application-bundle` filters it to `.app` packages. `default
// location` (set to /Applications) gives the user a sane starting
// point; they can navigate anywhere from there.
const APPLESCRIPT: &str = r#"
set theApp to choose file with prompt "Select an application" of type {"com.apple.application-bundle"} default location (POSIX file "/Applications")
return POSIX path of theApp
"#;

/// Show the native file chooser filtered to `.app` bundles. Returns
/// `Ok(None)` when the user cancels.
pub fn choose_app() -> Result<Option<PickedApp>, String> {
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", APPLESCRIPT])
        .output()
        .map_err(|e| format!("osascript spawn: {e}"))?;

    if !output.status.success() {
        // Cancel → osascript exits 1 with "User canceled. (-128)" on
        // stderr. Real failures (missing osascript, sandbox denial)
        // are rare; treat any non-success as a user dismiss so we
        // don't surface noise.
        return Ok(None);
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Ok(None);
    }

    // Read Info.plist for the bundle id + display name. Falling back
    // to the path basename keeps "name" populated even for bundles
    // with weird Info.plist schemas.
    let info_plist = format!("{}/Contents/Info", path.trim_end_matches('/'));

    let bundle_id = read_plist_key(&info_plist, "CFBundleIdentifier").unwrap_or_default();

    // Display name precedence mirrors what Finder picks: localized
    // bundle name > bundle name > basename. CFBundleDisplayName is
    // user-facing (overridable by localization); CFBundleName is the
    // build-time name; basename is the absolute fallback.
    let name = read_plist_key(&info_plist, "CFBundleDisplayName")
        .or_else(|| read_plist_key(&info_plist, "CFBundleName"))
        .unwrap_or_else(|| basename_without_app(&path));

    Ok(Some(PickedApp {
        name,
        bundle_id,
        app_path: path,
    }))
}

fn read_plist_key(info_plist_path: &str, key: &str) -> Option<String> {
    let out = Command::new("/usr/bin/defaults")
        .args(["read", info_plist_path, key])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn basename_without_app(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Application".to_string())
}
