//! Config IO: path resolution, atomic write, in-memory store, and the
//! fsnotify watcher that detects external edits without re-broadcasting our
//! own writes (anti-echo).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{Event, EventKind, RecursiveMode, Watcher};
use thiserror::Error;
use uuid::Uuid;

use super::{ConfigDocument, Meta, WriterId};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("notify: {0}")]
    Notify(#[from] notify::Error),
    #[error("could not resolve a default config directory for this platform")]
    NoDefaultDir,
}

pub type Result<T> = std::result::Result<T, ConfigError>;

/// Default config path per platform:
///
/// - macOS:   `$HOME/Library/Application Support/LinkPilot/linkpilot.config.json`
/// - Linux:   `$XDG_CONFIG_HOME/linkpilot/linkpilot.config.json`
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

/// What kind of change a fsnotify subscriber receives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeOrigin {
    /// File modification matched our last write token — ignore.
    Echo,
    /// External editor / git pull changed the file; broadcast to GUI.
    External,
}

#[derive(Default)]
struct State {
    doc: ConfigDocument,
    last_writer_token: Option<Uuid>,
}

/// Owns the on-disk document. Clone-able and thread-safe: the daemon shares
/// one store between Tauri command handlers, the fsnotify thread, and the
/// (future) IPC server.
#[derive(Clone)]
pub struct ConfigStore {
    path: PathBuf,
    state: Arc<Mutex<State>>,
}

impl ConfigStore {
    /// Load from `path`, or initialize the file with [`ConfigDocument::demo`]
    /// if it doesn't exist yet. Returns the populated store and `true` when
    /// the file was just created.
    pub fn load_or_init(path: PathBuf) -> Result<(Self, bool)> {
        if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            // v0.2 removed `Rule.priority`; list order is now the single
            // source of priority truth. If we're loading a pre-v0.2
            // config that still has `priority` on its rules, honor the
            // user's intent by reordering rules by priority desc
            // (stable), then strip the field. Migration is one-shot —
            // the migrated doc gets persisted below.
            let (doc, migrated) = parse_with_priority_migration(&raw)?;
            let store = Self {
                path,
                state: Arc::new(Mutex::new(State {
                    doc: doc.clone(),
                    last_writer_token: doc.meta.last_writer_token,
                })),
            };
            if migrated {
                tracing::info!(
                    "config: migrated legacy `priority` field — list order is now authoritative"
                );
                // Stamp a fresh writer token + drop priority from disk.
                store.persist(WriterId::Cli)?;
            }
            Ok((store, false))
        } else {
            let store = Self {
                path,
                state: Arc::new(Mutex::new(State {
                    doc: ConfigDocument::demo(),
                    last_writer_token: None,
                })),
            };
            store.persist(WriterId::Cli)?;
            Ok((store, true))
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Snapshot of the current document.
    pub fn document(&self) -> ConfigDocument {
        self.state
            .lock()
            .expect("config store mutex poisoned")
            .doc
            .clone()
    }

    /// Replace the in-memory document and persist it.
    pub fn replace(&self, doc: ConfigDocument, writer: WriterId) -> Result<()> {
        {
            let mut guard = self.state.lock().expect("config store mutex poisoned");
            guard.doc = doc;
        }
        self.persist(writer)
    }

    /// Atomically rewrite the on-disk file using the in-memory document.
    /// Stamps the meta token so fsnotify echoes can be ignored.
    pub fn persist(&self, writer: WriterId) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let token = Uuid::new_v4();
        let json = {
            let mut guard = self.state.lock().expect("config store mutex poisoned");
            guard.doc.meta = Meta {
                last_writer_token: Some(token),
                last_writer: Some(writer),
            };
            guard.last_writer_token = Some(token);
            serde_json::to_string_pretty(&guard.doc)?
        };

        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    /// Watch the on-disk file. `on_change` fires on every notify event we
    /// observe; the [`ChangeOrigin`] argument distinguishes self-echoes from
    /// real external edits. Returns the watcher handle — drop it to stop
    /// watching.
    pub fn watch<F>(&self, on_change: F) -> Result<RecommendedWatcherHandle>
    where
        F: Fn(ChangeOrigin) + Send + 'static,
    {
        let path = self.path.clone();
        let state = Arc::clone(&self.state);

        // Watch the parent dir so we still get events after atomic rename.
        let watch_dir = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        std::fs::create_dir_all(&watch_dir)?;

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            let Ok(event) = res else {
                return;
            };
            let touches_file = event.paths.iter().any(|p| p == &path);
            if !touches_file {
                return;
            }
            if !matches!(
                event.kind,
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Any
            ) {
                return;
            }
            // Small debounce: notify can fire before the writer finishes
            // (atomic-rename produces Create immediately after rename).
            std::thread::sleep(Duration::from_millis(30));
            handle_disk_change(&path, &state, &on_change);
        })?;
        watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;
        Ok(RecommendedWatcherHandle { _watcher: watcher })
    }
}

