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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::store::{ConfigError, Result as ConfigResult};
use crate::config::{default_config_path, ConfigStore};
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

    /// Convenience constructor used by the standalone `linkpilot-daemon`
    /// binary. Resolves the default config path (or uses `config_path`
    /// override), loads-or-initialises the ConfigStore, creates a fresh
    /// RouteHistory, and ties the supplied platform provider together.
    ///
    /// Errors when the config path can't be resolved (e.g. no `$HOME`)
    /// or the file is unreadable / malformed JSON.
    pub fn bootstrap(
        config_path: Option<PathBuf>,
        platform: Arc<dyn PlatformProvider>,
        version: impl Into<String>,
    ) -> Result<(Self, bool), crate::config::store::ConfigError> {
        let path = match config_path {
            Some(p) => p,
            None => default_config_path()?,
        };
        let (config, created) = ConfigStore::load_or_init(path)?;
        Ok((
            Self::new(config, Arc::new(RouteHistory::new()), platform, version),
            created,
        ))
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

            Request::RouteHistory { request_id, limit } => {
                // Default to 100 records — matches design §14.2.2 and
                // is comfortably below the buffer's 1000-entry cap.
                let n = limit.unwrap_or(100);
                let records = self.history.recent(n);
                Response::RouteHistorySnapshot {
                    request_id,
                    records,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PID file management
//
// Shared between the standalone `linkpilot-daemon` binary (writes/cleans
// on startup + shutdown) and the `lp daemon ...` CLI (reads for status /
// stop). The socket remains the source of truth for "is the daemon
// alive?" — a successful StatePing is canonical — but the PID file is
// what the CLI uses to *signal* a running daemon, and what reveals a
// half-dead daemon (socket bind failed but process still up) for the
// diagnostics.
// ---------------------------------------------------------------------------

/// Where the daemon writes its PID file. Same directory as the config
/// file (`~/Library/Application Support/LinkPilot/...` on macOS). The
/// path is platform-derived, not user-configurable — CLI subcommands
/// must be able to locate it without any config plumbing.
pub fn pid_file_path() -> ConfigResult<PathBuf> {
    let cfg = default_config_path()?;
    let dir = cfg.parent().ok_or(ConfigError::NoDefaultDir)?;
    Ok(dir.join("linkpilot-daemon.pid"))
}

/// Atomically write the current process's PID to `path`. Caller is
/// responsible for resolving the path (typically via [`pid_file_path`]).
///
/// Uses temp + rename so a reader that opens the file mid-write never
/// sees a truncated integer. Creates the parent directory if missing
/// — the daemon may run before the config dir has ever been touched.
pub fn write_pid_file(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = pid_tmp_path(path);
    std::fs::write(&tmp, std::process::id().to_string())?;
    std::fs::rename(&tmp, path)
}

/// Read the PID stored at `path`. Returns:
/// - `Ok(Some(pid))` — file exists and parses as a positive u32.
/// - `Ok(None)` — file is missing, empty, or unparseable. The caller
///   should treat all three the same (no live daemon claimed).
/// - `Err(e)` — IO errors other than `NotFound`.
pub fn read_pid_file(path: &Path) -> std::io::Result<Option<u32>> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s.trim().parse::<u32>().ok()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Remove the PID file. `NotFound` is not an error — the daemon may
/// have been killed before reaching its shutdown hook.
pub fn remove_pid_file(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Check whether the PID file at `path` is stale. If it names a dead
/// process, unlink it and return `Ok(true)`. If the named process is
/// alive, leave the file and return `Ok(false)`. Missing / unparseable
/// file is also `Ok(false)`.
///
/// Call this before [`write_pid_file`] on daemon startup so a
/// previously-crashed daemon's PID doesn't keep a `lp daemon status`
/// thinking we're "running" forever.
pub fn cleanup_stale_pid_file(path: &Path) -> std::io::Result<bool> {
    let Some(pid) = read_pid_file(path)? else {
        return Ok(false);
    };
    if process_is_alive(pid) {
        return Ok(false);
    }
    remove_pid_file(path)?;
    Ok(true)
}

/// True if the given PID is a running process owned by the current
/// user (or a process we can't signal but that exists — EPERM).
///
/// Unix: `kill(pid, 0)` — sig 0 is the documented permission/existence
/// probe; no signal is delivered. Non-unix is a stub: the daemon-mgmt
/// surface only ships on macOS in v0.2, so the CLI will refuse to run
/// on Windows long before this matters.
#[cfg(unix)]
pub fn process_is_alive(pid: u32) -> bool {
    // libc::pid_t is i32 on all supported targets. PIDs that overflow
    // i32 (>= 2^31) aren't reachable on macOS / Linux.
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    let res = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if res == 0 {
        return true;
    }
    // EPERM = process exists but we lack signal permission. For the
    // daemon (user-scoped LaunchAgent runs as the same user) this is
    // unreachable, but we treat it as "alive" defensively so a
    // privileged-but-uncooperative process isn't mistaken for dead.
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
pub fn process_is_alive(_pid: u32) -> bool {
    // Daemon mgmt is macOS-only in v0.2; pretend alive so callers never
    // unlink a PID file they shouldn't on platforms where we can't tell.
    true
}

fn pid_tmp_path(path: &Path) -> PathBuf {
    let mut tmp = path.to_path_buf();
    let mut name = path
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    tmp.set_file_name(name);
    tmp
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

    fn tmp_pid_path() -> PathBuf {
        std::env::temp_dir().join(format!("linkpilot-pid-test-{}.pid", Uuid::new_v4()))
    }

    #[test]
    fn write_then_read_pid_round_trips() {
        let path = tmp_pid_path();
        write_pid_file(&path).unwrap();
        let pid = read_pid_file(&path).unwrap();
        assert_eq!(pid, Some(std::process::id()));
        remove_pid_file(&path).unwrap();
    }

    #[test]
    fn cleanup_stale_unlinks_dead_pid() {
        let path = tmp_pid_path();
        // PID 999999 is virtually guaranteed to not exist (PID_MAX is
        // 99999 on macOS by default, much lower on Linux out of the box).
        std::fs::write(&path, "999999").unwrap();
        let removed = cleanup_stale_pid_file(&path).unwrap();
        assert!(removed, "should unlink stale PID file");
        assert!(!path.exists());
    }

    #[test]
    fn cleanup_stale_keeps_live_pid() {
        let path = tmp_pid_path();
        // Our own PID is definitely alive.
        std::fs::write(&path, std::process::id().to_string()).unwrap();
        let removed = cleanup_stale_pid_file(&path).unwrap();
        assert!(!removed, "live PID should not be unlinked");
        assert!(path.exists());
        remove_pid_file(&path).unwrap();
    }

    #[test]
    fn cleanup_stale_missing_file_is_noop() {
        let path = tmp_pid_path();
        let removed = cleanup_stale_pid_file(&path).unwrap();
        assert!(!removed);
    }

    #[test]
    fn read_pid_returns_none_for_garbage() {
        let path = tmp_pid_path();
        std::fs::write(&path, "not a number\n").unwrap();
        assert_eq!(read_pid_file(&path).unwrap(), None);
        remove_pid_file(&path).unwrap();
    }

    #[test]
    fn remove_pid_missing_is_ok() {
        let path = tmp_pid_path();
        assert!(remove_pid_file(&path).is_ok());
    }

    fn ctx(url: &str) -> RoutingContext {
        RoutingContext {
            url: url.into(),
            source: crate::routing::Source {
                kind: crate::routing::SourceKind::Cli,
                app_name: None,
                bundle_id: None,
                browser: None,
                profile: None,
            },
            navigation: None,
            environment: None,
        }
    }

    #[test]
    fn route_history_verb_returns_newest_first() {
        let rt = fixture();
        // Log three records so we can spot ordering.
        for url in [
            "https://a.example.com",
            "https://b.example.com",
            "https://c.example.com",
        ] {
            let _ = rt.evaluate_and_log(ctx(url));
        }
        let resp = rt.handle(Request::RouteHistory {
            request_id: "h".into(),
            limit: Some(10),
        });
        match resp {
            Response::RouteHistorySnapshot {
                request_id,
                records,
            } => {
                assert_eq!(request_id, "h");
                assert_eq!(records.len(), 3);
                // Newest first — `c` was logged last.
                assert_eq!(records[0].context.url, "https://c.example.com");
                assert_eq!(records[2].context.url, "https://a.example.com");
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn route_history_default_limit_is_finite() {
        // Verb without a limit must not return an unbounded snapshot;
        // protocol contract is "daemon picks 100 by default" (design §14.2.2).
        let rt = fixture();
        for _ in 0..150 {
            let _ = rt.evaluate_and_log(ctx("https://example.com"));
        }
        let resp = rt.handle(Request::RouteHistory {
            request_id: "h".into(),
            limit: None,
        });
        match resp {
            Response::RouteHistorySnapshot { records, .. } => {
                assert_eq!(records.len(), 100, "default cap");
            }
            other => panic!("unexpected: {other:?}"),
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
