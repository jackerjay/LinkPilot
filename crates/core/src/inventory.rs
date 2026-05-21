//! Browser profile parsing — shared across platforms.
//!
//! Platform crates only have to point at the user-data root; the JSON / INI
//! parsing logic lives here so the same Chromium "Local State" reader runs
//! on macOS, Windows, and Linux.

use std::path::Path;

use serde::Deserialize;

use crate::browser::BrowserProfile;

/// 12-color palette used to assign each profile a stable accent. Picked
/// to match Google's own profile-picker palette where possible (Chrome's
/// avatar fill colors come from a similar set) so a freshly-imported
/// Chrome profile feels familiar in the LinkPilot picker.
///
/// The wheel uses these to paint outer-rim color bands, hover wedges,
/// and the Crown variant's center-display avatar. None of these
/// colors are user-overridable — if we ever want that, route it
/// through a separate `Settings.profile_accent_overrides` map keyed
/// by profile id.
const ACCENT_PALETTE: &[&str] = &[
    "#4285F4", // blue
    "#34A853", // green
    "#EA4335", // red
    "#FBBC04", // yellow
    "#9C27B0", // purple
    "#FF6D00", // orange
    "#00ACC1", // teal
    "#43A047", // emerald
    "#5E35B1", // indigo
    "#6E6E73", // graphite
    "#E91E63", // pink
    "#3F51B5", // navy
];

/// Deterministic hash → palette index. We use the FNV-1a 32-bit variant
/// because Rust's `DefaultHasher` is unstable across releases and we
/// want the same profile to keep the same color even after a Rust
/// toolchain upgrade. The cost is one extra small function; FNV is in
/// no way load-bearing for correctness.
fn accent_for(id: &str) -> String {
    let mut hash: u32 = 0x811c9dc5;
    for b in id.as_bytes() {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    ACCENT_PALETTE[(hash as usize) % ACCENT_PALETTE.len()].to_string()
}

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
        .map(|(id, entry)| {
            let accent_color = Some(accent_for(&id));
            let is_default = id == "Default";
            BrowserProfile {
                display_name: entry
                    .name
                    .or(entry.gaia_name.clone())
                    .unwrap_or_else(|| "Profile".to_string()),
                avatar_url: entry.avatar_icon,
                email: entry.user_name.or(entry.gaia_name),
                accent_color,
                is_default,
                id,
            }
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
    let mut current_default = false;

    let flush = |out: &mut Vec<BrowserProfile>,
                 name: Option<String>,
                 path: Option<String>,
                 default: bool| {
        if let (Some(name), Some(path)) = (name, path) {
            let accent_color = Some(accent_for(&path));
            out.push(BrowserProfile {
                display_name: name,
                avatar_url: None,
                email: None,
                accent_color,
                is_default: default,
                id: path,
            });
        }
    };

    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            flush(
                &mut out,
                current_name.take(),
                current_path.take(),
                current_default,
            );
            current_default = false;
            continue;
        }
        if let Some(rest) = line.strip_prefix("Name=") {
            current_name = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("Path=") {
            current_path = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("Default=") {
            current_default = rest.trim() == "1";
        }
    }
    flush(&mut out, current_name, current_path, current_default);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_for_is_stable() {
        // FNV-1a is deterministic — same id → same color forever. The
        // picker depends on this so a user's Work profile doesn't
        // change color between launches.
        let first = accent_for("Default");
        let second = accent_for("Default");
        assert_eq!(first, second);
        assert!(ACCENT_PALETTE.contains(&first.as_str()));
    }

    #[test]
    fn accent_for_varies_by_id() {
        // Different profile dirs should usually get different colors.
        // We don't assert uniqueness (12-color palette + 12 profiles
        // would hit by pigeonhole), but at least one pair must differ.
        let ids = ["Default", "Profile 1", "Profile 2", "Profile 3"];
        let colors: Vec<_> = ids.iter().map(|id| accent_for(id)).collect();
        let distinct: std::collections::BTreeSet<_> = colors.iter().collect();
        assert!(
            distinct.len() >= 2,
            "expected at least 2 distinct colors, got {colors:?}"
        );
    }
}
