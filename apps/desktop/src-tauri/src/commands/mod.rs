//! Tauri commands exposed to the web frontend. These are the Rust ↔ JS
//! boundary; everything below them is plain Rust the daemon owns.

use std::path::PathBuf;

use linkpilot_core::browser::{BrowserId, BrowserProfile, InstalledBrowser};
use linkpilot_core::config::{ConfigDocument, WriterId};
use linkpilot_core::history::RouteRecord;
use linkpilot_core::platform::SetDefaultOutcome;
use linkpilot_core::protocol::DoctorReport;
use linkpilot_core::routing::{Router, RoutingContext, RoutingDecision, Source, SourceKind};
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
}

#[tauri::command]
pub fn route_evaluate(
    state: State<'_, AppState>,
    request: RouteRequest,
) -> RoutingDecision {
    let context = build_context(&request);
    let doc = state.config.document();
    Router::new(&doc).evaluate(&context)
}

#[tauri::command]
pub fn route_open(
    app: AppHandle,
    state: State<'_, AppState>,
    request: RouteRequest,
) -> Result<RoutingDecision, String> {
    let context = build_context(&request);
    let doc = state.config.document();
    let decision = Router::new(&doc).evaluate(&context);
    let record = RouteRecord::new(context.clone(), decision.clone());
    state.history.log(record.clone());
    let _ = app.emit("route-logged", &record);

    if let RoutingDecision::Open { target, .. } = &decision {
        let parsed = url::Url::parse(&request.url).map_err(|e| e.to_string())?;
        state
            .platform
            .url_launcher()
            .open(target, &parsed)
            .map_err(|e| e.to_string())?;
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
    RoutingContext {
        url: req.url.clone(),
        source: Source {
            kind: SourceKind::BrowserExtension, // GUI-initiated; refined later
            app_name: req.from_app.clone(),
            bundle_id: None,
            browser: None,
            profile: None,
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
