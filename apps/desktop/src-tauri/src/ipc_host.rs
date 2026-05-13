//! Daemon-side glue: routes incoming IPC [`Request`] frames to the same code
//! the Tauri commands call. The actual transport is owned by
//! [`linkpilot_ipc::server`]; this module only provides the [`RequestHandler`]
//! impl that the server invokes for every frame.

use linkpilot_core::history::RouteRecord;
use linkpilot_core::protocol::{DoctorReport, Request, Response};
use linkpilot_core::routing::Router;
use linkpilot_ipc::server::RequestHandler;
use tauri::{AppHandle, Emitter};

use crate::dispatch::{self, LaunchOutcome};
use crate::state::AppState;

pub struct DaemonHandler {
    pub state: AppState,
    pub app: AppHandle,
    pub version: String,
}

impl DaemonHandler {
    pub fn new(state: AppState, app: AppHandle) -> Self {
        Self {
            state,
            app,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl RequestHandler for DaemonHandler {
    fn handle(&self, request: Request) -> Response {
        match request {
            Request::RouteEvaluate { request_id, context } => {
                let doc = self.state.config.document();
                let decision = Router::new(&doc).evaluate(&context);
                Response::RouteDecision {
                    request_id,
                    decision,
                }
            }

            Request::RouteOpen { request_id, context } => {
                let doc = self.state.config.document();
                let explained = Router::new(&doc).evaluate_explained(&context);
                let decision = explained.decision.clone();
                let record = RouteRecord::with_explanation(
                    context.clone(),
                    decision.clone(),
                    explained.explanation,
                );
                self.state.history.log(record.clone());
                let _ = self.app.emit("route-logged", &record);

                match dispatch::execute(&self.state, &decision, &context.url) {
                    LaunchOutcome::Launched(_)
                    | LaunchOutcome::Skipped
                    | LaunchOutcome::Cancelled => {}
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

            Request::ConfigGet { request_id } => Response::ConfigSnapshot {
                request_id,
                doc: self.state.config.document(),
            },

            Request::ConfigReplace {
                request_id,
                doc,
                writer,
            } => match self.state.config.replace(doc, writer) {
                Ok(()) => Response::Ack { request_id },
                Err(err) => Response::Error {
                    request_id,
                    code: "config-replace".into(),
                    message: err.to_string(),
                },
            },

            Request::Doctor { request_id } => Response::DoctorReport {
                request_id,
                report: DoctorReport {
                    daemon_version: self.version.clone(),
                    is_default_browser: self
                        .state
                        .platform
                        .default_browser()
                        .is_linkpilot_default()
                        .unwrap_or(false),
                    config_path: Some(self.state.config.path().display().to_string()),
                    installed_browser_count: self
                        .state
                        .platform
                        .browser_inventory()
                        .installed_browsers()
                        .map(|v| v.len())
                        .unwrap_or(0),
                    ipc_socket_path: match linkpilot_ipc::path::default_endpoint() {
                        linkpilot_ipc::path::Endpoint::UnixSocket(p) => {
                            Some(p.display().to_string())
                        }
                        linkpilot_ipc::path::Endpoint::NamedPipe(s) => Some(s),
                    },
                },
            },

            Request::StatePing { request_id } => Response::Pong {
                request_id,
                daemon_version: self.version.clone(),
            },
        }
    }
}
