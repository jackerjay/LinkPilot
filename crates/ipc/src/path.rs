//! IPC endpoint resolution.
//!
//! The actual logic moved to `linkpilot_core::endpoint` in v0.2 so that
//! `linkpilot_core::daemon::DaemonRuntime` can use it without a circular
//! crate dependency. This module re-exports the same types so existing
//! call sites under `linkpilot_ipc::path::*` keep compiling unchanged.

pub use linkpilot_core::endpoint::{default_endpoint, Endpoint};
