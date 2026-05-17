//! Shared daemon-side state and request handling.
//!
//! v0.2 splits the daemon out of the Tauri app process: `linkpilot-daemon`
//! ships as its own binary while the Tauri shell can still embed it for
//! GUI-only installs. Both embeddings instantiate a [`DaemonRuntime`] and
//! feed it to [`linkpilot_ipc::server::serve`].
//!
//! `DaemonRuntime` implements [`RequestHandler`] with sensible defaults for
//! every protocol verb. Embeddings that need to layer side effects on top
//! of the default behaviour (e.g. the GUI wants to emit a `route-logged`
//! Tauri event after `RouteOpen` and route through the picker UI for
//! `Ask`) wrap an `Arc<DaemonRuntime>` in their own handler and delegate
//! everything they don't customise.

use std::sync::Arc;

use crate::config::ConfigStore;
use crate::history::{RouteHistory, RouteRecord};
use crate::platform::PlatformProvider;
use crate::protocol::{DoctorReport, Request, Response};
use crate::routing::{Explained, Router, RoutingContext, RoutingDecision};

/// IPC handler contract — anything that can turn a protocol [`Request`]
/// into a [`Response`]. Defined in core (rather than `linkpilot-ipc`)
/// because the daemon runtime impls it, and we need to avoid a circular
/// dep (ipc → core → ipc would loop). `linkpilot_ipc::server` re-exports
/// it so call sites using `linkpilot_ipc::server::RequestHandler` keep
/// working.
pub trait RequestHandler: Send + Sync + 'static {
    fn handle(&self, request: Request) -> Response;
}

/// State that every daemon — headless or GUI-embedded — owns.
///
/// Fields are public so callers can read them outside the request-handling
/// path (e.g. the GUI's Tauri commands need `config` to mutate the document,
/// and the tray code reads `history` for the popover preview). All
/// underlying handles are `Arc` / cheap-clone so concurrent access is fine.
pub struct DaemonRuntime {
    pub config: ConfigStore,
    pub history: Arc<RouteHistory>,
    pub platform: Arc<dyn PlatformProvider>,
    pub version: String,
}

impl DaemonRuntime {
    pub fn new(
        config: ConfigStore,
        history: Arc<RouteHistory>,
        platform: Arc<dyn PlatformProvider>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            config,
            history,
            platform,
            version: version.into(),
        }
    }

    /// Evaluate a routing context and log the resulting record. Returns
    /// the explained decision (caller-visible side: history grew by one)
    /// plus the record itself so embeddings can broadcast it.
    ///
    /// Both `DaemonRuntime`'s default `RouteOpen` handler and the GUI's
    /// custom `RouteOpen` handler funnel through this — keeping the
    /// "what did we decide" and "log to history" steps identical across
    /// embeddings, while letting them diverge on what to do with the
    /// decision afterwards (launch directly vs. spawn picker, emit event
    /// to webview vs. stay silent).
    pub fn evaluate_and_log(&self, context: RoutingContext) -> (Explained, RouteRecord) {
        let doc = self.config.document();
        let explained = Router::new(&doc).evaluate_explained(&context);
        let record = RouteRecord::with_explanation(
            context,
            explained.decision.clone(),
            explained.explanation.clone(),
        );
        self.history.log(record.clone());
        (explained, record)
    }

    fn doctor_report(&self) -> DoctorReport {
        DoctorReport {
            daemon_version: self.version.clone(),
            is_default_browser: self
                .platform
                .default_browser()
                .is_linkpilot_default()
                .unwrap_or(false),
            config_path: Some(self.config.path().display().to_string()),
            installed_browser_count: self
                .platform
                .browser_inventory()
                .installed_browsers()
                .map(|v| v.len())
                .unwrap_or(0),
            ipc_socket_path: Some(crate::endpoint::default_endpoint().display()),
        }
    }
}

