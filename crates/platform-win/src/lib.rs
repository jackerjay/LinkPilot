//! Windows backend stub. Real implementation arrives in v0.5.
//!
//! The stub re-exports the workspace-wide `StubProvider` under a Windows-
//! specific name so consumer crates can `cfg`-select consistently.

#![cfg(target_os = "windows")]

pub use linkpilot_core::platform::StubProvider as WinProvider;
