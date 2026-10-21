//! Small C bridge so the production parser runs inside the native app's sandbox.
use std::ffi::{CStr, CString, OsStr};
use std::os::raw::c_char;
use std::path::Path;

use linkpilot_core::inventory::parse_chromium_profiles;
use serde_json::{json, Value};

fn profiles(path: &Path) -> Result<Value, String> {
    if path.file_name() != Some(OsStr::new("Local State")) {
        return Err("Select the browser's Local State file".into());
    }
    let root = path.parent().ok_or("Missing browser data directory")?;
    let parsed = parse_chromium_profiles(root).map_err(|error| error.to_string())?;
    let mut output = Vec::new();
    for profile in parsed {
        if profile.id.is_empty()
            || matches!(profile.id.as_str(), "." | "..")
            || profile.id.contains(['/', '\\'])
            || profile.id.chars().any(char::is_control)
        {
            return Err("Invalid profile directory identifier; scan stopped".into());
        }
        // Account email, avatar and other Local State contents stay out of the bridge.
        output.push(json!({"id": profile.id, "name": profile.display_name}));
    }
    Ok(json!({"profiles": output}))
}

/// Read the selected file using the same parser as the shipped desktop app.
///
/// # Safety
/// A non-null path must point to a valid NUL-terminated UTF-8 string. Free the
/// returned string exactly once using `linkpilot_probe_free`.
#[no_mangle]
pub unsafe extern "C" fn linkpilot_probe_profiles(path: *const c_char) -> *mut c_char {
    let result = if path.is_null() {
        Err("Missing selected file".into())
    } else {
        CStr::from_ptr(path)
            .to_str()
            .map_err(|_| "Selected path is not UTF-8".into())
            .and_then(|path| profiles(Path::new(path)))
    };
    let value = result.unwrap_or_else(|error| json!({"error": error}));
    CString::new(value.to_string())
        .expect("JSON serialization escapes NUL bytes")
        .into_raw()
}

/// Release a result allocated by `linkpilot_probe_profiles`.
///
/// # Safety
/// A non-null pointer must be an unfreed result from `linkpilot_probe_profiles`.
#[no_mangle]
pub unsafe extern "C" fn linkpilot_probe_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_parser_projects_profiles_without_account_data() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&root).unwrap();
        let file = root.join("Local State");
        std::fs::write(&file, r#"{"profile":{"info_cache":{"Default":{"name":"Personal","user_name":"private@example.com","avatar_icon":"private-avatar"},"Profile 2":{"gaia_name":"Work"}}}}"#).unwrap();
        let result = profiles(&file).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            result,
            json!({"profiles":[{"id":"Default","name":"Personal"},{"id":"Profile 2","name":"Work"}]})
        );
    }

    #[test]
    fn unreadable_file_is_an_error_instead_of_empty_profiles() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        assert!(profiles(&root.join("Local State")).is_err());
    }

    #[test]
    fn invalid_profile_identifier_is_reported_instead_of_skipped() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&root).unwrap();
        let file = root.join("Local State");
        std::fs::write(
            &file,
            r#"{"profile":{"info_cache":{"../Other":{"name":"Invalid"}}}}"#,
        )
        .unwrap();
        let result = profiles(&file);
        std::fs::remove_dir_all(&root).unwrap();
        assert!(result.unwrap_err().contains("Invalid profile directory"));
    }

    #[test]
    fn null_ffi_input_returns_a_freeable_error() {
        unsafe {
            let output = linkpilot_probe_profiles(std::ptr::null());
            let value: Value = serde_json::from_slice(CStr::from_ptr(output).to_bytes()).unwrap();
            linkpilot_probe_free(output);
            assert_eq!(value, json!({"error":"Missing selected file"}));
        }
    }
}
