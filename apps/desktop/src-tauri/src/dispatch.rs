//! Decision → action helper shared by every URL-launch entry point
//! (system URL handler, `route_open` Tauri command, IPC RouteOpen).

use std::collections::HashMap;

use linkpilot_core::browser::BrowserTarget;
use linkpilot_core::platform::UrlLauncher;
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

    let default_target = state.config.document().default_target;

    match decision {
        RoutingDecision::Open { target, .. } => {
            match open_with_default_fallback(
                state.platform.url_launcher(),
                target,
                &default_target,
                &parsed,
            ) {
                Ok(launched) => LaunchOutcome::Launched(launched),
                Err(err) => LaunchOutcome::Failed(err),
            }
        }

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
                if let Err(err) = open_with_default_fallback(
                    state.platform.url_launcher(),
                    &target,
                    &default_target,
                    &parsed,
                ) {
                    tracing::error!(err, "ask: launcher failed (default fallback exhausted)");
                }
            });
            LaunchOutcome::Pending
        }

        RoutingDecision::Allow { .. } | RoutingDecision::Block { .. } => LaunchOutcome::Skipped,
    }
}

/// Try `primary`; on failure (e.g. the rule references a browser the
/// user uninstalled or mistyped) retry with the config's
/// `default_target`. Returns the [`BrowserTarget`] that actually
/// opened, or a combined error string when both attempts fail.
///
/// The retry is skipped when the primary and the default share a
/// browser id — there's nothing to gain from launching the same
/// missing binary twice, and skipping avoids an infinite loop if the
/// user's default is itself broken.
fn open_with_default_fallback(
    launcher: &dyn UrlLauncher,
    primary: &BrowserTarget,
    default_target: &BrowserTarget,
    url: &Url,
) -> std::result::Result<BrowserTarget, String> {
    match launcher.open(primary, url) {
        Ok(()) => Ok(primary.clone()),
        Err(primary_err) => {
            if primary.browser == default_target.browser {
                return Err(format!(
                    "launch failed: {primary_err} (no fallback — primary already matches default browser '{}')",
                    primary.browser
                ));
            }
            tracing::warn!(
                primary = ?primary,
                default = ?default_target,
                error = %primary_err,
                "dispatch: primary launch failed — falling back to default browser"
            );
            match launcher.open(default_target, url) {
                Ok(()) => Ok(default_target.clone()),
                Err(default_err) => Err(format!(
                    "primary launch failed ({primary_err}); default fallback also failed ({default_err})"
                )),
            }
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
            // Filled in by `show_picker` itself (pre-rendered base64).
            // Dispatch doesn't have to know about pixel data.
            icon_data_url: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use linkpilot_core::browser::BrowserId;
    use linkpilot_core::platform::{PlatformError, Result as PlatformResult};
    use std::sync::Mutex;

    /// Test double: each `open` call pops the next scripted result off
    /// `script` and records the target it was called with.
    struct ScriptedLauncher {
        script: Mutex<Vec<PlatformResult<()>>>,
        calls: Mutex<Vec<BrowserTarget>>,
    }

    impl ScriptedLauncher {
        fn new(script: Vec<PlatformResult<()>>) -> Self {
            Self {
                script: Mutex::new(script),
                calls: Mutex::new(Vec::new()),
            }
        }
        fn calls(&self) -> Vec<BrowserTarget> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl UrlLauncher for ScriptedLauncher {
        fn open(&self, target: &BrowserTarget, _url: &Url) -> PlatformResult<()> {
            self.calls.lock().unwrap().push(target.clone());
            self.script
                .lock()
                .unwrap()
                .pop()
                .unwrap_or(Err(PlatformError::Other("script exhausted".into())))
        }
    }

    fn url() -> Url {
        Url::parse("https://example.com/").unwrap()
    }

    #[test]
    fn falls_back_to_default_when_primary_launcher_errors() {
        // User configured a rule that targets a browser they later
        // uninstalled / mistyped. Primary fails → default catches it.
        let launcher = ScriptedLauncher::new(vec![
            Ok(()),                                            // 2nd call (default)
            Err(PlatformError::Other("not installed".into())), // 1st call (primary)
        ]);
        let primary = BrowserTarget::new(BrowserId::new("ghost-browser"));
        let default_target = BrowserTarget::new(BrowserId::new("safari"));

        let launched = open_with_default_fallback(&launcher, &primary, &default_target, &url())
            .expect("default fallback should succeed");

        assert_eq!(launched.browser.0, "safari");
        let calls = launcher.calls();
        assert_eq!(calls.len(), 2, "should have tried primary then default");
        assert_eq!(calls[0].browser.0, "ghost-browser");
        assert_eq!(calls[1].browser.0, "safari");
    }

    #[test]
    fn no_fallback_when_primary_already_is_default() {
        // Default itself is broken — no point re-trying the same thing.
        let launcher =
            ScriptedLauncher::new(vec![Err(PlatformError::Other("not installed".into()))]);
        let same = BrowserTarget::new(BrowserId::new("ghost-browser"));

        let err = open_with_default_fallback(&launcher, &same, &same, &url())
            .expect_err("should not retry when primary == default");
        assert!(
            err.contains("no fallback"),
            "msg should explain skip: {err}"
        );
        assert_eq!(launcher.calls().len(), 1, "exactly one launch attempt");
    }

    #[test]
    fn both_failures_reported_when_default_also_breaks() {
        let launcher = ScriptedLauncher::new(vec![
            Err(PlatformError::Other("default broken too".into())),
            Err(PlatformError::Other("primary broken".into())),
        ]);
        let primary = BrowserTarget::new(BrowserId::new("ghost-browser"));
        let default_target = BrowserTarget::new(BrowserId::new("safari"));

        let err = open_with_default_fallback(&launcher, &primary, &default_target, &url())
            .expect_err("both failures should bubble up");
        assert!(
            err.contains("primary broken"),
            "msg missing primary err: {err}"
        );
        assert!(
            err.contains("default broken too"),
            "msg missing default err: {err}"
        );
        assert_eq!(launcher.calls().len(), 2);
    }

    #[test]
    fn primary_success_does_not_trigger_fallback() {
        let launcher = ScriptedLauncher::new(vec![Ok(())]);
        let primary = BrowserTarget::new(BrowserId::new("chrome"));
        let default_target = BrowserTarget::new(BrowserId::new("safari"));

        let launched = open_with_default_fallback(&launcher, &primary, &default_target, &url())
            .expect("primary success should pass through");
        assert_eq!(launched.browser.0, "chrome");
        assert_eq!(launcher.calls().len(), 1, "default should not be tried");
    }
}
