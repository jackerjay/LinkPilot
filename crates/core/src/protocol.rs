//! IPC message types shared by daemon, CLI, native host, and (eventually)
//! the Tauri frontend. Wire format is length-prefixed JSON; encoding lives
//! in `linkpilot-ipc`.

use serde::{Deserialize, Serialize};

use crate::config::{ConfigDocument, WriterId};
use crate::routing::{RoutingContext, RoutingDecision};

pub const PROTOCOL_VERSION: u32 = 1;

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
