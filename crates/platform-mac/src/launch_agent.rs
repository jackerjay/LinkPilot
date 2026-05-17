//! `~/Library/LaunchAgents/app.linkpilot.daemon.plist` management.
//!
//! Owns the daemon LaunchAgent's lifecycle: write the plist, load it via
//! `launchctl`, query its run state, and (on uninstall) unload + delete.
//! The GUI auto-installs this on first run so users get a background
//! daemon without any CLI ceremony; `lp daemon install` / `uninstall`
//! (M2) call the same functions for symmetry.
//!
//! Separate from `autostart.rs` (which manages the GUI's own launch-at-
//! login plist) because the two services are independent: a user can have
//! the daemon auto-start but skip auto-opening LinkPilot.app, or vice
//! versa.

use std::path::{Path, PathBuf};
use std::process::Command;

use linkpilot_core::platform::{PlatformError, Result};

pub const DAEMON_LABEL: &str = "app.linkpilot.daemon";

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct LaunchAgentStatus {
    pub plist_exists: bool,
    /// True if `launchctl list <label>` finds the agent. Stale plist
    /// files (e.g. after a manual `launchctl unload`) read as false here.
    pub loaded: bool,
    /// PID of the running daemon process when launchd has it active.
    pub pid: Option<i32>,
}

/// Write the daemon's plist and try to `launchctl load -w` it.
///
/// Idempotent — calling twice on the same `exec_path` produces the same
/// plist and a single loaded LaunchAgent. If `launchctl load` fails (e.g.
/// daemon is already loaded), we leave the plist in place and return
/// Ok so the caller can decide whether to surface it.
pub fn install_daemon(exec_path: &Path) -> Result<()> {
    let plist = daemon_plist_path()?;
    let logs = log_dir()?;
    std::fs::create_dir_all(&logs).map_err(PlatformError::Io)?;

    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>--serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>ProcessType</key>
    <string>Background</string>
    <key>StandardOutPath</key>
    <string>{stdout}</string>
    <key>StandardErrorPath</key>
    <string>{stderr}</string>
</dict>
</plist>
"#,
        label = DAEMON_LABEL,
        exe = exec_path.display(),
        stdout = logs.join("daemon.out.log").display(),
        stderr = logs.join("daemon.err.log").display(),
    );

    if let Some(parent) = plist.parent() {
        std::fs::create_dir_all(parent).map_err(PlatformError::Io)?;
    }
    std::fs::write(&plist, body).map_err(PlatformError::Io)?;

    // Best-effort unload-then-load makes this idempotent against a
    // previous plist that's already running with a different exec path.
    let _ = Command::new("launchctl")
        .args(["unload", "-w"])
        .arg(&plist)
        .output();
    let out = Command::new("launchctl")
        .args(["load", "-w"])
        .arg(&plist)
        .output()
        .map_err(PlatformError::Io)?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        tracing::warn!(%stderr, "launchctl load failed; plist installed but daemon not started");
    }
    Ok(())
}

/// `launchctl unload` and delete the plist. No-op if the plist doesn't
/// exist. Does not kill the daemon process directly — `launchctl unload`
/// signals it to exit; if it ignores SIGTERM the caller can still pkill.
pub fn uninstall_daemon() -> Result<()> {
    let plist = daemon_plist_path()?;
    if !plist.exists() {
        return Ok(());
    }
    let _ = Command::new("launchctl")
        .args(["unload", "-w"])
        .arg(&plist)
        .output();
    std::fs::remove_file(&plist).map_err(PlatformError::Io)?;
    Ok(())
}

/// Report what the OS thinks of the daemon LaunchAgent right now.
pub fn daemon_status() -> Result<LaunchAgentStatus> {
    let plist = daemon_plist_path()?;
    let plist_exists = plist.exists();
    if !plist_exists {
        return Ok(LaunchAgentStatus::default());
    }
    // `launchctl list <label>` exits 0 + prints a plist-like dict when
    // the agent is loaded, exits non-zero otherwise. We grep the `PID`
    // key — present only when the daemon is actively running.
    let out = Command::new("launchctl")
        .args(["list", DAEMON_LABEL])
        .output()
        .map_err(PlatformError::Io)?;
    if !out.status.success() {
        return Ok(LaunchAgentStatus {
            plist_exists: true,
            loaded: false,
            pid: None,
        });
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let pid = text
        .lines()
        .find(|l| l.trim_start().starts_with("\"PID\""))
        .and_then(|l| l.split('=').nth(1))
        .and_then(|p| p.trim().trim_end_matches(';').parse::<i32>().ok());
    Ok(LaunchAgentStatus {
        plist_exists: true,
        loaded: true,
        pid,
    })
}

pub fn daemon_plist_path() -> Result<PathBuf> {
    let home =
        std::env::var_os("HOME").ok_or_else(|| PlatformError::Other("HOME not set".into()))?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{DAEMON_LABEL}.plist")))
}

fn log_dir() -> Result<PathBuf> {
    let home =
        std::env::var_os("HOME").ok_or_else(|| PlatformError::Other("HOME not set".into()))?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Logs")
        .join("LinkPilot"))
}
