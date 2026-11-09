//! This is deliberately separate from the daemon protocol: no config writes,
//! arbitrary files, scripts, executable paths, or non-web URL schemes.
use linkpilot_core::browser::{BrowserId, BrowserProfile, BrowserTarget};
use serde::{Deserialize, Serialize};

pub const PROTOCOL: u16 = 1;
pub const HELPER_ID: &str = "app.linkpilot.helper";
pub const MAX_FRAME: usize = 64 * 1024;

/// Expand only after proving that the browser honors profile selection.
/// Enumeration alone is not evidence that external profile launch works.
pub const PROFILE_BROWSERS: &[&str] = &["chrome", "brave"];

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol: u16,
    pub command: Command,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Command {
    Status,
    Profiles { browser: BrowserId },
    Open { target: BrowserTarget, url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub version: String,
    pub profile_browsers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Response {
    Status { capabilities: Capabilities },
    Profiles { profiles: Vec<BrowserProfile> },
    Opened,
    Error { message: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Reply {
    pub protocol: u16,
    pub response: Response,
}

pub fn dispatch_request(
    bytes: &[u8],
    handler: impl FnOnce(Command) -> Result<Response, String>,
) -> Reply {
    let response = if bytes.len() > MAX_FRAME {
        Err("Oversized Helper request".into())
    } else {
        serde_json::from_slice::<Request>(bytes)
            .map_err(|e| e.to_string())
            .and_then(|request| {
                if request.protocol != PROTOCOL {
                    return Err(
                        "Helper protocol is incompatible. Update LinkPilot and Helper together"
                            .into(),
                    );
                }
                handler(request.command)
            })
    }
    .unwrap_or_else(|message| Response::Error { message });
    Reply {
        protocol: PROTOCOL,
        response,
    }
}

pub fn validate_target(target: &BrowserTarget) -> Result<(), String> {
    if !PROFILE_BROWSERS.contains(&target.browser.0.as_str()) {
        return Err("Helper profile routing currently supports Chrome and Brave only".into());
    }
    if target.workspace.is_some() {
        return Err("Helper does not support browser workspaces".into());
    }
    if let Some(profile) = &target.profile {
        if profile.is_empty()
            || profile.len() > 128
            || profile.contains(['/', '\\', '\0'])
            || profile == "."
            || profile == ".."
            || profile.starts_with('-')
        {
            return Err("Invalid browser profile identifier".into());
        }
    }
    Ok(())
}

pub fn validate_url(input: &str) -> Result<url::Url, String> {
    if input.len() > 16 * 1024 || input.chars().any(char::is_control) {
        return Err("Invalid or oversized web URL".into());
    }
    let url = url::Url::parse(input).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Helper only opens HTTP and HTTPS links".into());
    }
    Ok(url)
}

#[cfg(target_os = "macos")]
pub mod transport;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incompatible_or_malformed_messages_never_reach_browser_operations() {
        for bytes in [
            br#"{"protocol":999,"command":{"kind":"status"}}"#.to_vec(),
            br#"{"protocol":1,"command":{"kind":"exec"}}"#.to_vec(),
            vec![b' '; MAX_FRAME + 1],
        ] {
            let reply = dispatch_request(&bytes, |_| panic!("must reject before dispatch"));
            assert!(matches!(reply.response, Response::Error { .. }));
        }
    }
    #[test]
    fn rejects_commands_that_would_expand_helper_privileges() {
        for command in [
            r#"{"kind":"exec","path":"/bin/sh"}"#,
            r#"{"kind":"profiles","browser":"chrome","path":"/etc/passwd"}"#,
        ] {
            assert!(serde_json::from_str::<Command>(command).is_err());
        }
    }
    #[test]
    fn web_only_and_bounded() {
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://x.test/\n",
        ] {
            assert!(validate_url(url).is_err());
        }
        assert!(validate_url("https://example.com/?a=--incognito").is_ok());
    }
    #[test]
    fn unsupported_profiles_never_fall_back_to_an_active_profile() {
        let mut target = BrowserTarget::new(BrowserId::new("safari")).with_profile("Default");
        assert!(validate_target(&target).is_err());
        target.browser = BrowserId::new("chrome");
        for profile in ["../Default", "--user-data-dir=bad", "", ".", "a\\b"] {
            target.profile = Some(profile.into());
            assert!(validate_target(&target).is_err());
        }
        target.profile = Some("Profile 1".into());
        assert!(validate_target(&target).is_ok());
    }
}
