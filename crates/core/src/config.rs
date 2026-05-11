//! Configuration document — the on-disk JSON single source of truth.
//!
//! The full anti-echo / fsnotify pipeline lands in a later step. v0.1 step 1
//! only ships the schema and a minimal in-memory store; persistence and
//! file-watching are wired up alongside `platform-mac` in step 4.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::browser::{BrowserId, BrowserTarget};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub launch_at_login: bool,
    #[serde(default)]
    pub history_retention_days: Option<u32>,
    #[serde(default)]
    pub record_query_strings: bool,
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
