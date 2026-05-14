//! Decision → action helper shared by every URL-launch entry point
//! (system URL handler, `route_open` Tauri command, IPC RouteOpen).

use std::collections::HashMap;

use linkpilot_core::browser::BrowserTarget;
use linkpilot_core::routing::RoutingDecision;
use tauri::AppHandle;
use url::Url;

use crate::picker::{self, PickerChoice};
use crate::state::AppState;

#[derive(Debug)]
pub enum LaunchOutcome {
    /// We did launch a browser (either the rule's target, or the user's
    /// pick from an Ask prompt). The final target is included so the
    /// caller can attribute history to it — currently unused but kept
    /// in the signature for a follow-up that logs Ask resolutions.
    Launched(#[allow(dead_code)] BrowserTarget),
    /// Decision was Allow / Block — intentionally no launch.
    Skipped,
    /// Ask flow kicked off on a worker thread; launch (or cancel)
    /// happens after the user picks. Caller returns immediately so the
    /// main thread isn't tied up while the picker is on screen.
    Pending,
    /// A real failure (bad URL, no installed browsers, launcher error).
    Failed(String),
}

/// Carry out the launch side of a routing decision. Pure plumbing — the
/// decision is assumed already logged to history by the caller.
pub fn execute(
    app: &AppHandle,
    state: &AppState,
    decision: &RoutingDecision,
    raw_url: &str,
) -> LaunchOutcome {
    tracing::debug!(?decision, %raw_url, "dispatch::execute");
    let parsed = match Url::parse(raw_url) {
        Ok(u) => u,
        Err(err) => return LaunchOutcome::Failed(format!("bad URL {raw_url}: {err}")),
    };

    match decision {
        RoutingDecision::Open { target, .. } => match state
            .platform
            .url_launcher()
            .open(target, &parsed)
        {
            Ok(()) => LaunchOutcome::Launched(target.clone()),
            Err(err) => LaunchOutcome::Failed(err.to_string()),
        },

        RoutingDecision::Ask { candidates, .. } => {
            tracing::info!(
                candidate_count = candidates.len(),
                %raw_url,
                "dispatch: ask — spawning picker thread"
            );
            // Detach to a worker thread: opening the picker window +
            // blocking-recv on the user's pick would otherwise tie up
            // the caller (main thread for deep-link callback / sync
            // Tauri command), and then `picker_resolve` (also a sync
            // command, also on main thread) could never run — classic
            // deadlock that froze the UI for 60s on the first attempt.
            let app = app.clone();
            let state = state.clone();
            let candidates = candidates.clone();
            let url = raw_url.to_string();
            std::thread::spawn(move || {
                let target = match resolve_ask(&app, &state, &candidates, &url) {
                    Some(t) => t,
                    None => {
                        tracing::info!("dispatch: ask cancelled");
                        return;
                    }
                };
                let parsed = match Url::parse(&url) {
                    Ok(u) => u,
                    Err(err) => {
                        tracing::error!(?err, %url, "ask: bad URL after pick");
                        return;
                    }
                };
                tracing::info!(?target, "dispatch: ask resolved — launching");
                if let Err(err) = state.platform.url_launcher().open(&target, &parsed) {
                    tracing::error!(?err, "ask: launcher failed");
                }
            });
            LaunchOutcome::Pending
        }

        RoutingDecision::Allow { .. } | RoutingDecision::Block { .. } => {
            LaunchOutcome::Skipped
        }
    }
}

/// Build the picker choices, show the Tauri picker window, map the
/// returned id back to a BrowserTarget. When the rule's candidates list
/// is empty we fall back to every installed browser.
fn resolve_ask(
    app: &AppHandle,
    state: &AppState,
    candidates: &[BrowserTarget],
    url: &str,
) -> Option<BrowserTarget> {
    let installed = state
        .platform
        .browser_inventory()
        .installed_browsers()
        .ok()?;

    let source: Vec<BrowserTarget> = if candidates.is_empty() {
        installed
            .iter()
            .map(|b| BrowserTarget::new(b.id.clone()))
            .collect()
    } else {
        candidates.to_vec()
    };

    if source.is_empty() {
        return None;
    }

    // Parallel: choices for the picker UI + lookup back to the full
    // BrowserTarget (so we preserve any profile / new-window / incognito
    // hints the original rule specified).
    let mut target_by_id: HashMap<String, BrowserTarget> = HashMap::new();
    let mut choices: Vec<PickerChoice> = Vec::with_capacity(source.len());
    for target in source {
        let info = installed.iter().find(|b| b.id == target.browser);
        let name = info
            .map(|b| b.display_name.clone())
            .unwrap_or_else(|| target.browser.0.clone());
        let bundle_id = info.and_then(|b| b.platform_app_id.clone());
        let app_path = info.map(|b| app_path_from_executable(&b.executable));
        let id = target.browser.0.clone();
        target_by_id.insert(id.clone(), target);
        choices.push(PickerChoice {
            id,
            name,
            bundle_id,
            app_path,
        });
    }

    let picked_id = picker::show_picker(app, url, choices)?;
    target_by_id.remove(&picked_id)
}

/// `/Applications/Foo.app/Contents/MacOS/Foo` → `/Applications/Foo.app`.
fn app_path_from_executable(exe: &std::path::Path) -> String {
    let s = exe.to_string_lossy();
    match s.rfind(".app/") {
        Some(idx) => s[..idx + 4].to_string(),
        None => s.into_owned(),
    }
}
