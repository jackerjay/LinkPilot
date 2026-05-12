//! Receive `open URL` events (macOS Apple Events surfaced via the deep-link
//! plugin) and dispatch them through the LinkPilot router.

use linkpilot_core::history::RouteRecord;
use linkpilot_core::routing::{Router, RoutingContext, RoutingDecision, Source, SourceKind};
use tauri::{AppHandle, Emitter};

use crate::state::AppState;

/// Build a [`RoutingContext`] from an incoming system URL event and execute
/// the resulting decision. Any errors are logged and a `route-failed` event
/// is emitted so the GUI can surface them.
pub fn dispatch_system_url(state: &AppState, app: &AppHandle, url: String) {
    let context = RoutingContext {
        url: url.clone(),
        source: Source {
            kind: SourceKind::System,
            app_name: None,
            bundle_id: None,
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

    if let RoutingDecision::Open { target, .. } = &decision {
        match url::Url::parse(&url) {
            Ok(parsed) => {
                if let Err(err) = state.platform.url_launcher().open(target, &parsed) {
                    tracing::error!(?err, %url, "url_handler: launch failed");
                    let _ = app.emit(
                        "route-failed",
                        format!("launch failed for {url}: {err}"),
                    );
                }
            }
            Err(err) => {
                tracing::warn!(?err, %url, "url_handler: malformed URL");
            }
        }
    }
}
