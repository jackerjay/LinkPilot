//! Linux backend stub. Real implementation arrives in v0.6+.

#![cfg(target_os = "linux")]

pub use linkpilot_core::platform::StubProvider as LinuxProvider;
