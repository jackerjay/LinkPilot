//! Public AppKit APIs for the sandboxed, browser-only Store edition.
//! No subprocesses, browser data access, or LaunchAgent writes.
use std::ffi::CStr;
use std::path::{Path, PathBuf};

use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation_sys::base::{CFGetTypeID, CFRelease, CFTypeRef};
use core_foundation_sys::number::{CFBooleanGetTypeID, CFBooleanGetValue, CFBooleanRef};

use linkpilot_core::browser::{BrowserId, BrowserProfile, BrowserTarget, InstalledBrowser};
use linkpilot_core::platform::*;
use objc2::rc::{autoreleasepool, Allocated, Retained};
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send, msg_send_id};
use url::Url;

#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecTaskCreateFromSelf(allocator: CFTypeRef) -> CFTypeRef;
    fn SecTaskCopyValueForEntitlement(
        task: CFTypeRef,
        entitlement: CFStringRef,
        error: *mut CFTypeRef,
    ) -> CFTypeRef;
}

/// A bare feature-enabled executable must not touch the direct edition's data.
/// Reject it before creating WebKit windows or resolving configuration paths.
pub fn require_sandbox() -> Result<()> {
    let key = CFString::new("com.apple.security.app-sandbox");
    let enabled = unsafe {
        let task = SecTaskCreateFromSelf(std::ptr::null());
        if task.is_null() {
            return Err(PlatformError::Other(
                "Cannot inspect App Sandbox entitlement".into(),
            ));
        }
        let value =
            SecTaskCopyValueForEntitlement(task, key.as_concrete_TypeRef(), std::ptr::null_mut());
        let enabled = !value.is_null()
            && CFGetTypeID(value) == CFBooleanGetTypeID()
            && CFBooleanGetValue(value as CFBooleanRef);
        if !value.is_null() {
            CFRelease(value);
        }
        CFRelease(task);
        enabled
    };
    if enabled {
        Ok(())
    } else {
        Err(PlatformError::Other(
            "Run the App Store edition from a signed App Sandbox bundle built with bundle:store"
                .into(),
        ))
    }
}

pub struct StoreProvider {
    default_browser: crate::default_browser::MacDefaultBrowser,
    opener: crate::opener::MacOpenerDetector,
}

impl StoreProvider {
    pub fn new(bundle_id: String) -> Self {
        Self {
            default_browser: crate::default_browser::MacDefaultBrowser::new(bundle_id.clone()),
            opener: crate::opener::MacOpenerDetector::start(bundle_id),
        }
    }
}

impl PlatformProvider for StoreProvider {
    fn default_browser(&self) -> &dyn DefaultBrowserController {
        &self.default_browser
    }
    fn browser_inventory(&self) -> &dyn BrowserInventory {
        self
    }
    fn url_launcher(&self) -> &dyn UrlLauncher {
        self
    }
    fn autostart(&self) -> &dyn Autostart {
        self
    }
    fn notifier(&self) -> &dyn Notifier {
        &crate::notifier::MacNotifier
    }
    fn opener_detector(&self) -> &dyn OpenerDetector {
        &self.opener
    }
}

impl BrowserInventory for StoreProvider {
    fn installed_browsers(&self) -> Result<Vec<InstalledBrowser>> {
        Ok(crate::inventory::store_browsers())
    }
    fn profiles(&self, _browser: &BrowserId) -> Result<Vec<BrowserProfile>> {
        // This edition routes by browser; it does not request profile permissions.
        Ok(Vec::new())
    }
}

pub fn validate_target(target: &BrowserTarget) -> Result<()> {
    if target.profile.is_some()
        || target.incognito
        || target.new_window
        || target.workspace.is_some()
    {
        return Err(PlatformError::Other(
            "The App Store edition supports browser targets without a profile, private mode, workspace or new-window flag".into(),
        ));
    }
    Ok(())
}

impl UrlLauncher for StoreProvider {
    fn open(&self, target: &BrowserTarget, url: &Url) -> Result<()> {
        validate_target(target)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(PlatformError::Other(
                "Only HTTP and HTTPS links can be routed".into(),
            ));
        }
        let browsers = self.installed_browsers()?;
        let bundle_id = browsers
            .iter()
            .find(|b| b.id == target.browser)
            .and_then(|b| b.platform_app_id.as_deref())
            .unwrap_or(&target.browser.0);
        if bundle_id.starts_with("app.linkpilot.") || application_path(bundle_id).is_none() {
            return Err(PlatformError::Other(
                "Browser is unavailable or would route back to LinkPilot".into(),
            ));
        }
        autoreleasepool(|_| unsafe {
            let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
            let url_string = nsstring(url.as_str());
            let native_url: *mut AnyObject = msg_send![class!(NSURL), URLWithString: &*url_string];
            if native_url.is_null() {
                return Err(PlatformError::Other("macOS rejected the URL".into()));
            }
            let urls: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: native_url];
            let identifier = nsstring(bundle_id);
            // The synchronous public URL API fits UrlLauncher's Result contract.
            // No argv is passed: macOS ignores it for sandboxed callers.
            let opened: Bool = msg_send![workspace, openURLs: urls
                withAppBundleIdentifier: &*identifier options: 0usize
                additionalEventParamDescriptor: std::ptr::null::<AnyObject>()
                launchIdentifiers: std::ptr::null_mut::<*mut AnyObject>()];
            if opened.as_bool() {
                Ok(())
            } else {
                Err(PlatformError::Other(
                    "macOS refused to open the URL in the selected browser".into(),
                ))
            }
        })
    }
}

