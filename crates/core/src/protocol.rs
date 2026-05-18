//! IPC message types shared by daemon, CLI, native host, and (eventually)
//! the Tauri frontend. Wire format is length-prefixed JSON; encoding lives
//! in `linkpilot-ipc`.

use serde::{Deserialize, Serialize};

use crate::config::{ConfigDocument, WriterId};
use crate::history::RouteRecord;
use crate::routing::{RoutingContext, RoutingDecision};

/// Bumped in v0.2 (M3): adds the `RouteHistory` verb and the symmetric
/// `RouteHistorySnapshot` response. v0.2 daemon also sends a friendly
/// `Error { code: "unknown-verb", ... }` for unrecognised request
/// types (see `linkpilot-ipc::server`) — clients on protocol >= 2 can
/// rely on this; v0.1 daemons just drop the connection on a bad verb.
pub const PROTOCOL_VERSION: u32 = 2;

/// Stable error code the daemon sends when a request's `type` doesn't
/// match any known variant. CLIs match on this string rather than on
/// the message text — the message is for humans, the code is the API.
pub const ERROR_UNKNOWN_VERB: &str = "unknown-verb";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Request {
    /// Evaluate without side effects — returns a decision only.
    RouteEvaluate {
        request_id: String,
        context: RoutingContext,
    },
    /// Evaluate AND have the daemon launch the resulting target.
    RouteOpen {
        request_id: String,
        context: RoutingContext,
    },
    ConfigGet {
        request_id: String,
    },
    ConfigReplace {
        request_id: String,
        doc: ConfigDocument,
        writer: WriterId,
    },
    Doctor {
        request_id: String,
    },
    StatePing {
        request_id: String,
    },
    /// Read the daemon's in-memory route history (newest first). Added
    /// in protocol v2; older daemons answer with Error{code:"unknown-verb"}
    /// — when they speak v2+ at all. v0.1 daemons drop the connection.
    RouteHistory {
        request_id: String,
        /// Cap on records returned. `None` means "daemon default" (100).
        #[serde(default)]
        limit: Option<usize>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Response {
    RouteDecision {
        request_id: String,
        decision: RoutingDecision,
    },
    ConfigSnapshot {
        request_id: String,
        doc: ConfigDocument,
    },
    DoctorReport {
        request_id: String,
        report: DoctorReport,
    },
    Ack {
        request_id: String,
    },
    Pong {
        request_id: String,
        daemon_version: String,
    },
    Error {
        request_id: String,
        code: String,
        message: String,
    },
    /// Reply to `Request::RouteHistory`. Records are newest-first,
    /// trimmed to the requested limit.
    RouteHistorySnapshot {
        request_id: String,
        records: Vec<RouteRecord>,
    },
}

/// Broadcast events the daemon pushes to subscribers (Tauri front-end, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Event {
    ConfigChanged { writer: WriterId },
    RouteLogged,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DoctorReport {
    pub daemon_version: String,
    pub is_default_browser: bool,
    pub config_path: Option<String>,
    pub installed_browser_count: usize,
    pub ipc_socket_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_version_is_v2() {
        // Locks the bump done in M3; bumping again should be intentional
        // (and accompanied by a migration note + matching CLI/daemon
        // updates), so the constant lives behind a test rather than as
        // a free-floating literal.
        assert_eq!(PROTOCOL_VERSION, 2);
    }

    #[test]
    fn route_history_request_round_trips() {
        let req = Request::RouteHistory {
            request_id: "h-1".into(),
            limit: Some(5),
        };
        let s = serde_json::to_string(&req).unwrap();
        // Verify the wire form so a casual struct rename can't silently
        // change what clients on the wire see.
        assert!(s.contains(r#""type":"route-history""#), "wire form: {s}");
        assert!(s.contains(r#""request_id":"h-1""#));
        assert!(s.contains(r#""limit":5"#));
        let back: Request = serde_json::from_str(&s).unwrap();
        match back {
            Request::RouteHistory { request_id, limit } => {
                assert_eq!(request_id, "h-1");
                assert_eq!(limit, Some(5));
            }
            other => panic!("round-trip mismatch: {other:?}"),
        }
    }

    #[test]
    fn route_history_request_limit_defaults_to_none() {
        // Older clients (and the CLI default path) omit `limit`. v0.2
        // daemon must accept that and use its own default.
        let s = r#"{"type":"route-history","request_id":"x"}"#;
        let req: Request = serde_json::from_str(s).unwrap();
        match req {
            Request::RouteHistory { limit, .. } => assert_eq!(limit, None),
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn route_history_snapshot_round_trips() {
        let resp = Response::RouteHistorySnapshot {
            request_id: "h-1".into(),
            records: vec![],
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains(r#""type":"route-history-snapshot""#));
        let back: Response = serde_json::from_str(&s).unwrap();
        assert!(matches!(back, Response::RouteHistorySnapshot { .. }));
    }

    #[test]
    fn unknown_verb_error_is_a_real_response_shape() {
        // A daemon answering an unrecognised verb must produce a payload
        // that any client can deserialise as Response::Error and match
        // on the code field. Lock the shape.
        let resp = Response::Error {
            request_id: "abc".into(),
            code: ERROR_UNKNOWN_VERB.into(),
            message: "demo".into(),
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains(r#""code":"unknown-verb""#));
        let back: Response = serde_json::from_str(&s).unwrap();
        match back {
            Response::Error { code, .. } => assert_eq!(code, ERROR_UNKNOWN_VERB),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
