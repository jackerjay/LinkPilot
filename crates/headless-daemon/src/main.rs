//! Placeholder for a future GUI-less daemon binary.
//!
//! In v0.1 the daemon lives inside the Tauri app process. This crate exists so
//! that splitting the daemon out later is a pure refactor — every other crate
//! already targets the IPC surface defined in `linkpilot-ipc`.

use anyhow::Result;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let endpoint = linkpilot_ipc::path::default_endpoint();
    tracing::info!(
        ?endpoint,
        "linkpilot-daemon placeholder (v0.1 runs inside Tauri)"
    );
    Ok(())
}