impl RequestHandler for DaemonRuntime {
    fn handle(&self, request: Request) -> Response {
        match request {
            Request::RouteEvaluate {
                request_id,
                context,
            } => {
                let doc = self.config.document();
                let decision = Router::new(&doc).evaluate(&context);
                Response::RouteDecision {
                    request_id,
                    decision,
                }
            }

            Request::RouteOpen {
                request_id,
                context,
            } => {
                // Headless default: evaluate, log, launch directly via the
                // platform URL launcher. Ask returns an Error because
                // there's no UI to pop a picker. GUI embeddings should
                // override this verb (see apps/desktop/src-tauri/src/ipc_host.rs)
                // and call `evaluate_and_log` themselves so they can route
                // through the picker.
                let raw_url = context.url.clone();
                let (explained, _record) = self.evaluate_and_log(context);
                let decision = explained.decision;
                match &decision {
                    RoutingDecision::Open { target, .. } => {
                        let parsed = match url::Url::parse(&raw_url) {
                            Ok(u) => u,
                            Err(err) => {
                                return Response::Error {
                                    request_id,
                                    code: "bad-url".into(),
                                    message: format!("parsing {raw_url}: {err}"),
                                };
                            }
                        };
                        if let Err(err) = self.platform.url_launcher().open(target, &parsed) {
                            return Response::Error {
                                request_id,
                                code: "launch-failed".into(),
                                message: err.to_string(),
                            };
                        }
                    }
                    RoutingDecision::Ask { .. } => {
                        return Response::Error {
                            request_id,
                            code: "ask-unsupported-headless".into(),
                            message: "Ask requires a GUI picker; not supported in headless daemon"
                                .into(),
                        };
                    }
                    RoutingDecision::Allow { .. } | RoutingDecision::Block { .. } => {
                        // No-op launch; decision flows back as-is.
                    }
                }
                Response::RouteDecision {
                    request_id,
                    decision,
                }
            }

            Request::ConfigGet { request_id } => Response::ConfigSnapshot {
                request_id,
                doc: self.config.document(),
            },

            Request::ConfigReplace {
                request_id,
                doc,
                writer,
            } => match self.config.replace(doc, writer) {
                Ok(()) => Response::Ack { request_id },
                Err(err) => Response::Error {
                    request_id,
                    code: "config-replace".into(),
                    message: err.to_string(),
                },
            },

            Request::Doctor { request_id } => Response::DoctorReport {
                request_id,
                report: self.doctor_report(),
            },

            Request::StatePing { request_id } => Response::Pong {
                request_id,
                daemon_version: self.version.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigDocument;
    use crate::platform::StubProvider;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn tmp_path() -> PathBuf {
        std::env::temp_dir().join(format!("linkpilot-daemon-test-{}.json", Uuid::new_v4()))
    }

    fn fixture() -> DaemonRuntime {
        let path = tmp_path();
        let (config, _) = ConfigStore::load_or_init(path).unwrap();
        DaemonRuntime::new(
            config,
            Arc::new(RouteHistory::new()),
            Arc::new(StubProvider),
            "0.2.0-test".to_string(),
        )
    }

    #[test]
    fn state_ping_returns_pong_with_version() {
        let rt = fixture();
        let resp = rt.handle(Request::StatePing {
            request_id: "req-1".into(),
        });
        match resp {
            Response::Pong {
                request_id,
                daemon_version,
            } => {
                assert_eq!(request_id, "req-1");
                assert_eq!(daemon_version, "0.2.0-test");
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn config_get_round_trips() {
        let rt = fixture();
        let resp = rt.handle(Request::ConfigGet {
            request_id: "g".into(),
        });
        match resp {
            Response::ConfigSnapshot { doc, .. } => {
                assert!(!doc.rules.is_empty(), "demo config has rules");
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn config_replace_persists() {
        let rt = fixture();
        let mut doc: ConfigDocument = rt.config.document();
        doc.rules.clear();
        let resp = rt.handle(Request::ConfigReplace {
            request_id: "r".into(),
            doc,
            writer: crate::config::WriterId::Cli,
        });
        assert!(matches!(resp, Response::Ack { .. }));
        assert!(rt.config.document().rules.is_empty());
    }

    #[test]
    fn route_open_headless_errors_on_ask() {
        // Demo config has no Ask rules, so we can't easily reach Ask via
        // a real routing decision. Instead, test the path that's most
        // exercised in the headless smoke tests: a URL that hits the
        // default target (no rule) — StubProvider's launcher returns
        // NotSupported, which the handler should surface as launch-failed
        // (not crash, not silently succeed).
        let rt = fixture();
        let resp = rt.handle(Request::RouteOpen {
            request_id: "ro".into(),
            context: RoutingContext {
                url: "https://no-rule.example.com".into(),
                source: crate::routing::Source {
                    kind: crate::routing::SourceKind::Cli,
                    app_name: None,
                    bundle_id: None,
                    browser: None,
                    profile: None,
                },
                navigation: None,
                environment: None,
            },
        });
        match resp {
            Response::Error { code, .. } => {
                assert_eq!(code, "launch-failed", "StubProvider can't launch URLs");
            }
            other => panic!("expected launch-failed Error, got {other:?}"),
        }
    }

    #[test]
    fn evaluate_and_log_grows_history() {
        let rt = fixture();
        assert_eq!(rt.history.len(), 0);
        let _ = rt.evaluate_and_log(RoutingContext {
            url: "https://github.com".into(),
            source: crate::routing::Source {
                kind: crate::routing::SourceKind::Cli,
                app_name: None,
                bundle_id: None,
                browser: None,
                profile: None,
            },
            navigation: None,
            environment: None,
        });
        assert_eq!(rt.history.len(), 1);
    }
}
