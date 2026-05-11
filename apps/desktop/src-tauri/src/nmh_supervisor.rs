//! Installs Native Messaging Host manifests into the right per-browser
//! locations (Chrome / Edge / Arc / Firefox / etc.) so the bundled
//! `linkpilot-native-host` binary can be discovered.
//!
//! Lands in v0.3 with the Chromium extension.

#![allow(dead_code)]

pub fn install_manifests() -> anyhow::Result<()> {
    Ok(())
}
