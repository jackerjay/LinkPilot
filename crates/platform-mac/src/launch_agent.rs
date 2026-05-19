//! `~/Library/LaunchAgents/app.linkpilot.daemon.plist` management.
//!
//! Owns the daemon LaunchAgent's lifecycle: write the plist, load it via
//! `launchctl`, query its run state, and (on uninstall) unload + delete.
//! The GUI auto-installs this on first run so users get a background
//! daemon without any CLI ceremony; `lpt daemon install` / `uninstall`
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
    /// Resolved `ProgramArguments[0]` from the installed plist — the
    /// daemon binary launchd will exec. None if the plist is missing or
    /// malformed. Read by `lpt daemon status` so users can tell which
    /// build of the daemon they're running against.
    pub exec_path: Option<PathBuf>,
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
    let exec_path = read_exec_path_from_plist(&plist).ok().flatten();
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
            exec_path,
        });
    }
    let pid = parse_pid_from_launchctl_output(&String::from_utf8_lossy(&out.stdout));
    Ok(LaunchAgentStatus {
        plist_exists: true,
        loaded: true,
        pid,
        exec_path,
    })
}

/// Pull `ProgramArguments[0]` out of an installed daemon plist. Pure
/// string parsing — Apple's plist format here is small and stable, and
/// we want zero plist-crate deps in platform-mac. Returns `Ok(None)`
/// for any structural mismatch (older / malformed file).
pub(crate) fn read_exec_path_from_plist(path: &Path) -> std::io::Result<Option<PathBuf>> {
    let body = std::fs::read_to_string(path)?;
    // Find `<key>ProgramArguments</key>` then the first `<string>` after
    // it before the closing `</array>`.
    let Some(idx) = body.find("<key>ProgramArguments</key>") else {
        return Ok(None);
    };
    let tail = &body[idx..];
    let Some(arr_end) = tail.find("</array>") else {
        return Ok(None);
    };
    let arr = &tail[..arr_end];
    let Some(start) = arr.find("<string>") else {
        return Ok(None);
    };
    let after = &arr[start + "<string>".len()..];
    let Some(end) = after.find("</string>") else {
        return Ok(None);
    };
    let raw = &after[..end];
    if raw.is_empty() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(raw)))
}

/// Parse the `PID = NNNN;` line out of `launchctl list <label>` stdout.
/// Returns None if the agent is loaded but not currently running, or
/// the format changed in a future macOS.
pub(crate) fn parse_pid_from_launchctl_output(text: &str) -> Option<i32> {
    text.lines()
        .find(|l| l.trim_start().starts_with("\"PID\""))
        .and_then(|l| l.split('=').nth(1))
        .and_then(|p| p.trim().trim_end_matches(';').parse::<i32>().ok())
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn tmp_plist() -> PathBuf {
        std::env::temp_dir().join(format!("linkpilot-plist-test-{}.plist", Uuid::new_v4()))
    }

    fn sample_plist(exec_path: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exec_path}</string>
        <string>--serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#,
            label = DAEMON_LABEL,
        )
    }

    #[test]
    fn read_exec_path_picks_first_program_argument() {
        let path = tmp_plist();
        std::fs::write(
            &path,
            sample_plist("/Applications/LinkPilot.app/Contents/MacOS/linkpilot-daemon"),
        )
        .unwrap();
        let exec = read_exec_path_from_plist(&path).unwrap().unwrap();
        assert_eq!(
            exec,
            PathBuf::from("/Applications/LinkPilot.app/Contents/MacOS/linkpilot-daemon")
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn read_exec_path_missing_program_arguments_is_none() {
        let path = tmp_plist();
        std::fs::write(
            &path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>app.linkpilot.daemon</string>
</dict>
</plist>
"#,
        )
        .unwrap();
        assert!(read_exec_path_from_plist(&path).unwrap().is_none());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn read_exec_path_io_error_propagates() {
        let path = tmp_plist();
        // Path doesn't exist — NotFound bubbles up as Err.
        assert!(read_exec_path_from_plist(&path).is_err());
    }

    #[test]
    fn parse_pid_extracts_integer() {
        // Sample shape of `launchctl list app.linkpilot.daemon` stdout.
        let out = r#"{
	"LimitLoadToSessionType" = "Aqua";
	"Label" = "app.linkpilot.daemon";
	"OnDemand" = false;
	"LastExitStatus" = 0;
	"PID" = 41721;
	"Program" = "/Applications/LinkPilot.app/Contents/MacOS/linkpilot-daemon";
};"#;
        assert_eq!(parse_pid_from_launchctl_output(out), Some(41721));
    }

    #[test]
    fn parse_pid_none_when_not_running() {
        // Loaded-but-not-running output has no PID key.
        let out = r#"{
	"Label" = "app.linkpilot.daemon";
	"OnDemand" = false;
	"LastExitStatus" = 0;
};"#;
        assert_eq!(parse_pid_from_launchctl_output(out), None);
    }

    #[test]
    fn parse_pid_ignores_garbage() {
        assert_eq!(
            parse_pid_from_launchctl_output("totally not launchctl"),
            None
        );
        assert_eq!(parse_pid_from_launchctl_output(""), None);
    }
}
