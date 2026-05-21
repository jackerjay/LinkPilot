use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Stable identifier for a browser product (e.g. `"chrome"`, `"arc"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BrowserId(pub String);

impl BrowserId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for BrowserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Engine family — drives shared parsing logic for profiles, args, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BrowserKind {
    Chromium,
    Firefox,
    Safari,
    Arc,
    Unknown,
}

/// A browser detected on the user's machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledBrowser {
    pub id: BrowserId,
    pub display_name: String,
    pub kind: BrowserKind,
    pub executable: PathBuf,
    /// macOS bundle id, Windows AppUserModelID, Linux .desktop name.
    pub platform_app_id: Option<String>,
    /// Root of the user-data directory; used to enumerate profiles.
    pub profile_root: Option<PathBuf>,
}

/// A profile within an installed browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProfile {
    /// Browser-native id (e.g. Chrome's `"Profile 1"`).
    pub id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
    /// Stable hex accent color (`#RRGGBB`) derived deterministically from
    /// the profile id. Used by the picker wheel and the inspector — never
    /// trusted for security purposes. None when the inventory parser
    /// can't infer one (Safari, Unknown).
    #[serde(default)]
    pub accent_color: Option<String>,
    /// True for Chromium's literal `Default` profile, Firefox's
    /// `IsDefault=1`, etc. The picker uses this to seed the Crown
    /// idle-state preview ("press Enter to open this one").
    #[serde(default)]
    pub is_default: bool,
}

/// Routing target as expressed by user configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserTarget {
    pub browser: BrowserId,
    pub profile: Option<String>,
    pub workspace: Option<String>,
    #[serde(default)]
    pub incognito: bool,
    #[serde(default)]
    pub new_window: bool,
}

impl BrowserTarget {
    pub fn new(browser: BrowserId) -> Self {
        Self {
            browser,
            profile: None,
            workspace: None,
            incognito: false,
            new_window: false,
        }
    }

    pub fn with_profile(mut self, profile: impl Into<String>) -> Self {
        self.profile = Some(profile.into());
        self
    }
}

/// Reorder a list of profiles according to a user-saved visible order.
/// Profiles whose id appears in `order` render in that order. Anything
/// not in a non-empty saved order is hidden from the Halo wheel until
/// the user adds it back in Settings.
///
/// This means:
///   * A browser with no saved order falls through to the legacy default
///     ordering, so behavior is unchanged for users who never touched
///     the editor.
///   * Stale ids in `order` (profile was deleted from the browser) are
///     simply skipped — no allocation, no error path.
pub fn apply_profile_order(
    profiles: Vec<BrowserProfile>,
    order: Option<&[String]>,
) -> Vec<BrowserProfile> {
    let order = match order {
        Some(o) if !o.is_empty() => o,
        _ => {
            // No saved order — apply the default sort and return.
            let mut p = profiles;
            p.sort_by(|a, b| {
                b.is_default
                    .cmp(&a.is_default)
                    .then_with(|| a.display_name.cmp(&b.display_name))
            });
            return p;
        }
    };

    // O(N+M) two-pass: index profiles by id, then walk the saved visible
    // list, pulling matching entries out. Leftovers are intentionally not
    // appended: omission is how Settings hides a profile from Halo.
    let mut by_id: std::collections::HashMap<String, BrowserProfile> =
        profiles.into_iter().map(|p| (p.id.clone(), p)).collect();
    let mut out: Vec<BrowserProfile> = Vec::with_capacity(by_id.len());
    for id in order {
        if let Some(p) = by_id.remove(id) {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: &str, name: &str, is_default: bool) -> BrowserProfile {
        BrowserProfile {
            id: id.into(),
            display_name: name.into(),
            avatar_url: None,
            email: None,
            accent_color: None,
            is_default,
        }
    }

    #[test]
    fn no_order_falls_back_to_default_first_then_alpha() {
        let input = vec![
            p("Profile 2", "Work", false),
            p("Default", "Personal", true),
            p("Profile 1", "Side", false),
        ];
        let out = apply_profile_order(input, None);
        assert_eq!(out[0].id, "Default"); // default wins
        assert_eq!(out[1].id, "Profile 1"); // "Side" < "Work" alphabetically
        assert_eq!(out[2].id, "Profile 2");
    }

    #[test]
    fn saved_order_wins_for_listed_profiles() {
        let input = vec![
            p("Default", "Personal", true),
            p("Profile 1", "Side", false),
            p("Profile 2", "Work", false),
        ];
        let order = vec!["Profile 2".into(), "Default".into(), "Profile 1".into()];
        let out = apply_profile_order(input, Some(&order));
        assert_eq!(
            out.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            vec!["Profile 2", "Default", "Profile 1"]
        );
    }

    #[test]
    fn saved_order_hides_unlisted_profiles() {
        // The saved list is the complete visible Halo inventory. Profiles
        // not listed here stay hidden until the user adds them back.
        let input = vec![
            p("Default", "Personal", true),
            p("Profile 1", "Side", false),
            p("Profile 2", "Work", false),
            p("Profile 3", "AI", false),
        ];
        let order = vec!["Profile 2".into(), "Default".into()];
        let out = apply_profile_order(input, Some(&order));
        let ids: Vec<&str> = out.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["Profile 2", "Default"]);
    }

    #[test]
    fn stale_ids_in_order_are_skipped() {
        // User deleted "Profile 2" in their browser. The saved order
        // still references it. We just skip and keep going.
        let input = vec![
            p("Default", "Personal", true),
            p("Profile 1", "Side", false),
        ];
        let order = vec!["Profile 2".into(), "Default".into(), "Profile 1".into()];
        let out = apply_profile_order(input, Some(&order));
        assert_eq!(
            out.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            vec!["Default", "Profile 1"]
        );
    }
}