impl Autostart for StoreProvider {
    fn is_enabled(&self) -> Result<bool> {
        Ok(false)
    }
    fn set_enabled(&self, _on: bool) -> Result<()> {
        Err(PlatformError::NotSupported)
    }
}

fn nsstring(value: &str) -> Retained<AnyObject> {
    unsafe {
        let string: Allocated<AnyObject> = msg_send_id![class!(NSString), alloc];
        msg_send_id![string, initWithBytes: value.as_ptr().cast::<std::ffi::c_void>() length: value.len() encoding: 4usize]
    }
}

unsafe fn string(value: *mut AnyObject) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let bytes: *const std::ffi::c_char = msg_send![value, UTF8String];
    if bytes.is_null() {
        None
    } else {
        Some(CStr::from_ptr(bytes).to_string_lossy().into_owned())
    }
}

pub fn application_path(bundle_id: &str) -> Option<PathBuf> {
    autoreleasepool(|_| unsafe {
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        let identifier = nsstring(bundle_id);
        let app: *mut AnyObject =
            msg_send![workspace, URLForApplicationWithBundleIdentifier: &*identifier];
        if app.is_null() {
            return None;
        }
        let path: *mut AnyObject = msg_send![app, path];
        string(path).map(PathBuf::from)
    })
}

pub fn application_support_directory() -> Result<PathBuf> {
    autoreleasepool(|_| unsafe {
        let manager: *mut AnyObject = msg_send![class!(NSFileManager), defaultManager];
        let urls: *mut AnyObject = msg_send![manager, URLsForDirectory: 14usize inDomains: 1usize];
        let url: *mut AnyObject = msg_send![urls, firstObject];
        if url.is_null() {
            return Err(PlatformError::Other(
                "Application Support is unavailable".into(),
            ));
        }
        let path: *mut AnyObject = msg_send![url, path];
        string(path)
            .map(PathBuf::from)
            .ok_or_else(|| PlatformError::Other("Invalid Application Support path".into()))
    })
}

/// Called by a synchronous Tauri command on the UI thread.
pub fn choose_app() -> std::result::Result<Option<crate::app_picker::PickedApp>, String> {
    autoreleasepool(|_| unsafe {
        let panel: *mut AnyObject = msg_send![class!(NSOpenPanel), openPanel];
        let types: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: &*nsstring("app")];
        let _: () = msg_send![panel, setAllowedFileTypes: types];
        let _: () = msg_send![panel, setAllowsMultipleSelection: Bool::NO];
        let _: () = msg_send![panel, setCanChooseDirectories: Bool::NO];
        let result: isize = msg_send![panel, runModal];
        if result != 1 {
            return Ok(None);
        }
        let url: *mut AnyObject = msg_send![panel, URL];
        if url.is_null() {
            return Err("No application was selected".into());
        }
        let path: *mut AnyObject = msg_send![url, path];
        let path = string(path).ok_or("Invalid application path")?;
        let bundle: *mut AnyObject = msg_send![class!(NSBundle), bundleWithPath: &*nsstring(&path)];
        if bundle.is_null() {
            return Err("Select an application bundle".into());
        }
        let id: *mut AnyObject = msg_send![bundle, bundleIdentifier];
        let bundle_id = string(id)
            .filter(|v| !v.is_empty())
            .ok_or("Application has no bundle identifier")?;
        let name: *mut AnyObject =
            msg_send![bundle, objectForInfoDictionaryKey: &*nsstring("CFBundleName")];
        Ok(Some(crate::app_picker::PickedApp {
            name: string(name).unwrap_or_else(|| bundle_id.clone()),
            bundle_id,
            app_path: path,
        }))
    })
}

