use linkpilot_core::browser::BrowserId;
use linkpilot_core::platform::{
    DefaultBrowserController, PlatformError, Result, SetDefaultOutcome,
};

/// Wraps `LSSetDefaultHandlerForURLScheme` for http / https.
///
/// v0.1 step 1 is a scaffold; real `objc2-application-services` calls land
/// when wiring the Settings → "Set as default browser" button.
pub struct MacDefaultBrowser;

impl DefaultBrowserController for MacDefaultBrowser {
    fn current_default(&self) -> Result<Option<BrowserId>> {
        Err(PlatformError::Other(
            "MacDefaultBrowser::current_default unimplemented".into(),
        ))
    }

    fn is_linkpilot_default(&self) -> Result<bool> {
        Ok(false)
    }

    fn request_set_default(&self) -> Result<SetDefaultOutcome> {
        Ok(SetDefaultOutcome::NotSupported)
    }
}
