//! LinkPilot core: routing engine, configuration model, and platform traits.
//!
//! This crate is intentionally platform-agnostic. All OS-specific behaviour
//! flows through traits in [`platform`]; concrete implementations live in the
//! `platform-mac` / `platform-win` / `platform-linux` crates.

pub mod browser;
pub mod config;
pub mod history;
pub mod inventory;
pub mod platform;
pub mod protocol;
pub mod routing;
pub mod rules;

pub use browser::{BrowserId, BrowserKind, BrowserProfile, BrowserTarget, InstalledBrowser};
pub use config::ConfigDocument;
pub use routing::{Router, RoutingContext, RoutingDecision};
pub use rules::Rule;
