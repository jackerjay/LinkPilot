//! macOS "Launch at Login" via a `~/Library/LaunchAgents/<id>.plist`.
//!
//! SMAppService (macOS 13+) would be the modern API but requires a sandboxed
//! / signed bundle; the LaunchAgent approach works for an unsigned dev build
//! and is forward-compatible.

use std::path::PathBuf;

use linkpilot_core::platform::{Autostart, PlatformError, Result};

pub struct MacAutostart {
    label: String,
}

impl MacAutostart {
    pub fn new(label: String) -> Self {
        Self { label }
    }

    fn plist_path(&self) -> Result<PathBuf> {
        let home = std::env::var_os("HOME")
            .ok_or_else(|| PlatformError::Other("HOME not set".into()))?;
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
                label = self.label,
                exe = exe.display(),
            );
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(PlatformError::Io)?;
            }
            std::fs::write(&path, body).map_err(PlatformError::Io)?;
            Ok(())
        }
    }
}
