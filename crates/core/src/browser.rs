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
