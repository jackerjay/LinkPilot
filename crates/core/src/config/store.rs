//! Config IO: path resolution, atomic write, and the in-memory store the
//! daemon holds. fsnotify integration lands in a later slice.

use std::path::{Path, PathBuf};

use thiserror::Error;
use uuid::Uuid;

use super::{ConfigDocument, Meta, WriterId};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("could not resolve a default config directory for this platform")]
    NoDefaultDir,
}

pub type Result<T> = std::result::Result<T, ConfigError>;

/// Default config path per platform:
///
/// - macOS:   `$HOME/Library/Application Support/LinkPilot/linkpilot.config.json`
/// - Linux:   `$XDG_CONFIG_HOME/linkpilot/linkpilot.config.json`
///            (or `$HOME/.config/linkpilot/linkpilot.config.json`)
/// - Windows: `%APPDATA%\LinkPilot\linkpilot.config.json`
pub fn default_config_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").ok_or(ConfigError::NoDefaultDir)?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("LinkPilot")
            .join("linkpilot.config.json"))
    }
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .ok_or(ConfigError::NoDefaultDir)?;
        Ok(base.join("linkpilot").join("linkpilot.config.json"))
    }
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or(ConfigError::NoDefaultDir)?;
        Ok(base.join("LinkPilot").join("linkpilot.config.json"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(ConfigError::NoDefaultDir)
    }
}

/// Owns the on-disk document. The daemon is the only writer in production;
/// the CLI uses it directly in v0.1 while no IPC server exists yet.
pub struct ConfigStore {
    path: PathBuf,
    doc: ConfigDocument,
    last_writer_token: Option<Uuid>,
}

impl ConfigStore {
    /// Load from `path`, or initialize the file with [`ConfigDocument::demo`]
    /// if it doesn't exist yet. Returns the populated store and a flag that
    /// is `true` when the file was just created.
    pub fn load_or_init(path: PathBuf) -> Result<(Self, bool)> {
        if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            let doc: ConfigDocument = serde_json::from_str(&raw)?;
            Ok((
                Self {
                    last_writer_token: doc.meta.last_writer_token,
                    path,
                    doc,
                },
                false,
            ))
        } else {
            let doc = ConfigDocument::demo();
            let mut store = Self {
                path,
                doc,
                last_writer_token: None,
            };
            store.persist(WriterId::Cli)?;
            Ok((store, true))
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn document(&self) -> &ConfigDocument {
        &self.doc
    }

    /// Write the in-memory document atomically (write-temp + rename) and
    /// stamp the writer token so an incoming fsnotify event can detect the
    /// echo.
    pub fn persist(&mut self, writer: WriterId) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let token = Uuid::new_v4();
        self.doc.meta = Meta {
            last_writer_token: Some(token),
            last_writer: Some(writer),
        };
        let json = serde_json::to_string_pretty(&self.doc)?;

        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &self.path)?;
        self.last_writer_token = Some(token);
        Ok(())
    }

    pub fn last_writer_token(&self) -> Option<Uuid> {
        self.last_writer_token
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("linkpilot-test-{nanos}.json"))
    }

    #[test]
    fn initializes_with_demo_when_missing() {
        let path = tmp_path();
        let (store, created) = ConfigStore::load_or_init(path.clone()).unwrap();
        assert!(created);
        assert!(!store.document().rules.is_empty(), "demo rules expected");
        assert!(path.exists());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn roundtrips_existing_file() {
        let path = tmp_path();
        let (_first, _) = ConfigStore::load_or_init(path.clone()).unwrap();
        let (second, created) = ConfigStore::load_or_init(path.clone()).unwrap();
        assert!(!created);
        assert!(!second.document().rules.is_empty());
        std::fs::remove_file(path).ok();
    }
}
