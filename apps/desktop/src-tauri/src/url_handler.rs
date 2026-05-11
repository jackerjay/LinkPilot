//! Receive macOS `open URL` events and dispatch them to the router.
//!
//! Wired in step 6 once the platform crate exposes a real `UrlLauncher` and
//! the Info.plist registers `http`/`https` schemes.

#![allow(dead_code)]

use linkpilot_core::routing::{RoutingContext, RoutingDecision};

pub fn dispatch(_context: RoutingContext) -> RoutingDecision {
    RoutingDecision::Allow {
        reason: "url_handler stub".into(),
    }
}