/// Parse a config JSON string, applying the v0.2 list-order priority
/// migration when needed. Returns `(doc, migrated)` where `migrated`
/// is `true` iff at least one rule carried a legacy `priority` field
/// (meaning the caller should persist the cleaned doc back to disk).
///
/// The migration walks the raw JSON's `rules` array, reads each
/// rule's `priority` (defaulting to 0), reorders the array by
/// priority descending using a stable sort (so equal-priority rules
/// keep their relative on-disk order), strips the field, then parses
/// the cleaned JSON into a `ConfigDocument`.
fn parse_with_priority_migration(raw: &str) -> Result<(ConfigDocument, bool)> {
    let mut value: serde_json::Value = serde_json::from_str(raw)?;
    let mut migrated = false;
    if let Some(rules) = value.get_mut("rules").and_then(|v| v.as_array_mut()) {
        let any_priority = rules
            .iter()
            .any(|r| r.as_object().map(|o| o.contains_key("priority")).unwrap_or(false));
        if any_priority {
            migrated = true;
            // Stable sort by `priority` desc; missing → 0.
            rules.sort_by(|a, b| {
                let pa = a.get("priority").and_then(|v| v.as_i64()).unwrap_or(0);
                let pb = b.get("priority").and_then(|v| v.as_i64()).unwrap_or(0);
                pb.cmp(&pa)
            });
            for r in rules.iter_mut() {
                if let Some(obj) = r.as_object_mut() {
                    obj.remove("priority");
                }
            }
        }
    }
    let doc: ConfigDocument = serde_json::from_value(value)?;
    Ok((doc, migrated))
}

fn handle_disk_change<F>(path: &Path, state: &Arc<Mutex<State>>, on_change: &F)
where
    F: Fn(ChangeOrigin),
{
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) => {
            tracing::warn!(?err, "config watcher: read failed");
            return;
        }
    };
    let (parsed, _migrated) = match parse_with_priority_migration(&raw) {
        Ok(p) => p,
        Err(err) => {
            tracing::warn!(?err, "config watcher: parse failed");
            return;
        }
    };
    let mut guard = state.lock().expect("config store mutex poisoned");
    let origin = match (parsed.meta.last_writer_token, guard.last_writer_token) {
        (Some(disk), Some(remembered)) if disk == remembered => ChangeOrigin::Echo,
        _ => ChangeOrigin::External,
    };
    if matches!(origin, ChangeOrigin::External) {
        guard.doc = parsed;
        guard.last_writer_token = guard.doc.meta.last_writer_token;
    }
    drop(guard);
    on_change(origin);
}

/// Drop-guard wrapper around the platform watcher. The watcher stops when
/// this value is dropped.
pub struct RecommendedWatcherHandle {
    _watcher: notify::RecommendedWatcher,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path() -> PathBuf {
        // Per-call UUID instead of SystemTime nanos: `cargo test` runs the
        // three tests below in parallel, and on macOS SystemTime resolves
        // at microsecond granularity (not nanoseconds like Linux), so two
        // calls in the same microsecond produce the same path and the
        // tests race each other's `remove_file`.
        std::env::temp_dir().join(format!("linkpilot-test-{}.json", Uuid::new_v4()))
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

    #[test]
    fn migrates_legacy_priority_to_list_order() {
        // Hand-written legacy config: two rules, the SECOND one has the
        // higher numeric priority. After migration the order should
        // flip and both `priority` fields should be gone from disk.
        let path = tmp_path();
        let legacy = serde_json::json!({
            "version": 1,
            "default_target": { "browser": "arc" },
            "rules": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "priority": 10,
                    "enabled": true,
                    "when": { "op": "url-host", "pattern": "github.com" },
                    "then": { "kind": "open", "target": { "browser": "chrome" } }
                },
                {
                    "id": "22222222-2222-2222-2222-222222222222",
                    "priority": 100,
                    "enabled": true,
                    "when": { "op": "source-app", "name": "Lark" },
                    "then": { "kind": "ask" }
                }
            ]
        });
        std::fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

        let (store, created) = ConfigStore::load_or_init(path.clone()).unwrap();
        assert!(!created);
        let doc = store.document();
        assert_eq!(doc.rules.len(), 2);
        // Lark (was prio 100) now comes first.
        assert_eq!(doc.rules[0].id.0.to_string(), "22222222-2222-2222-2222-222222222222");
        assert_eq!(doc.rules[1].id.0.to_string(), "11111111-1111-1111-1111-111111111111");

        // And `priority` is gone from disk — second load is a no-op
        // (no migration, identical doc).
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(!on_disk.contains("\"priority\""), "priority field must be stripped");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn replace_persists_and_updates_token() {
        let path = tmp_path();
        let (store, _) = ConfigStore::load_or_init(path.clone()).unwrap();
        let mut doc = store.document();
        doc.rules.clear();
        store.replace(doc, WriterId::Gui).unwrap();

        let (reloaded, _) = ConfigStore::load_or_init(path.clone()).unwrap();
        assert!(reloaded.document().rules.is_empty());
        std::fs::remove_file(path).ok();
    }
}
