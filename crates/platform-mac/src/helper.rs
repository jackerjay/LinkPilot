//! The external Helper and direct distribution share inventory/parsing/launching.
//! Keep the Helper's privilege surface smaller than PlatformProvider/daemon IPC.
use linkpilot_core::browser::{BrowserId, BrowserProfile, BrowserTarget};
use linkpilot_core::platform::{BrowserInventory, UrlLauncher};
use linkpilot_helper_ipc::{Capabilities, Command, Response, PROFILE_BROWSERS};

fn capabilities(inventory: &dyn BrowserInventory) -> Result<Capabilities, String> {
    let browsers = inventory.installed_browsers().map_err(|e| e.to_string())?;
    Ok(Capabilities {
        version: env!("CARGO_PKG_VERSION").into(),
        profile_browsers: browsers
            .into_iter()
            .filter(|b| PROFILE_BROWSERS.contains(&b.id.0.as_str()))
            .map(|b| b.id.0)
            .collect(),
    })
}

fn profiles(
    inventory: &dyn BrowserInventory,
    browser: &BrowserId,
) -> Result<Vec<BrowserProfile>, String> {
    if !PROFILE_BROWSERS.contains(&browser.0.as_str()) {
        return Err("Helper profile routing currently supports Chrome and Brave only".into());
    }
    inventory.profiles(browser).map_err(|error| {
        if matches!(&error, linkpilot_core::platform::PlatformError::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied) {
            format!("LinkPilot Helper cannot read browser profiles: {error}. Allow LinkPilot Helper in System Settings > Privacy & Security > Files & Folders, then retry")
        } else { error.to_string() }
    })
}

pub fn handle(command: Command) -> Result<Response, String> {
    handle_with(
        command,
        &crate::inventory::MacInventory,
        &crate::launcher::MacUrlLauncher,
    )
}

fn handle_with(
    command: Command,
    inventory: &dyn BrowserInventory,
    launcher: &dyn UrlLauncher,
) -> Result<Response, String> {
    match command {
        Command::Status => Ok(Response::Status {
            capabilities: capabilities(inventory)?,
        }),
        Command::Profiles { browser } => Ok(Response::Profiles {
            profiles: profiles(inventory, &browser)?,
        }),
        Command::Open { target, url } => {
            linkpilot_helper_ipc::validate_target(&target)?;
            let url = linkpilot_helper_ipc::validate_url(&url)?;
            // A deleted/stale profile must never silently open the active profile.
            if let Some(profile) = &target.profile {
                if !profiles(inventory, &target.browser)?
                    .iter()
                    .any(|p| &p.id == profile)
                {
                    return Err("Selected profile is unavailable. Refresh browser profiles and choose it again".into());
                }
            }
            launcher.open(&target, &url).map_err(|e| e.to_string())?;
            Ok(Response::Opened)
        }
    }
}

/// Client used only by the sandboxed provider. Do not retry Open: losing a reply
/// after launch is ambiguous, and a retry could open the same link twice.
pub fn open(target: &BrowserTarget, url: &url::Url) -> Result<(), String> {
    match linkpilot_helper_ipc::transport::request(Command::Open {
        target: target.clone(),
        url: url.to_string(),
    })? {
        Response::Opened => Ok(()),
        _ => Err("Unexpected Helper launch response".into()),
    }
}

pub fn list_profiles(browser: &BrowserId) -> Result<Vec<BrowserProfile>, String> {
    if !PROFILE_BROWSERS.contains(&browser.0.as_str()) {
        return Ok(Vec::new());
    }
    match linkpilot_helper_ipc::transport::request(Command::Profiles {
        browser: browser.clone(),
    })? {
        Response::Profiles { profiles } => Ok(profiles),
        _ => Err("Unexpected Helper profiles response".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use linkpilot_core::browser::InstalledBrowser;
    use linkpilot_core::platform::{PlatformError, Result as PlatformResult};
    use std::sync::Mutex;

    struct Inventory {
        denied: bool,
    }
    impl BrowserInventory for Inventory {
        fn installed_browsers(&self) -> PlatformResult<Vec<InstalledBrowser>> {
            Ok(Vec::new())
        }
        fn profiles(&self, _browser: &BrowserId) -> PlatformResult<Vec<BrowserProfile>> {
            if self.denied {
                return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into());
            }
            Ok(vec![BrowserProfile {
                id: "Profile 4".into(),
                display_name: "Test".into(),
                avatar_url: None,
                email: None,
                accent_color: None,
                is_default: false,
            }])
        }
    }
    #[derive(Default)]
    struct Launcher(Mutex<Vec<BrowserTarget>>);
    impl UrlLauncher for Launcher {
        fn open(&self, target: &BrowserTarget, _url: &url::Url) -> PlatformResult<()> {
            self.0.lock().unwrap().push(target.clone());
            Ok(())
        }
    }
    fn command(profile: &str) -> Command {
        Command::Open {
            target: BrowserTarget::new(BrowserId::new("chrome")).with_profile(profile),
            url: "https://example.test/".into(),
        }
    }
    #[test]
    fn missing_or_denied_profiles_never_reach_the_launcher() {
        let launcher = Launcher::default();
        assert!(handle_with(command("deleted"), &Inventory { denied: false }, &launcher).is_err());
        let error =
            handle_with(command("Profile 4"), &Inventory { denied: true }, &launcher).unwrap_err();
        assert!(error.contains("Files & Folders"));
        assert!(launcher.0.lock().unwrap().is_empty());
    }
    #[test]
    fn a_verified_profile_reaches_the_shared_launcher_unchanged() {
        let launcher = Launcher::default();
        assert!(matches!(
            handle_with(
                command("Profile 4"),
                &Inventory { denied: false },
                &launcher
            ),
            Ok(Response::Opened)
        ));
        assert_eq!(
            launcher.0.lock().unwrap()[0].profile.as_deref(),
            Some("Profile 4")
        );
    }
    #[test]
    fn launch_errors_are_returned_to_the_caller() {
        struct Broken;
        impl UrlLauncher for Broken {
            fn open(&self, _target: &BrowserTarget, _url: &url::Url) -> PlatformResult<()> {
                Err(PlatformError::Other("browser failed".into()))
            }
        }
        assert!(
            handle_with(command("Profile 4"), &Inventory { denied: false }, &Broken)
                .unwrap_err()
                .contains("browser failed")
        );
    }
}
