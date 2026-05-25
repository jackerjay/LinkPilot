//! Configuration document — the on-disk JSON single source of truth.
//!
//! [`ConfigDocument`] defines the schema; [`store`] handles load/save and
//! path resolution. The fsnotify pipeline and anti-echo token enforcement
//! land in a later slice alongside the IPC server.

pub mod store;

pub use store::{default_config_path, ConfigStore};

use std::collections::BTreeMap;

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
    /// Rules are listed top-to-bottom in priority order — first match wins.
    pub fn demo() -> Self {
        use crate::rules::{Action, MatcherTree, RuleId, RuleSource};

        let chrome_work = BrowserTarget::new(BrowserId::new("chrome")).with_profile("Default");
        let arc = BrowserTarget::new(BrowserId::new("arc"));

        let mk = |host: &str, target: BrowserTarget| Rule {
            id: RuleId::default(),
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
            default_target: BrowserTarget::new(BrowserId::new("system")),
            rules: vec![
                mk("github.com", chrome_work.clone()),
                mk("notion.so", chrome_work.clone()),
                mk("figma.com", arc.clone()),
                mk("youtube.com", arc),
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
    /// Check GitHub Releases for newer LinkPilot builds when the GUI app
    /// starts. The app only reports availability; installation remains a
    /// user-driven download flow while releases are unsigned.
    #[serde(default = "default_auto_check_updates")]
    pub auto_check_updates: bool,
    /// Master kill-switch for rule evaluation. When false the router
    /// skips every rule (and every workspace) and opens links straight
    /// in `default_target` — the tray popover's "Smart routing" toggle
    /// flips this. Defaults to true so a fresh install behaves like
    /// LinkPilot always has.
    #[serde(default = "default_smart_routing")]
    pub smart_routing_enabled: bool,
    /// Visual style for the browser+profile picker that appears for
    /// `ask` routes with multi-profile browsers. The picker window
    /// reads this once on open and renders the matching Halo variant.
    /// Defaults to Frosted because the design lead picked it as the
    /// most macOS-native ("baseline") — Bezel and Crown are opt-in.
    #[serde(default)]
    pub picker_style: PickerStyle,
    /// User-customized visible profile order, per browser id. The picker
    /// uses this to place profiles in specific wheel slots — important
    /// because keyboard 1–9 shortcuts follow position order.
    ///
    /// Semantics: an empty/missing list means default sort (`is_default`
    /// first, then alphabetical). A non-empty list is the complete visible
    /// Halo inventory for that browser; profiles missing from it are hidden
    /// until the user adds them back in Settings. Stale ids are skipped.
    #[serde(default)]
    pub profile_orders: BTreeMap<String, Vec<String>>,
    /// Browser ids the user has hidden from the ask-popup picker (and the
    /// Settings → preview picker, for consistency). Disabled browsers stay
    /// on disk and remain valid as explicit routing targets — a rule that
    /// names a disabled browser still opens links there. This is purely a
    /// chooser-UI filter so the wheel doesn't surface browsers the user
    /// keeps installed but never wants to ask about.
    #[serde(default)]
    pub disabled_browsers: Vec<String>,
    /// UI display language preference. `System` defers to the OS / WebKit
    /// `navigator.languages` chain at startup; the other variants are hard
    /// overrides. The frontend owns the actual translation lookup (i18next
    /// keyed off this value); core only persists the choice.
    #[serde(default)]
    pub language: LanguagePref,
}

/// Visual variant for the browser-pick wheel. The three values come
/// from the design exploration (chat2.md): all share the same geometry
/// + interaction model, only the painted look differs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PickerStyle {
    /// Translucent white sectors, profile color on the outer rim.
    /// Closest to existing popover language — recommended default.
    #[default]
    Frosted,
    /// Instrument-ring look: ticks + colored dots at rest, hovered
    /// wedge paints in.
    Bezel,
    /// Apple-Watch style: the wheel surrounds a center "display" that
    /// shows the currently-aimed profile (or the default at idle).
    Crown,
}

/// Persisted UI language preference. Frontend renders the actual
/// strings; core stores the choice and exposes it via IPC + CLI so
/// `lpt settings language` and the Settings page agree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanguagePref {
    /// Follow the operating system / WebKit `navigator.languages`. The
    /// frontend resolves this to one of the explicit variants at boot.
    #[default]
    System,
    En,
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "zh-TW")]
    ZhTw,
    #[serde(rename = "ja-JP")]
    JaJp,
}

fn default_smart_routing() -> bool {
    true
}

fn default_auto_check_updates() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            history_retention_days: None,
            record_query_strings: false,
            auto_check_updates: true,
            smart_routing_enabled: true,
            picker_style: PickerStyle::Frosted,
            profile_orders: BTreeMap::new(),
            disabled_browsers: Vec::new(),
            language: LanguagePref::System,
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
