//! IPC transport for LinkPilot.
//!
//! Wire format: length-prefixed JSON (`u32` BE length + UTF-8 JSON payload).
//! Carrier:
//! - macOS / Linux: Unix Domain Socket
//! - Windows:       Named Pipe (NYI in v0.1; daemon falls back to "no IPC")

pub mod client;
pub mod path;
pub mod server;
pub mod transport;

pub use linkpilot_core::protocol::{Event, Request, Response, PROTOCOL_VERSION};
