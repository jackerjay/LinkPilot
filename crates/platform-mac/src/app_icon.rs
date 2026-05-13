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

/// Extract a 64pt PNG for the app identified by `bundle_id`, an explicit
/// `app_path` (e.g. `/Applications/Google Chrome.app`), or — as a last
/// resort — a Spotlight-resolvable display / bundle name (e.g. "Slack").
/// Returns the on-disk path to the cached PNG.
pub fn ensure_png(
    bundle_id: Option<&str>,
    app_path: Option<&Path>,
    name: Option<&str>,
    size: u32,
) -> Result<PathBuf> {
    let key = cache_key(bundle_id, app_path, name);
    let out = cache_dir()?.join(format!("{key}-{size}.png"));
    if out.exists() {
        return Ok(out);
    }

    let resolved = match (bundle_id, app_path, name) {
        (_, Some(p), _) => p.to_path_buf(),
        (Some(bid), None, _) => resolve_app_path(bid)?,
        (None, None, Some(n)) => resolve_app_path_by_name(n)?,
        _ => {
            return Err(PlatformError::Other(
                "ensure_png: need bundle_id, app_path, or name".into(),
            ))
        }
    };
    let icns = locate_icns(&resolved)?;
    convert_icns_to_png(&icns, &out, size)?;
    Ok(out)
}

fn cache_key(
    bundle_id: Option<&str>,
    app_path: Option<&Path>,
    name: Option<&str>,
) -> String {
    if let Some(b) = bundle_id {
        sanitize(b)
    } else if let Some(p) = app_path {
        sanitize(&p.to_string_lossy())
    } else if let Some(n) = name {
        format!("name-{}", sanitize(n))
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

/// Resolve a .app by its localized display name. Tries the cheap path
/// first and only falls back to Spotlight when filesystem probing fails.
///
/// Why the fallback chain: Spotlight's `kMDItemDisplayName` /
/// `kMDItemCFBundleName` aren't reliably populated for some apps
/// (Electron-based ones in particular — Lark indexes neither). The
/// `kMDItemFSName` attribute is much more dependable because Spotlight
/// always indexes the filename.
fn resolve_app_path_by_name(name: &str) -> Result<PathBuf> {
    // 1. /Applications/{Name}.app — the overwhelming common case.
    let primary = PathBuf::from("/Applications").join(format!("{name}.app"));
    if primary.exists() {
        return Ok(primary);
    }
    // 2. ~/Applications/{Name}.app — user-scoped install.
    if let Some(home) = std::env::var_os("HOME") {
        let user_path = PathBuf::from(home)
            .join("Applications")
            .join(format!("{name}.app"));
        if user_path.exists() {
            return Ok(user_path);
        }
    }
    // 3. Spotlight by file system name. Works even for apps that don't
    //    surface their display name in mds metadata.
    if let Some(path) = mdfind_first(&format!("kMDItemFSName == \"{name}.app\"")) {
        return Ok(path);
    }
    // 4. Last resort: localized display name OR CFBundleName.
    let safe = name.replace('"', "");
    let by_meta = format!(
        "(kMDItemContentType == 'com.apple.application-bundle') && \
         ((kMDItemDisplayName == \"{safe}\") || (kMDItemCFBundleName == \"{safe}\"))"
    );
    if let Some(path) = mdfind_first(&by_meta) {
        return Ok(path);
    }
    Err(PlatformError::Other(format!("no app found for name {name}")))
}

/// Run `mdfind <query>` and return the most relevant hit. Prefers
/// /Applications over user / system locations when there are multiple.
fn mdfind_first(query: &str) -> Option<PathBuf> {
    let out = Command::new("/usr/bin/mdfind").arg(query).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout
        .lines()
        .find(|l| l.starts_with("/Applications/"))
        .or_else(|| stdout.lines().next())
        .map(PathBuf::from)
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
    // Use `raw` format, not `json`: plutil's json mode requires the
    // extracted subtree to be a top-level array/object, so a bare string
    // value like CFBundleIconFile = "AppIcon" makes it bail with
    // "Invalid object in plist for JSON format". `raw` prints just the
    // string content with no quoting, which is exactly what we want.
    let out = Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleIconFile", "raw", "-o", "-"])
        .arg(&info_plist)
        .output()
        .map_err(PlatformError::Io)?;
    if !out.status.success() {
        return Err(PlatformError::Other(format!(
            "plutil failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
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
