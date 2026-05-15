//! Configuration document — the on-disk JSON single source of truth.
//!
//! [`ConfigDocument`] defines the schema; [`store`] handles load/save and
//! path resolution. The fsnotify pipeline and anti-echo token enforcement
//! land in a later slice alongside the IPC server.

pub mod store;

pub use store::{default_config_path, ConfigStore};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::browser::{BrowserId, BrowserTarget, InstalledBrowser};
use crate::rules::Rule;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDocument {
    #[serde(default = "default_schema_version")]
    pub version: u32,
    pub default_target: BrowserTarget,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
    /// User-added browsers — apps the inventory didn't auto-detect
    /// (niche browsers, dev builds, sideloaded .app bundles, etc.).
    /// Merged with the inventory at `list_browsers` time; entries with
    /// the same `id` as an auto-detected browser override the auto
    /// entry so the user can correct stale display names / exec paths.
    #[serde(default)]
    pub custom_browsers: Vec<InstalledBrowser>,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub meta: Meta,
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

impl ConfigDocument {
    /// Empty document with the given fallback target.
    pub fn with_default(target: BrowserTarget) -> Self {
        Self {
            version: SCHEMA_VERSION,
            default_target: target,
            rules: Vec::new(),
            workspaces: Vec::new(),
            custom_browsers: Vec::new(),
            settings: Settings::default(),
            meta: Meta::default(),
        }
    }

    /// Demo config matching PRD §22. Used when no config exists on disk.
    pub fn demo() -> Self {
        use crate::rules::{Action, MatcherTree, RuleId, RuleSource};

        let chrome_work = BrowserTarget::new(BrowserId::new("chrome")).with_profile("Default");
        let arc = BrowserTarget::new(BrowserId::new("arc"));

        let mk = |host: &str, target: BrowserTarget, prio: i32| Rule {
            id: RuleId::default(),
            priority: prio,
            enabled: true,
            when: MatcherTree::UrlHost {
                pattern: host.to_string(),
            },
            then: Action::Open { target },
            source: RuleSource::Gui,
            note: None,
            workspace_id: None,
        };

        Self {
            version: SCHEMA_VERSION,
            default_target: arc.clone(),
            rules: vec![
                mk("github.com", chrome_work.clone(), 10),
                mk("notion.so", chrome_work.clone(), 10),
                mk("figma.com", arc.clone(), 10),
                mk("youtube.com", arc, 10),
            ],
            workspaces: Vec::new(),
            custom_browsers: Vec::new(),
            settings: Settings::default(),
            meta: Meta::default(),
        }
    }
}

impl Default for ConfigDocument {
    fn default() -> Self {
        Self::with_default(BrowserTarget::new(BrowserId::new("system")))
    }
}

/// A named group that aggregates multiple [`Rule`]s. Toggling
/// `enabled = false` deactivates every rule whose `workspace_id`
/// matches `id` without losing the per-rule `enabled` flag — flip the
/// workspace back on and the rules light up exactly as they were.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_workspace_enabled")]
    pub enabled: bool,
}

fn default_workspace_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub launch_at_login: bool,
    #[serde(default)]
    pub history_retention_days: Option<u32>,
    #[serde(default)]
    pub record_query_strings: bool,
    /// Master kill-switch for rule evaluation. When false the router
    /// skips every rule (and every workspace) and opens links straight
    /// in `default_target` — the tray popover's "Smart routing" toggle
    /// flips this. Defaults to true so a fresh install behaves like
    /// LinkPilot always has.
    #[serde(default = "default_smart_routing")]
    pub smart_routing_enabled: bool,
}

fn default_smart_routing() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            history_retention_days: None,
            record_query_strings: false,
            smart_routing_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    /// Token used to suppress self-induced fsnotify echoes. Rewritten on every
    /// daemon-side save; if a reread observes a different token, the change
    /// came from an external editor and must be broadcast to the GUI.
    #[serde(default)]
    pub last_writer_token: Option<Uuid>,
    #[serde(default)]
    pub last_writer: Option<WriterId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WriterId {
    Gui,
    File,
    Cli,
    TsCompiled,
}
