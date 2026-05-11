//! IPC transport for LinkPilot.
//!
//! Wire format: length-prefixed JSON, identical on all platforms. The carrier
//! differs — Unix Domain Socket on macOS/Linux, Named Pipe on Windows.
//!
//! v0.1 step 1 ships only the protocol surface and the socket-path resolver;
//! the actual `tokio` server/client land in step 4.

pub mod path;
pub mod transport;

pub use linkpilot_core::protocol::{Event, Request, Response, PROTOCOL_VERSION};
