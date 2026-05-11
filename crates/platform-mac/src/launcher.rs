use linkpilot_core::browser::BrowserTarget;
use linkpilot_core::platform::{PlatformError, Result, UrlLauncher};
use url::Url;

pub struct MacUrlLauncher;

impl UrlLauncher for MacUrlLauncher {
    fn open(&self, _target: &BrowserTarget, _url: &Url) -> Result<()> {
        Err(PlatformError::Other(
            "MacUrlLauncher::open unimplemented (step 5)".into(),
        ))
    }
}
