//! `LSSetDefaultHandlerForURLScheme` for http / https.
//!
//! `LSSetDefaultHandlerForURLScheme` is technically deprecated since macOS 12
//! (NSWorkspace gained an async replacement), but it still works on macOS 14 /
//! 15 and avoids pulling in AppKit + completion handlers. The system shows a
//! confirmation dialog before honouring the change.

use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use linkpilot_core::browser::BrowserId;
use linkpilot_core::platform::{
    DefaultBrowserController, PlatformError, Result, SetDefaultOutcome,
};

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn LSSetDefaultHandlerForURLScheme(
        in_url_scheme: CFStringRef,
        in_handler_bundle_id: CFStringRef,
    ) -> i32;
    fn LSCopyDefaultHandlerForURLScheme(in_url_scheme: CFStringRef) -> CFStringRef;
}

pub struct MacDefaultBrowser {
    bundle_id: String,
}

impl MacDefaultBrowser {
    pub fn new(bundle_id: String) -> Self {
        Self { bundle_id }
    }

    fn copy_handler(scheme: &str) -> Option<String> {
        let scheme_cf = CFString::new(scheme);
        unsafe {
            let raw = LSCopyDefaultHandlerForURLScheme(scheme_cf.as_concrete_TypeRef());
            if raw.is_null() {
                None
            } else {
                let s = CFString::wrap_under_create_rule(raw).to_string();
                Some(s)
            }
        }
    }
}

impl DefaultBrowserController for MacDefaultBrowser {
    fn current_default(&self) -> Result<Option<BrowserId>> {
        // We return the http handler's bundle id (Safari is `com.apple.Safari`,
        // Chrome is `com.google.Chrome`, etc.). Mapping back to our
        // BrowserRegistry is the caller's job.
        Ok(Self::copy_handler("http").map(BrowserId::new))
    }

    fn is_linkpilot_default(&self) -> Result<bool> {
        let http = Self::copy_handler("http");
        let https = Self::copy_handler("https");
        Ok(matches!((&http, &https),
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(&self.bundle_id) && b.eq_ignore_ascii_case(&self.bundle_id)))
    }

    fn request_set_default(&self) -> Result<SetDefaultOutcome> {
        let bundle_cf = CFString::new(&self.bundle_id);
        for scheme in ["http", "https"] {
            let scheme_cf = CFString::new(scheme);
            let status = unsafe {
                LSSetDefaultHandlerForURLScheme(
                    scheme_cf.as_concrete_TypeRef(),
                    bundle_cf.as_concrete_TypeRef(),
                )
            };
            if status != 0 {
                return Err(PlatformError::Other(format!(
                    "LSSetDefaultHandlerForURLScheme({scheme}) returned OSStatus {status}; \
                    is the app installed in /Applications and registered with LaunchServices?"
                )));
            }
        }
        Ok(SetDefaultOutcome::Done)
    }
}
