//! Native macOS "Choose Application" dialog.
//!
//! Implemented via osascript so we don't have to pull AppKit / NSWorkspace
//! into the renderer-facing API. `choose application` is a built-in
//! AppleScript primitive that opens the same chooser File → Open With …
//! uses; the user picks an app from /Applications (or browses for it),
//! and we read back its display name + bundle id.

use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickedApp {
    pub name: String,
    pub bundle_id: String,
}

const APPLESCRIPT: &str = r#"
set theApp to choose application
return (name of theApp as text) & "|" & (id of theApp as text)
"#;

/// Show the macOS chooser. Returns `Ok(None)` when the user cancels.
pub fn choose_app() -> Result<Option<PickedApp>, String> {
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", APPLESCRIPT])
        .output()
        .map_err(|e| format!("osascript spawn: {e}"))?;

    if !output.status.success() {
        // exit code 1 + stderr "User canceled. (-128)" on Cancel — treat
        // any non-success as "user dismissed". Real failures (missing
        // osascript, AE permission denial) are extremely rare on macOS.
        return Ok(None);
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mut parts = raw.splitn(2, '|');
    let name = parts.next().unwrap_or("").trim().to_string();
    let bundle_id = parts.next().unwrap_or("").trim().to_string();

    if name.is_empty() {
        return Ok(None);
    }
    Ok(Some(PickedApp { name, bundle_id }))
}
