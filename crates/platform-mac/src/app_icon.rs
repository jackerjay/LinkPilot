//! Extract the .icns icon of a macOS application and convert it to a PNG.
//!
//! We deliberately avoid AppKit's NSWorkspace.icon(forFile:) here because
//! that path requires running on the main thread and a full Cocoa runtime
//! state, which is fragile when called from a Tauri command worker thread.
//! Instead we shell out to two well-known macOS binaries:
//!
//!   - `mdfind kMDItemCFBundleIdentifier == "…"` — Spotlight metadata
//!     query, resolves bundle id → .app path in ~10ms (cached by mds).
//!   - `sips -s format png -Z N`              — Image Services Tool, the
//!     official macOS icns → png converter that ships with the OS.
//!
//! Results are cached to disk by a stable key (the bundle id if known,
//! else a SHA-ish digest of the .app path) so the second lookup for the
//! same app is a single fs::read.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use linkpilot_core::platform::{PlatformError, Result};

/// Where the PNG cache lives. Co-located with the daemon's config so a
/// clean uninstall is a single rm -rf.
fn cache_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| PlatformError::Other("HOME not set".into()))?;
    let dir = PathBuf::from(home)
        .join("Library/Application Support/LinkPilot/icons");
    fs::create_dir_all(&dir).map_err(PlatformError::Io)?;
    Ok(dir)
}

/// Extract a 64pt PNG for the app identified by `bundle_id` OR an
/// explicit `app_path` (e.g. `/Applications/Google Chrome.app`). Returns
/// the on-disk path to the cached PNG. Callers should serve the bytes
/// from there.
pub fn ensure_png(
    bundle_id: Option<&str>,
    app_path: Option<&Path>,
    size: u32,
) -> Result<PathBuf> {
    let key = cache_key(bundle_id, app_path);
    let out = cache_dir()?.join(format!("{key}-{size}.png"));
    if out.exists() {
        return Ok(out);
    }

    let resolved = match app_path {
        Some(p) => p.to_path_buf(),
        None => {
            let bid = bundle_id.ok_or_else(|| {
                PlatformError::Other("ensure_png: need bundle_id or app_path".into())
            })?;
            resolve_app_path(bid)?
        }
    };
    let icns = locate_icns(&resolved)?;
    convert_icns_to_png(&icns, &out, size)?;
    Ok(out)
}

fn cache_key(bundle_id: Option<&str>, app_path: Option<&Path>) -> String {
    if let Some(b) = bundle_id {
        sanitize(b)
    } else if let Some(p) = app_path {
        sanitize(&p.to_string_lossy())
    } else {
        "unknown".into()
    }
}

/// File-system safe: keep alphanumerics, replace anything else with `_`.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' })
        .collect()
}

/// Spotlight-backed lookup. Returns the first matching `.app` URL.
fn resolve_app_path(bundle_id: &str) -> Result<PathBuf> {
    let out = Command::new("/usr/bin/mdfind")
        .arg(format!(
            "kMDItemCFBundleIdentifier == '{}'",
            bundle_id.replace('\'', "")
        ))
        .output()
        .map_err(PlatformError::Io)?;
    if !out.status.success() {
        return Err(PlatformError::Other(format!(
            "mdfind exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let first = stdout.lines().next().ok_or_else(|| {
        PlatformError::Other(format!("no app found for bundle id {bundle_id}"))
    })?;
    Ok(PathBuf::from(first))
}

/// Read `<app>/Contents/Info.plist` to find the icon file name, then
/// resolve it to an absolute path in Contents/Resources/.
fn locate_icns(app: &Path) -> Result<PathBuf> {
    let info_plist = app.join("Contents/Info.plist");
    if !info_plist.exists() {
        return Err(PlatformError::Other(format!(
            "no Info.plist at {}",
            info_plist.display()
        )));
    }
    // plutil's -extract returns the raw value of a key. We ask for json so
    // the output is a quoted string we can trivially strip.
    let out = Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleIconFile", "json", "-o", "-"])
        .arg(&info_plist)
        .output()
        .map_err(PlatformError::Io)?;
    if !out.status.success() {
        return Err(PlatformError::Other(format!(
            "plutil failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    // The value is JSON like "AppIcon" — strip the surrounding quotes.
    let name = raw.trim_matches('"');
    if name.is_empty() {
        return Err(PlatformError::Other("CFBundleIconFile empty".into()));
    }
    // macOS allows the value with or without the .icns extension.
    let with_ext = if name.ends_with(".icns") {
        name.to_string()
    } else {
        format!("{name}.icns")
    };
    let icns = app.join("Contents/Resources").join(&with_ext);
    if !icns.exists() {
        return Err(PlatformError::Other(format!(
            "icon file {} not found",
            icns.display()
        )));
    }
    Ok(icns)
}

fn convert_icns_to_png(icns: &Path, out: &Path, size: u32) -> Result<()> {
    let status = Command::new("/usr/bin/sips")
        .args(["-s", "format", "png", "-Z"])
        .arg(size.to_string())
        .arg(icns)
        .arg("--out")
        .arg(out)
        .output()
        .map_err(PlatformError::Io)?;
    if !status.status.success() {
        return Err(PlatformError::Other(format!(
            "sips failed: {}",
            String::from_utf8_lossy(&status.stderr).trim()
        )));
    }
    Ok(())
}
