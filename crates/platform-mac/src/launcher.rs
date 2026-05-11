use std::process::{Command, Stdio};

use linkpilot_core::browser::{BrowserKind, BrowserTarget};
use linkpilot_core::platform::{BrowserInventory, PlatformError, Result, UrlLauncher};
use url::Url;

use crate::inventory::MacInventory;

pub struct MacUrlLauncher;

impl UrlLauncher for MacUrlLauncher {
    fn open(&self, target: &BrowserTarget, url: &Url) -> Result<()> {
        let inventory = MacInventory;
        let browsers = inventory.installed_browsers()?;
        let browser = browsers
            .iter()
            .find(|b| b.id == target.browser)
            .ok_or_else(|| {
                PlatformError::Other(format!(
                    "browser '{}' is not installed on this Mac",
                    target.browser
                ))
            })?;

        let url_str = url.as_str();
        let cmd = match browser.kind {
            BrowserKind::Chromium | BrowserKind::Arc => {
                // Launching the binary directly always honours --profile-directory,
                // even when an instance is already running. Chrome / Edge / Arc /
                // Brave all share these flags.
                let mut c = Command::new(&browser.executable);
                if let Some(profile) = &target.profile {
                    c.arg(format!("--profile-directory={profile}"));
                }
                if target.new_window {
                    c.arg("--new-window");
                }
                if target.incognito {
                    c.arg("--incognito");
                }
                c.arg(url_str);
                c
            }
            BrowserKind::Firefox => {
                let mut c = Command::new(&browser.executable);
                if let Some(profile) = &target.profile {
                    c.arg("-P").arg(profile);
                }
                if target.incognito {
                    c.arg("-private-window").arg(url_str);
                } else if target.new_window {
                    c.arg("-new-window").arg(url_str);
                } else {
                    c.arg("-url").arg(url_str);
                }
                c
            }
            BrowserKind::Safari => {
                // Safari ignores positional URL args on the binary; defer to `open`.
                let mut c = Command::new("/usr/bin/open");
                c.arg("-a").arg(&browser.display_name).arg(url_str);
                c
            }
            BrowserKind::Unknown => {
                let mut c = Command::new("/usr/bin/open");
                c.arg("-a").arg(&browser.display_name).arg(url_str);
                c
            }
        };

        spawn_detached(cmd)
    }
}

fn spawn_detached(mut cmd: Command) -> Result<()> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    tracing::debug!(?cmd, "spawning browser");
    let _child = cmd.spawn().map_err(PlatformError::Io)?;
    // Browser child runs independently of LinkPilot; do not `wait`.
    Ok(())
}
