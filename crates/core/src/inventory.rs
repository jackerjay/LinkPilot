//! Browser profile parsing — shared across platforms.
//!
//! Platform crates only have to point at the user-data root; the JSON / INI
//! parsing logic lives here so the same Chromium "Local State" reader runs
//! on macOS, Windows, and Linux.

use std::path::Path;

use serde::Deserialize;

use crate::browser::BrowserProfile;

#[derive(Debug, Deserialize)]
struct ChromiumLocalState {
    profile: ChromiumProfileSection,
}

#[derive(Debug, Deserialize)]
struct ChromiumProfileSection {
    info_cache: std::collections::BTreeMap<String, ChromiumProfileEntry>,
}

#[derive(Debug, Deserialize)]
struct ChromiumProfileEntry {
    name: Option<String>,
    gaia_name: Option<String>,
    user_name: Option<String>,
    #[serde(default)]
    avatar_icon: Option<String>,
}

/// Parse Chromium-family `Local State` (Chrome / Edge / Brave / Arc).
pub fn parse_chromium_profiles(user_data_root: &Path) -> std::io::Result<Vec<BrowserProfile>> {
    let path = user_data_root.join("Local State");
    let raw = std::fs::read_to_string(&path)?;
    let parsed: ChromiumLocalState = serde_json::from_str(&raw)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    Ok(parsed
        .profile
        .info_cache
        .into_iter()
        .map(|(id, entry)| BrowserProfile {
            id,
            display_name: entry
                .name
                .or(entry.gaia_name.clone())
                .unwrap_or_else(|| "Profile".to_string()),
            avatar_url: entry.avatar_icon,
            email: entry.user_name.or(entry.gaia_name),
        })
        .collect())
}

/// Parse Firefox `profiles.ini`. Returns empty if the file is malformed —
/// callers should treat that as "no profiles detected".
pub fn parse_firefox_profiles(profiles_ini: &Path) -> std::io::Result<Vec<BrowserProfile>> {
    let raw = std::fs::read_to_string(profiles_ini)?;
    let mut out = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_path: Option<String> = None;

    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            if let (Some(name), Some(path)) = (current_name.take(), current_path.take()) {
                out.push(BrowserProfile {
                    id: path,
                    display_name: name,
                    avatar_url: None,
                    email: None,
                });
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("Name=") {
            current_name = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("Path=") {
            current_path = Some(rest.to_string());
        }
    }
    if let (Some(name), Some(path)) = (current_name, current_path) {
        out.push(BrowserProfile {
            id: path,
            display_name: name,
            avatar_url: None,
            email: None,
        });
    }
    Ok(out)
}
