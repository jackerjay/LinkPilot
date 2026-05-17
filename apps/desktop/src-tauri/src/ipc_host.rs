//! Daemon-side glue: routes incoming IPC [`Request`] frames.
//!
//! Most verbs go straight to [`linkpilot_core::daemon::DaemonRuntime`]'s
//! default `RequestHandler` impl. We only intercept `RouteOpen` so the
//! Tauri shell can:
//!   - emit a `route-logged` event for the Inspector page
//!   - run the launch through `dispatch::execute`, which knows how to
//!     spawn the picker window for `Ask` decisions
//!
//! All other verbs (RouteEvaluate, ConfigGet, ConfigReplace, Doctor,
//! StatePing) are pure functions of daemon state — sharing them with the
//! headless daemon avoids GUI-only drift.

use std::sync::Arc;

use linkpilot_core::daemon::{DaemonRuntime, RequestHandler};
use linkpilot_core::protocol::{Request, Response};
use tauri::{AppHandle, Emitter};

use crate::dispatch::{self, LaunchOutcome};
use crate::state::AppState;

pub struct DaemonHandler {
    runtime: Arc<DaemonRuntime>,
    state: AppState,
    app: AppHandle,
}

impl DaemonHandler {
    pub fn new(runtime: Arc<DaemonRuntime>, state: AppState, app: AppHandle) -> Self {
        Self {
            runtime,
            state,
            app,
        }
    }
}

impl RequestHandler for DaemonHandler {
    fn handle(&self, request: Request) -> Response {
        match request {
            Request::RouteOpen {
                request_id,
                context,
            } => {
                let raw_url = context.url.clone();
                let (explained, record) = self.runtime.evaluate_and_log(context);
                let _ = self.app.emit("route-logged", &record);
                let decision = explained.decision;

                match dispatch::execute(&self.app, &self.state, &decision, &raw_url) {
                    LaunchOutcome::Launched(_)
                    | LaunchOutcome::Skipped
                    | LaunchOutcome::Pending => {}
                    LaunchOutcome::Failed(err) => {
                        return Response::Error {
                            request_id,
                            code: "launch-failed".into(),
                            message: err,
                        };
                    }
                }
                Response::RouteDecision {
                    request_id,
                    decision,
                }
            }

            other => self.runtime.handle(other),
        }
    }
}
