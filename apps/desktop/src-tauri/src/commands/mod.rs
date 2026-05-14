//! Tauri commands exposed to the web frontend. These are the Rust ↔ JS
//! boundary; everything below them is plain Rust the daemon owns.

use std::path::PathBuf;

use linkpilot_core::browser::{BrowserId, BrowserProfile, InstalledBrowser};
use linkpilot_core::config::{ConfigDocument, WriterId};
use linkpilot_core::history::RouteRecord;
use linkpilot_core::platform::SetDefaultOutcome;
use linkpilot_core::protocol::DoctorReport;
use linkpilot_core::routing::{Explained, Router, RoutingContext, RoutingDecision, Source, SourceKind};

use crate::dispatch::{self, LaunchOutcome};
use linkpilot_core::rules::{Rule, RuleId};
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

// ----------------------------------------------------------------------------
// config

#[tauri::command]
pub fn config_get(state: State<'_, AppState>) -> ConfigDocument {
    state.config.document()
}

#[tauri::command]
pub fn config_replace(
    state: State<'_, AppState>,
    doc: ConfigDocument,
) -> Result<(), String> {
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
// inventory

#[tauri::command]
pub fn list_browsers(state: State<'_, AppState>) -> Result<Vec<InstalledBrowser>, String> {
    state
        .platform
        .browser_inventory()
        .installed_browsers()
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
pub fn route_evaluate(
    state: State<'_, AppState>,
    request: RouteRequest,
) -> Explained {
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
        LaunchOutcome::Launched(_)
        | LaunchOutcome::Skipped
        | LaunchOutcome::Pending => {}
        LaunchOutcome::Failed(err) => return Err(err),
    }
    Ok(decision)
}

#[tauri::command]
pub fn route_history(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Vec<RouteRecord> {
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
            bundle_id: None,
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
    let installed = state
        .platform
        .browser_inventory()
        .installed_browsers()
        .map(|v| v.len())
        .unwrap_or(0);
    DoctorReport {
        daemon_version: env!("CARGO_PKG_VERSION").to_string(),
        is_default_browser: state
            .platform
            .default_browser()
            .is_linkpilot_default()
            .unwrap_or(false),
        config_path: Some(state.config.path().display().to_string()),
        installed_browser_count: installed,
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
        let png_path = match linkpilot_platform_mac::app_icon::ensure_png(
            bundle,
            path,
            name,
            request.size,
        ) {
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
