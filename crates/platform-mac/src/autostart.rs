//! macOS "Launch at Login" via a `~/Library/LaunchAgents/<id>.plist`.
//!
//! SMAppService (macOS 13+) would be the modern API but requires a sandboxed
//! / signed bundle; the LaunchAgent approach works for an unsigned dev build
//! and is forward-compatible.

use std::path::{Path, PathBuf};

use linkpilot_core::platform::{Autostart, PlatformError, Result};

/// Distinguishes a login launch from an explicit request to open the UI.
pub const LAUNCH_AT_LOGIN_ARG: &str = "--launch-at-login";

pub struct MacAutostart {
    label: String,
}

impl MacAutostart {
    pub fn new(label: String) -> Self {
        Self { label }
    }

    fn plist_path(&self) -> Result<PathBuf> {
        let home =
            std::env::var_os("HOME").ok_or_else(|| PlatformError::Other("HOME not set".into()))?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{}.plist", self.label)))
    }
}

impl Autostart for MacAutostart {
    fn is_enabled(&self) -> Result<bool> {
        Ok(self.plist_path()?.exists())
    }

    fn set_enabled(&self, on: bool) -> Result<()> {
        let path = self.plist_path()?;
        if !on {
            match std::fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(PlatformError::Io(err)),
            }
        } else {
            let exe = std::env::current_exe().map_err(PlatformError::Io)?;
            let body = launch_agent_plist(&self.label, &exe);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(PlatformError::Io)?;
            }
            std::fs::write(&path, body).map_err(PlatformError::Io)?;
            Ok(())
        }
    }
}

fn launch_agent_plist(label: &str, exe: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>{LAUNCH_AT_LOGIN_ARG}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#,
        label = label,
        exe = exe.display(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::process::{Command, Stdio};

    #[test]
    fn login_launch_passes_background_marker_as_a_separate_argument() {
        let exe = Path::new("/Applications/LinkPilot 测试.app/Contents/MacOS/linkpilot-desktop");
        let body = launch_agent_plist("app.linkpilot.desktop", exe);
        let mut child = Command::new("/usr/bin/plutil")
            .args(["-convert", "json", "-o", "-", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(body.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let plist: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(plist["Label"], "app.linkpilot.desktop");
        assert_eq!(
            plist["ProgramArguments"],
            serde_json::json!([exe.to_str().unwrap(), LAUNCH_AT_LOGIN_ARG])
        );
        assert_eq!(plist["RunAtLoad"], true);
        assert_eq!(plist["KeepAlive"], false);
    }
}
