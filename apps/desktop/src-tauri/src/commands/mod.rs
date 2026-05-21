//! Tauri commands exposed to the web frontend. These are the Rust ↔ JS
//! boundary; everything below them is plain Rust the daemon owns.

use std::path::PathBuf;
use std::process::Command;

use linkpilot_core::browser::{BrowserId, BrowserProfile, InstalledBrowser};
use linkpilot_core::config::{ConfigDocument, PickerStyle, Workspace, WriterId};
use linkpilot_core::history::RouteRecord;
use linkpilot_core::platform::SetDefaultOutcome;
use linkpilot_core::protocol::DoctorReport;
use linkpilot_core::routing::{
    Explained, Router, RoutingContext, RoutingDecision, Source, SourceKind,
};

use crate::dispatch::{self, LaunchOutcome};
use linkpilot_core::rules::{Rule, RuleId};
use tauri::{AppHandle, Emitter, State};
use url::Url;

use crate::state::AppState;

// ----------------------------------------------------------------------------
// config

#[tauri::command]
pub fn config_get(state: State<'_, AppState>) -> ConfigDocument {
    state.config.document()
}

#[tauri::command]
pub fn config_replace(state: State<'_, AppState>, doc: ConfigDocument) -> Result<(), String> {
    state
        .config
        .replace(doc, WriterId::Gui)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rule_upsert(state: State<'_, AppState>, rule: Rule) -> Result<(), String> {
    let mut doc = state.config.document();
    if let Some(existing) = doc.rules.iter_mut().find(|r| r.id == rule.id) {
        *existing = rule;
    } else {
        doc.rules.push(rule);
    }
    state
        .config
        .replace(doc, WriterId::Gui)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rule_delete(state: State<'_, AppState>, id: RuleId) -> Result<(), String> {
    let mut doc = state.config.document();
    doc.rules.retain(|r| r.id != id);
    state
        .config
        .replace(doc, WriterId::Gui)
        .map_err(|e| e.to_string())
}

// ----------------------------------------------------------------------------
// workspaces — named groups of rules with a batch on/off switch. The
// router (`linkpilot_core::routing`) skips rules whose workspace is
// disabled; deleting a workspace clears `workspace_id` on every
// affected rule so no rule is left dangling.

#[tauri::command]
pub fn workspace_upsert(state: State<'_, AppState>, workspace: Workspace) -> Result<(), String> {
    let mut doc = state.config.document();
    if let Some(existing) = doc.workspaces.iter_mut().find(|w| w.id == workspace.id) {
        *existing = workspace;
    } else {
        doc.workspaces.push(workspace);
    }
    state
        .config
        .replace(doc, WriterId::Gui)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn workspace_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut doc = state.config.document();
    doc.workspaces.retain(|w| w.id != id);
    // Clear `workspace_id` on every rule that pointed at the deleted
    // group, so they revert to "ungrouped" instead of becoming
    // permanently dangling refs.
    for rule in &mut doc.rules {
        if rule.workspace_id.as_deref() == Some(id.as_str()) {
            rule.workspace_id = None;
        }
    }
    state
        .config
        .replace(doc, WriterId::Gui)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn workspace_set_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let mut doc = state.config.document();
    let Some(ws) = doc.workspaces.iter_mut().find(|w| w.id == id) else {
        return Err(format!("workspace not found: {id}"));
    };
    ws.enabled = enabled;
    state
        .config
        .replace(doc, WriterId::Gui)
        .map_err(|e| e.to_string())
}

/// Master rule-evaluation kill-switch. Flips
/// `Settings.smart_routing_enabled`; the router checks this before
/// walking the rule list (see `routing::evaluate_explained`). Used by
/// the tray popover's Smart Routing toggle.
#[tauri::command]
pub fn set_smart_routing(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut doc = state.config.document();
    doc.settings.smart_routing_enabled = enabled;
    state
        .config
        .replace(doc, WriterId::Gui)
        .map_err(|e| e.to_string())
}

/// Persist the user's picker variant choice (Frosted / Bezel / Crown).
/// The picker window reads `Settings.picker_style` on open, so this
/// only affects future Ask flows — no live-reload across an open
/// picker session.
#[tauri::command]
pub fn set_picker_style(state: State<'_, AppState>, style: PickerStyle) -> Result<(), String> {
    let mut doc = state.config.document();
    doc.settings.picker_style = style;
    state
        .config
        .replace(doc, WriterId::Gui)
        .map_err(|e| e.to_string())
}

/// Persist a user-customized visible profile ordering for one browser. An
/// empty `profile_ids` list deletes the entry — the picker then falls back to
/// the default ordering (`is_default` first, then alphabetical) and shows every
/// detected profile.
#[tauri::command]
pub fn set_profile_order(
    state: State<'_, AppState>,
    browser: String,
    profile_ids: Vec<String>,
) -> Result<(), String> {
    let mut doc = state.config.document();
    if profile_ids.is_empty() {
        doc.settings.profile_orders.remove(&browser);
    } else {
        // Dedup while preserving first-seen order. A duplicate id never
        // makes semantic sense (a profile can only sit in one wheel
        // slot) and would otherwise persist verbatim to disk; the
        // picker's `apply_profile_order` silently dedups via HashMap
        // remove, but we shouldn't write garbage to the config.
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let deduped: Vec<String> = profile_ids
            .iter()
            .filter(|id| seen.insert(id.as_str()))
            .cloned()
            .collect();
        doc.settings.profile_orders.insert(browser, deduped);
    }
    state
        .config
        .replace(doc, WriterId::Gui)
        .map_err(|e| e.to_string())
}

// ----------------------------------------------------------------------------
// inventory

/// Merged "every browser this app knows about" — auto-detected from
/// the platform inventory + user-added custom entries from the config.
/// Same-id collisions: custom wins (the user explicitly edited that
/// browser so their data is more authoritative than inventory's
/// REGISTRY scan). Used by both `list_browsers` (the rendered list)
/// and `doctor` (the count surfaced on the Overview stat grid) so
/// custom browsers can't get out of sync between the two views.
fn merged_browsers(state: &AppState) -> Vec<InstalledBrowser> {
    let mut detected = state
        .platform
        .browser_inventory()
        .installed_browsers()
        .unwrap_or_default();
    let doc = state.config.document();
    for custom in doc.custom_browsers {
        if let Some(existing) = detected.iter_mut().find(|b| b.id == custom.id) {
            *existing = custom;
        } else {
            detected.push(custom);
        }
    }
    detected
}

#[tauri::command]
pub fn list_browsers(state: State<'_, AppState>) -> Result<Vec<InstalledBrowser>, String> {
    Ok(merged_browsers(&state))
}

#[tauri::command]
pub fn add_custom_browser(
    state: State<'_, AppState>,
    browser: InstalledBrowser,
) -> Result<(), String> {
    let mut doc = state.config.document();
    if let Some(existing) = doc.custom_browsers.iter_mut().find(|b| b.id == browser.id) {
        *existing = browser;
    } else {
        doc.custom_browsers.push(browser);
    }
    state
        .config
        .replace(doc, WriterId::Gui)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_custom_browser(state: State<'_, AppState>, id: BrowserId) -> Result<(), String> {
    let mut doc = state.config.document();
    doc.custom_browsers.retain(|b| b.id != id);
    state
        .config
        .replace(doc, WriterId::Gui)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_profiles(
    state: State<'_, AppState>,
    browser: BrowserId,
) -> Result<Vec<BrowserProfile>, String> {
    state
        .platform
        .browser_inventory()
        .profiles(&browser)
        .map_err(|e| e.to_string())
}

// ----------------------------------------------------------------------------
// routing

#[derive(serde::Deserialize)]
pub struct RouteRequest {
    pub url: String,
    #[serde(default)]
    pub from_app: Option<String>,
    /// Bundle id of the source app, when known (the Test URL page captures
    /// this via AppPickerButton). Lets source-app rules with `bundle_id`
    /// match correctly across localized names — "Lark" vs "飞书" both
    /// resolve to `com.electron.lark`. When absent we fall back to plain
    /// name matching in `routing::eval_matcher` for `MatcherTree::SourceApp`.
    #[serde(default)]
    pub from_app_bundle_id: Option<String>,
    /// Simulate a route arriving from a browser extension (Test-URL panel).
    /// Together with `from_profile` these populate `Source.browser/profile`
    /// so rules with source-browser / source-profile matchers can be tested
    /// without actually hooking up the extension.
    #[serde(default)]
    pub from_browser: Option<String>,
    #[serde(default)]
    pub from_profile: Option<String>,
}

#[tauri::command]
pub fn route_evaluate(state: State<'_, AppState>, request: RouteRequest) -> Explained {
    let context = build_context(&request);
    let doc = state.config.document();
    Router::new(&doc).evaluate_explained(&context)
}

#[tauri::command]
pub fn route_open(
    app: AppHandle,
    state: State<'_, AppState>,
    request: RouteRequest,
) -> Result<RoutingDecision, String> {
    let context = build_context(&request);
    let doc = state.config.document();
    let explained = Router::new(&doc).evaluate_explained(&context);
    let decision = explained.decision.clone();
    let record =
        RouteRecord::with_explanation(context.clone(), decision.clone(), explained.explanation);
    state.history.log(record.clone());
    let _ = app.emit("route-logged", &record);

    match dispatch::execute(&app, state.inner(), &decision, &request.url) {
        LaunchOutcome::Launched(_) | LaunchOutcome::Skipped | LaunchOutcome::Pending => {}
        LaunchOutcome::Failed(err) => return Err(err),
    }
    Ok(decision)
}

#[tauri::command]
pub fn route_history(state: State<'_, AppState>, limit: Option<usize>) -> Vec<RouteRecord> {
    state.history.recent(limit.unwrap_or(100))
}

fn build_context(req: &RouteRequest) -> RoutingContext {
    // If the caller specifies a source browser the simulated event is
    // browser-extension-shaped; otherwise treat it as a system URL handoff
    // (the common case for Test-URL panel and route_open from the GUI).
    let kind = if req.from_browser.is_some() {
        SourceKind::BrowserExtension
    } else {
        SourceKind::System
    };
    RoutingContext {
        url: req.url.clone(),
        source: Source {
            kind,
            app_name: req.from_app.clone(),
            bundle_id: req.from_app_bundle_id.clone(),
            browser: req.from_browser.clone(),
            profile: req.from_profile.clone(),
        },
        navigation: None,
        environment: None,
    }
}

// ----------------------------------------------------------------------------
// default browser

#[tauri::command]
pub fn is_default_browser(state: State<'_, AppState>) -> bool {
    state
        .platform
        .default_browser()
        .is_linkpilot_default()
        .unwrap_or(false)
}

#[tauri::command]
pub fn request_set_default_browser(
    state: State<'_, AppState>,
) -> Result<SetDefaultOutcome, String> {
    state
        .platform
        .default_browser()
        .request_set_default()
        .map_err(|e| e.to_string())
}

// ----------------------------------------------------------------------------
// doctor / import / export

#[tauri::command]
pub fn doctor(state: State<'_, AppState>) -> DoctorReport {
    // Count is the *merged* list — without this, custom browsers added
    // through "Add manually" wouldn't bump the Overview stat tile.
    let installed_browser_count = merged_browsers(&state).len();
    DoctorReport {
        daemon_version: env!("CARGO_PKG_VERSION").to_string(),
        is_default_browser: state
            .platform
            .default_browser()
            .is_linkpilot_default()
            .unwrap_or(false),
        config_path: Some(state.config.path().display().to_string()),
        installed_browser_count,
        ipc_socket_path: None,
    }
}

#[tauri::command]
pub fn import_config(state: State<'_, AppState>, path: PathBuf) -> Result<(), String> {
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let doc: ConfigDocument = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    state
        .config
        .replace(doc, WriterId::Gui)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_config(state: State<'_, AppState>, path: PathBuf) -> Result<(), String> {
    let doc = state.config.document();
    let json = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

// ----------------------------------------------------------------------------
// updates — discover happens in the renderer so Settings can show release
// metadata immediately; the native side owns download-to-disk because DMGs are
// too large to shuttle through the WebView IPC boundary.

#[derive(serde::Deserialize)]
pub struct UpdateDownloadRequest {
    pub url: String,
    pub version: String,
    pub asset_name: String,
    #[serde(default)]
    pub expected_bytes: Option<u64>,
    /// Lowercase hex SHA-256 of the asset, sourced from the `checksums.txt`
    /// uploaded alongside the release. Required when the release ships a
    /// `checksums.txt`; without it the daemon refuses to write the file so
    /// an unsigned DMG never ends up in the updates dir unverified.
    #[serde(default)]
    pub expected_sha256: Option<String>,
}

/// The repo whose release assets we trust. Pinned here (not just inferred
/// from the URL) so a compromised renderer can't redirect the download to
/// a fork. Keep in sync with the frontend's `LATEST_RELEASE_API` URL.
const TRUSTED_REPO_PATH_PREFIX: &str = "/jackerjay/LinkPilot/releases/download/";

#[derive(serde::Serialize)]
pub struct UpdateDownload {
    pub version: String,
    pub asset_name: String,
    pub path: String,
    pub already_downloaded: bool,
    pub bytes: u64,
}

#[tauri::command]
pub async fn update_download(
    state: State<'_, AppState>,
    request: UpdateDownloadRequest,
) -> Result<UpdateDownload, String> {
    let parsed = Url::parse(&request.url).map_err(|e| format!("invalid update URL: {e}"))?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("github.com") {
        return Err("update downloads must come from GitHub release assets".to_string());
    }
    // Path pinning. The previous host-only check let any github.com URL
    // through — including arbitrary forks. Releases live at
    // `/jackerjay/LinkPilot/releases/download/<tag>/<asset>`, so anchor
    // there. (After this initial request curl --location follows the
    // redirect to objects.githubusercontent.com; we only need to anchor
    // the *first* hop, which is the one the renderer could influence.)
    if !parsed.path().starts_with(TRUSTED_REPO_PATH_PREFIX) {
        return Err(
            "update downloads must come from the official LinkPilot release path".to_string(),
        );
    }

    let asset_name = safe_update_asset_name(&request.asset_name)?;
    let expected_sha256 = normalize_expected_sha256(request.expected_sha256.as_deref())?;
    let Some(config_dir) = state.config.path().parent().map(PathBuf::from) else {
        return Err("could not locate LinkPilot config directory".to_string());
    };
    let updates_dir = config_dir.join("updates");
    std::fs::create_dir_all(&updates_dir)
        .map_err(|e| format!("creating {}: {e}", updates_dir.display()))?;

    let destination = updates_dir.join(&asset_name);
    if let Ok(meta) = std::fs::metadata(&destination) {
        let bytes = meta.len();
        let size_match = request.expected_bytes.map(|n| n == bytes).unwrap_or(true);
        // A cached file is only reusable if its SHA-256 still matches the
        // expected one. Otherwise (corrupted, half-overwritten, or the
        // tag was force-pushed) fall through to re-download.
        let hash_match = if let Some(expected) = expected_sha256.as_deref() {
            match sha256_of_file(&destination) {
                Ok(actual) => actual == expected,
                Err(_) => false,
            }
        } else {
            true
        };
        if bytes > 0 && size_match && hash_match {
            return Ok(UpdateDownload {
                version: request.version,
                asset_name,
                path: destination.display().to_string(),
                already_downloaded: true,
                bytes,
            });
        }
    }

    let tmp = destination.with_extension("download");
    let url = request.url;
    let download_path = destination.clone();
    let expected_for_task = expected_sha256.clone();
    tauri::async_runtime::spawn_blocking(move || {
        download_update_asset(&url, &tmp, &download_path, expected_for_task.as_deref())
    })
    .await
    .map_err(|e| format!("download task failed: {e}"))??;

    let bytes = std::fs::metadata(&destination)
        .map_err(|e| format!("reading {}: {e}", destination.display()))?
        .len();
    Ok(UpdateDownload {
        version: request.version,
        asset_name,
        path: destination.display().to_string(),
        already_downloaded: false,
        bytes,
    })
}

/// Normalize a hex SHA-256 (any case, no whitespace) and reject anything
/// that isn't exactly 64 hex chars. The expected value comes from
/// `checksums.txt`, which the renderer parses — we re-validate so a
/// compromised renderer can't slip a bogus expected hash through.
fn normalize_expected_sha256(raw: Option<&str>) -> Result<Option<String>, String> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() != 64 || !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("expected_sha256 must be a 64-character hex string".to_string());
    }
    Ok(Some(trimmed.to_ascii_lowercase()))
}

/// Compute the SHA-256 of `path` via `/usr/bin/shasum`, mirroring the
/// curl shell-out path used for the download itself. Avoids pulling in a
/// fresh Rust crate for a single use site on a macOS-only feature.
fn sha256_of_file(path: &std::path::Path) -> Result<String, String> {
    let output = Command::new("/usr/bin/shasum")
        .arg("-a")
        .arg("256")
        .arg(path)
        .output()
        .map_err(|e| format!("starting shasum: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "shasum exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    // shasum prints "<hex>  <path>\n"; the hash is the leading token.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let hex = stdout
        .split_whitespace()
        .next()
        .ok_or_else(|| "shasum produced no output".to_string())?;
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("unexpected shasum output: {stdout:?}"));
    }
    Ok(hex.to_ascii_lowercase())
}

fn safe_update_asset_name(name: &str) -> Result<String, String> {
    if name.contains('/') || name.contains('\\') {
        return Err("update asset name must be a plain filename".to_string());
    }
    let file_name = name.trim();
    if file_name.is_empty() {
        return Err("update asset name is empty".to_string());
    }
    if !file_name.ends_with(".dmg") {
        return Err("LinkPilot can only auto-download macOS DMG assets".to_string());
    }
    Ok(file_name.to_string())
}

fn download_update_asset(
    url: &str,
    tmp: &PathBuf,
    destination: &PathBuf,
    expected_sha256: Option<&str>,
) -> Result<(), String> {
    if tmp.exists() {
        std::fs::remove_file(tmp).map_err(|e| format!("removing {}: {e}", tmp.display()))?;
    }
    let status = Command::new("/usr/bin/curl")
        .arg("--fail")
        .arg("--location")
        .arg("--silent")
        .arg("--show-error")
        .arg("--output")
        .arg(tmp)
        .arg(url)
        .status()
        .map_err(|e| format!("starting curl: {e}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(tmp);
        return Err(format!("curl exited with {status}"));
    }
    // Verify SHA-256 BEFORE rename. The freshly-downloaded file lives in
    // `tmp` so a mismatch is a no-op for the destination — the user's
    // previous "good" download (if any) stays untouched.
    if let Some(expected) = expected_sha256 {
        match sha256_of_file(tmp) {
            Ok(actual) if actual == expected => {}
            Ok(actual) => {
                let _ = std::fs::remove_file(tmp);
                return Err(format!(
                    "checksum mismatch: expected {expected}, got {actual}"
                ));
            }
            Err(err) => {
                let _ = std::fs::remove_file(tmp);
                return Err(format!("hashing downloaded file failed: {err}"));
            }
        }
    }
    // POSIX rename(2) is an atomic replace on the same filesystem — no
    // need to remove `destination` first. Doing so would open a window
    // where the destination is gone but the rename hasn't completed.
    std::fs::rename(tmp, destination)
        .map_err(|e| format!("moving update to {}: {e}", destination.display()))
}

// ----------------------------------------------------------------------------
// app icons — extracted from the system's .icns files, cached to disk,
// returned to the renderer as base64-encoded PNGs.

#[derive(serde::Deserialize)]
pub struct AppIconRequest {
    #[serde(default)]
    pub bundle_id: Option<String>,
    #[serde(default)]
    pub app_path: Option<String>,
    /// Display name fallback — resolved through Spotlight when neither
    /// bundle_id nor app_path is available (e.g. a source-app matcher
    /// only stores "Slack").
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_icon_size")]
    pub size: u32,
}

fn default_icon_size() -> u32 {
    64
}

#[derive(serde::Serialize)]
pub struct AppIcon {
    /// base64-encoded PNG bytes for a `<img src="data:image/png;base64,…">`.
    pub data_url: String,
}

// ----------------------------------------------------------------------------
// app picker — open the native macOS "Choose Application" dialog. Used by
// the RuleEditor's source-app matcher and the Test-URL panel's "From app".

#[derive(serde::Serialize)]
pub struct PickedApp {
    pub name: String,
    pub bundle_id: String,
}

#[tauri::command]
pub fn pick_app() -> Result<Option<PickedApp>, String> {
    #[cfg(target_os = "macos")]
    {
        return linkpilot_platform_mac::app_picker::choose_app().map(|opt| {
            opt.map(|a| PickedApp {
                name: a.name,
                bundle_id: a.bundle_id,
            })
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(None)
    }
}

#[tauri::command]
pub fn app_icon(request: AppIconRequest) -> Option<AppIcon> {
    #[cfg(target_os = "macos")]
    {
        use base64::Engine;
        let bundle = request.bundle_id.as_deref().filter(|s| !s.is_empty());
        let path = request
            .app_path
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(std::path::Path::new);
        let name = request.name.as_deref().filter(|s| !s.is_empty());
        let png_path =
            match linkpilot_platform_mac::app_icon::ensure_png(bundle, path, name, request.size) {
                Ok(p) => p,
                Err(err) => {
                    tracing::debug!(?err, ?bundle, ?path, ?name, "app_icon: extraction failed");
                    return None;
                }
            };
        let bytes = std::fs::read(&png_path).ok()?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        return Some(AppIcon {
            data_url: format!("data:image/png;base64,{b64}"),
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = request;
        None
    }
}

// ----------------------------------------------------------------------------
// command-line tool — the `lpt` binary is bundled inside the .app under
// `Contents/MacOS/lp` (see release.yml). These commands let the Settings page
// surface its location and create a user-PATH symlink so users get a
// "GUI + CLI" install with one click.

#[derive(serde::Serialize)]
pub struct CliInstallStatus {
    /// Absolute path of `lpt` inside the running .app bundle, or `null`
    /// when running from a dev build (where the embed step hasn't run).
    pub bundled_path: Option<String>,
    /// Where `cli_install_to_path` would symlink by default: `~/.local/bin/lpt`.
    pub default_target: String,
    /// True iff `default_target` already symlinks to `bundled_path`.
    pub already_installed: bool,
}

#[tauri::command]
pub fn cli_install_status() -> CliInstallStatus {
    let bundled = locate_bundled_lp();
    let default_target = default_install_target();
    let already_installed = match (bundled.as_ref(), &default_target) {
        (Some(b), t) => std::fs::read_link(t)
            .map(|link| link == PathBuf::from(b))
            .unwrap_or(false),
        _ => false,
    };
    CliInstallStatus {
        bundled_path: bundled,
        default_target: default_target.display().to_string(),
        already_installed,
    }
}

#[tauri::command]
pub fn cli_install_to_path(target: Option<String>) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let bundled = locate_bundled_lp().ok_or_else(|| {
            "bundled `lpt` not found — this only works on the packaged .app, \
             not dev builds (`npx tauri dev`)"
                .to_string()
        })?;
        let target_path = target
            .map(PathBuf::from)
            .unwrap_or_else(default_install_target);

        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creating {}: {e}", parent.display()))?;
        }
        // Idempotency: if the existing entry already points at the bundled
        // `lpt`, treat the call as a no-op success. Otherwise replace it.
        if target_path.symlink_metadata().is_ok() {
            if let Ok(existing) = std::fs::read_link(&target_path) {
                if existing == PathBuf::from(&bundled) {
                    return Ok(target_path.display().to_string());
                }
            }
            std::fs::remove_file(&target_path)
                .map_err(|e| format!("removing existing {}: {e}", target_path.display()))?;
        }
        std::os::unix::fs::symlink(&bundled, &target_path)
            .map_err(|e| format!("symlinking to {}: {e}", target_path.display()))?;
        Ok(target_path.display().to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = target;
        Err("CLI install is only supported on macOS in v0.1".to_string())
    }
}

#[cfg(target_os = "macos")]
fn locate_bundled_lp() -> Option<String> {
    // `current_exe()` inside a .app bundle returns
    // `…/LinkPilot.app/Contents/MacOS/linkpilot-desktop`, so the sibling
    // `lpt` is what release.yml embeds. In `tauri dev` the exe lives in
    // `target/debug/` with no `lpt` next to it — we return None there.
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join("lpt");
    candidate.is_file().then(|| candidate.display().to_string())
}

#[cfg(not(target_os = "macos"))]
fn locate_bundled_lp() -> Option<String> {
    None
}

fn default_install_target() -> PathBuf {
    // ~/.local/bin/lpt — user-writable, no admin auth needed. The XDG
    // user-binary convention; users add it to PATH in their shell rc.
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    home.join(".local").join("bin").join("lpt")
}

// ----------------------------------------------------------------------------
// background daemon service — the LaunchAgent that runs `linkpilot-daemon`
// even when the GUI is closed. v0.2 ships the daemon as a sibling binary
// inside the .app (Contents/MacOS/linkpilot-daemon); these commands let the
// Settings page show its state and (un)install the LaunchAgent plist.

#[derive(serde::Serialize)]
pub struct DaemonServiceStatus {
    /// Path of the bundled daemon binary inside the running .app, or
    /// None for dev builds where the embed step hasn't run.
    pub bundled_path: Option<String>,
    /// True if `~/Library/LaunchAgents/app.linkpilot.daemon.plist` exists.
    pub plist_exists: bool,
    /// True if `launchctl list app.linkpilot.daemon` finds the agent.
    pub loaded: bool,
    /// PID of the running daemon, if launchd has it active.
    pub pid: Option<i32>,
    /// Which daemon path the GUI is using right now — "in-process" means
    /// the GUI hosts the daemon itself, "external" means it's talking to
    /// a separately-running `linkpilot-daemon`.
    pub gui_mode: &'static str,
}

#[tauri::command]
pub fn daemon_service_status(state: tauri::State<'_, AppState>) -> DaemonServiceStatus {
    #[cfg(target_os = "macos")]
    {
        let bundled = locate_bundled_daemon();
        let agent_status = linkpilot_platform_mac::launch_agent::daemon_status().ok();
        let gui_mode = match state.daemon_mode() {
            crate::state::DaemonMode::InProcess => "in-process",
            crate::state::DaemonMode::External => "external",
        };
        return DaemonServiceStatus {
            bundled_path: bundled,
            plist_exists: agent_status
                .as_ref()
                .map(|s| s.plist_exists)
                .unwrap_or(false),
            loaded: agent_status.as_ref().map(|s| s.loaded).unwrap_or(false),
            pid: agent_status.and_then(|s| s.pid),
            gui_mode,
        };
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = state;
        DaemonServiceStatus {
            bundled_path: None,
            plist_exists: false,
            loaded: false,
            pid: None,
            gui_mode: "in-process",
        }
    }
}

#[tauri::command]
pub fn daemon_service_install() -> Result<DaemonServiceStatus, String> {
    #[cfg(target_os = "macos")]
    {
        let bundled = locate_bundled_daemon().ok_or_else(|| {
            "bundled `linkpilot-daemon` not found — only the packaged .app installs the background service"
                .to_string()
        })?;
        linkpilot_platform_mac::launch_agent::install_daemon(std::path::Path::new(&bundled))
            .map_err(|e| e.to_string())?;
        Ok(refresh_daemon_status(bundled))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("background service is macOS-only in v0.2".into())
    }
}

#[tauri::command]
pub fn daemon_service_uninstall() -> Result<DaemonServiceStatus, String> {
    #[cfg(target_os = "macos")]
    {
        linkpilot_platform_mac::launch_agent::uninstall_daemon().map_err(|e| e.to_string())?;
        Ok(refresh_daemon_status(
            locate_bundled_daemon().unwrap_or_default(),
        ))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("background service is macOS-only in v0.2".into())
    }
}

#[cfg(target_os = "macos")]
fn refresh_daemon_status(bundled: String) -> DaemonServiceStatus {
    let status = linkpilot_platform_mac::launch_agent::daemon_status().ok();
    DaemonServiceStatus {
        bundled_path: if bundled.is_empty() {
            None
        } else {
            Some(bundled)
        },
        plist_exists: status.as_ref().map(|s| s.plist_exists).unwrap_or(false),
        loaded: status.as_ref().map(|s| s.loaded).unwrap_or(false),
        pid: status.and_then(|s| s.pid),
        gui_mode: "in-process",
    }
}

#[cfg(target_os = "macos")]
fn locate_bundled_daemon() -> Option<String> {
    // Sibling of the running .app's main binary, mirroring the lpt embed
    // pattern. In dev (`npx tauri dev`) the daemon isn't sitting in
    // target/debug next to the desktop binary unless the user ran
    // `cargo build -p linkpilot-headless-daemon` first.
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join("linkpilot-daemon");
    candidate.is_file().then(|| candidate.display().to_string())
}
