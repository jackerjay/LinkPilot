//! Receive `open URL` events (macOS Apple Events surfaced via the deep-link
//! plugin) and dispatch them through the LinkPilot router.

use linkpilot_core::history::RouteRecord;
use linkpilot_core::platform::OpenEventHint;
use linkpilot_core::routing::{Router, RoutingContext, Source, SourceKind};
use tauri::{AppHandle, Emitter};

use crate::dispatch::{self, LaunchOutcome};
use crate::state::AppState;

/// Build a [`RoutingContext`] from an incoming system URL event and execute
/// the resulting decision. Any errors are logged and a `route-failed` event
/// is emitted so the GUI can surface them.
pub fn dispatch_system_url(state: &AppState, app: &AppHandle, url: String) {
    // The Apple Event delivered by tauri-plugin-deep-link doesn't carry the
    // sender, so we ask the platform crate's opener detector — on macOS that
    // returns the most-recently-active app other than LinkPilot itself.
    let opener = state.platform.opener_detector().detect(&OpenEventHint::default());
    if let Some(o) = &opener {
        tracing::debug!(name = %o.name, bundle = ?o.bundle_id, "detected opener");
    }

    let context = RoutingContext {
        url: url.clone(),
        source: Source {
            kind: SourceKind::System,
            app_name: opener.as_ref().map(|o| o.name.clone()),
            bundle_id: opener.as_ref().and_then(|o| o.bundle_id.clone()),
            browser: None,
            profile: None,
        },
        navigation: None,
        environment: None,
    };

    let doc = state.config.document();
    let explained = Router::new(&doc).evaluate_explained(&context);
    let decision = explained.decision.clone();
    let record =
        RouteRecord::with_explanation(context.clone(), decision.clone(), explained.explanation);
    state.history.log(record.clone());
    let _ = app.emit("route-logged", &record);

    match dispatch::execute(state, &decision, &url) {
        LaunchOutcome::Launched(_) | LaunchOutcome::Skipped | LaunchOutcome::Cancelled => {}
        LaunchOutcome::Failed(err) => {
            tracing::error!(%err, %url, "url_handler: launch failed");
            let _ = app.emit("route-failed", format!("launch failed for {url}: {err}"));
        }
    }
}
