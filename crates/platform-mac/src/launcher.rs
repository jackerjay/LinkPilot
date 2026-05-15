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
            BrowserKind::Chromium => {
                // Chrome / Edge / Brave: launching the binary directly
                // routes the URL to the existing instance AND honours
                // --profile-directory even when running, so we keep the
                // direct-exec path which is the only way to target a
                // specific profile from outside the browser.
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
            BrowserKind::Arc => {
                // Arc has its own single-instance enforcement — running
                // the binary while Arc is open pops "Arc is already open.
                // Only one instance of Arc can be opened at a time."
                // Use `open -a Arc` which sends the URL via Apple Events
                // to the running instance instead. Arc's Space / profile
                // routing is delegated to its built-in Air Traffic Control
                // (see PRD §23) — there's no stable external API to
                // target a specific Space from here.
                let mut c = Command::new("/usr/bin/open");
                c.arg("-a").arg(&browser.display_name).arg(url_str);
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

        spawn_detached(cmd)?;

        // Post-launch activate-target.
        //
        // Why this is here even though the spawn above already
        // delivers the URL: when LinkPilot is the currently-active
        // app (it always is right after the picker resolves, and
        // often is when the user clicked a link inside LinkPilot's
        // own UI), AppKit keeps LinkPilot's main window in the
        // foreground UNLESS the about-to-be-active browser pushes
        // through a strong activation. Chrome/Firefox direct-exec
        // doesn't issue one. `open -a Foo URL` (Arc/Safari path)
        // does, but if LinkPilot has a visible window AppKit can
        // re-promote LinkPilot the moment our NSApp.deactivate
        // returns. `open -b <bundle_id>` issues an LSLaunchURLs
        // activate-target call which is sticky against that
        // re-promotion — it's what `open -a` uses internally but
        // we invoke it standalone here so it doesn't matter what
        // launch strategy got chosen above.
        //
        // Redundant for Arc/Safari (`open -a` already activated),
        // critical for Chromium/Firefox. Skip when bundle_id is
        // unknown — those bundles usually aren't running anyway
        // and the spawn alone is the activation.
        if let Some(bundle_id) = &browser.platform_app_id {
            activate_by_bundle_id(bundle_id);
        }

        Ok(())
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

/// `/usr/bin/open -b <bundle-id>` → activate the running app with
/// that bundle id (or launch it if not running). Fire-and-forget;
/// failure is silently swallowed because the URL itself was already
/// delivered by the main spawn above — losing the activation boost
/// is degraded UX, not a hard error.
fn activate_by_bundle_id(bundle_id: &str) {
    let mut c = Command::new("/usr/bin/open");
    c.arg("-b").arg(bundle_id);
    c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = c.spawn();
}
