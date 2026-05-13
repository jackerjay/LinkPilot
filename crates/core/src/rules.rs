//! Rule model: matchers, actions, and the building blocks the router evaluates.
//!
//! v0.1 ships a deliberately small surface — enough to express the PRD §22
//! demo rules. Later phases (v0.2+) extend [`MatcherTree`] with the remaining
//! PRD §9.3 dimensions (VPN, time, transition type, etc.).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::browser::BrowserTarget;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuleId(pub Uuid);

impl RuleId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for RuleId {
    fn default() -> Self {
        Self::new()
    }
}

/// Where a rule originated. GUI rules are editable inline; TsCompiled rules
/// are read-only in the GUI (Phase 2+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum RuleSource {
    #[default]
    Gui,
    File,
    TsCompiled,
}

/// A single routing rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: RuleId,
    /// Higher priority wins; ties broken by list order.
    pub priority: i32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub when: MatcherTree,
    pub then: Action,
    #[serde(default)]
    pub source: RuleSource,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_enabled() -> bool {
    true
}

/// Matcher AST. Composable so the GUI can render arbitrary boolean trees.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum MatcherTree {
    Always,
    All { of: Vec<MatcherTree> },
    Any { of: Vec<MatcherTree> },
    Not { of: Box<MatcherTree> },

    /// Glob-style host match: `github.com`, `*.corp.example.com`.
    UrlHost { pattern: String },
    UrlPath { pattern: String },

    /// Source application: matches by bundle id when set (stable across
    /// locales + display-name vs CFBundleName quirks), else falls back to
    /// case-insensitive name match. Older configs without bundle_id keep
    /// working via the default value.
    SourceApp {
        name: String,
        #[serde(default)]
        bundle_id: Option<String>,
    },
    /// Source browser id (when navigation came from extension).
    SourceBrowser { browser: String },
    /// Source profile id within the source browser.
    SourceProfile { profile: String },
}

/// What to do when a rule matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Action {
    Open { target: BrowserTarget },
    /// Keep navigation in the source browser (e.g. OAuth flows).
    KeepSource,
    Ask,
    Block,
}
