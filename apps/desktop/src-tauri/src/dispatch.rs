//! Decision → action helper shared by every URL-launch entry point
//! (system URL handler, `route_open` Tauri command, IPC RouteOpen).
//!
//! Previously each callsite had its own `if let Open { target } = decision`
//! branch and silently ignored Ask / Allow / Block. With the Ask branch
//! now needing real UI (native chooser), the logic belongs in one place.

use linkpilot_core::browser::BrowserTarget;
use linkpilot_core::routing::RoutingDecision;
use url::Url;

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
    /// User dismissed the Ask prompt.
    Cancelled,
    /// A real failure (bad URL, no installed browsers, launcher error).
    Failed(String),
}

/// Carry out the launch side of a routing decision. Pure plumbing — the
/// decision is assumed already logged to history by the caller.
pub fn execute(state: &AppState, decision: &RoutingDecision, raw_url: &str) -> LaunchOutcome {
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
                "dispatch: ask — showing chooser"
            );
            let target = match resolve_ask(state, candidates, raw_url) {
                Some(t) => t,
                None => {
                    tracing::info!("dispatch: ask cancelled or no candidates");
                    return LaunchOutcome::Cancelled;
                }
            };
            tracing::info!(?target, "dispatch: ask resolved");
            match state.platform.url_launcher().open(&target, &parsed) {
                Ok(()) => LaunchOutcome::Launched(target),
                Err(err) => LaunchOutcome::Failed(err.to_string()),
            }
        }

        RoutingDecision::Allow { .. } | RoutingDecision::Block { .. } => {
            LaunchOutcome::Skipped
        }
    }
}

/// Show the macOS chooser (osascript on macOS, stub elsewhere). When
/// the rule's candidates list is empty we fall back to every installed
/// browser the inventory knows about.
fn resolve_ask(
    state: &AppState,
    candidates: &[BrowserTarget],
    _url: &str,
) -> Option<BrowserTarget> {
    let installed = state
        .platform
        .browser_inventory()
        .installed_browsers()
        .ok()?;

    // Build (label → target) pairs in display order. Labels are the
    // installed browser's display_name when known; the rule may have
    // supplied targets that point at uninstalled browsers, in which
    // case we surface the id verbatim.
    let mut choices: Vec<(String, BrowserTarget)> = if candidates.is_empty() {
        installed
            .iter()
            .map(|b| {
                (
                    b.display_name.clone(),
                    BrowserTarget::new(b.id.clone()),
                )
            })
            .collect()
    } else {
        candidates
            .iter()
            .map(|t| {
                let label = installed
                    .iter()
                    .find(|b| b.id == t.browser)
                    .map(|b| b.display_name.clone())
                    .unwrap_or_else(|| t.browser.0.clone());
                (label, t.clone())
            })
            .collect()
    };

    if choices.is_empty() {
        return None;
    }

    #[cfg(target_os = "macos")]
    {
        let labels: Vec<String> = choices.iter().map(|(n, _)| n.clone()).collect();
        let picked = linkpilot_platform_mac::prompt::pick_browser(_url, &labels)?;
        // Linear search is fine for ≤ 10 entries.
        let idx = choices.iter().position(|(n, _)| n == &picked)?;
        return Some(choices.swap_remove(idx).1);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = &mut choices;
        None
    }
}