pub fn icon_png(bundle_id: Option<&str>, path: Option<&Path>, size: u32) -> Result<PathBuf> {
    let path = path
        .map(PathBuf::from)
        .or_else(|| bundle_id.and_then(application_path))
        .ok_or_else(|| PlatformError::Other("Application icon is unavailable".into()))?;
    let directory = application_support_directory()?.join("LinkPilot/icons");
    std::fs::create_dir_all(&directory)?;
    use std::hash::{Hash, Hasher};
    let mut key = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut key);
    let output = directory.join(format!("{:x}-{size}.png", key.finish()));
    if output.is_file() {
        return Ok(output);
    }
    autoreleasepool(|_| unsafe {
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        let image: *mut AnyObject =
            msg_send![workspace, iconForFile: &*nsstring(&path.to_string_lossy())];
        let tiff: *mut AnyObject = msg_send![image, TIFFRepresentation];
        if tiff.is_null() {
            return Err(PlatformError::Other(
                "Application icon has no image data".into(),
            ));
        }
        let bitmap: *mut AnyObject = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
        if bitmap.is_null() {
            return Err(PlatformError::Other(
                "Application icon is not a bitmap".into(),
            ));
        }
        let properties: *mut AnyObject = msg_send![class!(NSDictionary), dictionary];
        let png: *mut AnyObject =
            msg_send![bitmap, representationUsingType: 4usize properties: properties];
        if png.is_null() {
            return Err(PlatformError::Other(
                "Application icon conversion failed".into(),
            ));
        }
        let written: Bool = msg_send![png, writeToFile: &*nsstring(&output.to_string_lossy()) atomically: Bool::YES];
        if written.as_bool() {
            Ok(output)
        } else {
            Err(PlatformError::Other("Cannot save application icon".into()))
        }
    })
}

/// The native chooser owns authorization; no caller-supplied path is accepted.
/// The selected URL's scope is active only for the read/write operation.
fn with_json_file(
    save: bool,
    operation: impl FnOnce(&Path) -> std::result::Result<String, String>,
) -> std::result::Result<Option<String>, String> {
    autoreleasepool(|_| unsafe {
        let panel: *mut AnyObject = if save {
            msg_send![class!(NSSavePanel), savePanel]
        } else {
            msg_send![class!(NSOpenPanel), openPanel]
        };
        let types: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: &*nsstring("json")];
        let _: () = msg_send![panel, setAllowedFileTypes: types];
        if save {
            let _: () =
                msg_send![panel, setNameFieldStringValue: &*nsstring("linkpilot.config.json")];
        } else {
            let _: () = msg_send![panel, setAllowsMultipleSelection: Bool::NO];
            let _: () = msg_send![panel, setCanChooseDirectories: Bool::NO];
        }
        let response: isize = msg_send![panel, runModal];
        if response != 1 {
            return Ok(None);
        }
        let url: *mut AnyObject = msg_send![panel, URL];
        if url.is_null() {
            return Err("No configuration file was selected".into());
        }
        let path: *mut AnyObject = msg_send![url, path];
        let path = string(path).ok_or("Invalid configuration path")?;
        let active: Bool = msg_send![url, startAccessingSecurityScopedResource];
        let result = operation(Path::new(&path));
        if active.as_bool() {
            let _: () = msg_send![url, stopAccessingSecurityScopedResource];
        }
        result.map(Some)
    })
}

pub fn read_config_file() -> std::result::Result<Option<String>, String> {
    with_json_file(false, |path| {
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    })
}

pub fn write_config_file(contents: &str) -> std::result::Result<Option<String>, String> {
    with_json_file(true, |path| unsafe {
        let data: *mut AnyObject = msg_send![class!(NSData), dataWithBytes: contents.as_ptr().cast::<std::ffi::c_void>() length: contents.len()];
        let written: Bool =
            msg_send![data, writeToFile: &*nsstring(&path.to_string_lossy()) atomically: Bool::YES];
        if written.as_bool() {
            Ok(path.display().to_string())
        } else {
            Err("Cannot atomically export configuration".into())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bare_test_process_cannot_claim_to_be_sandboxed() {
        assert!(require_sandbox().is_err());
    }
    #[test]
    fn native_strings_preserve_unicode_and_spaces() {
        autoreleasepool(|_| unsafe {
            let native = nsstring("浏览器 Profile 2");
            assert_eq!(
                string(Retained::as_ptr(&native) as *mut AnyObject).as_deref(),
                Some("浏览器 Profile 2")
            );
        });
    }
    #[test]
    fn browser_only_targets_reject_every_unsupported_hint() {
        let plain = BrowserTarget::new(BrowserId::new("chrome"));
        assert!(validate_target(&plain).is_ok());
        for value in ["profile", "incognito", "new-window", "workspace"] {
            let mut target = plain.clone();
            match value {
                "profile" => target.profile = Some("Default".into()),
                "incognito" => target.incognito = true,
                "new-window" => target.new_window = true,
                _ => target.workspace = Some("work".into()),
            }
            assert!(validate_target(&target).is_err(), "{value}");
        }
    }
}
