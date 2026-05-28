//! Ask-mode behavior recording.
//!
//! Every time a user resolves an `Action::Ask` route via the picker we
//! append one [`Observation`] line to an NDJSON file alongside the
//! config:
//!
//! ```text
//! ~/Library/Application Support/LinkPilot/observations.ndjson
//! ```
//!
//! Aggregating the log on demand produces [`Suggestion`]s — host +
//! target patterns with enough repetition to warrant offering "make
//! this a rule". Dismissed suggestions are muted for 30 days via a
//! separate `observations-dismissed.json` file.
//!
//! The file format is intentionally LLM-friendly NDJSON: each line is a
//! self-contained, well-typed observation that a future ingestion
//! pipeline can stream without needing to parse the entire log up
//! front. See `docs/` for the contract.
//!
//! All data stays on disk under the user's `Application Support` —
//! nothing leaves the machine. The default-on opt-out lives on
//! [`crate::config::Settings::behavior_log_enabled`].

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::browser::{BrowserId, BrowserTarget};

/// Surface a [`Suggestion`] only when the user resolved the picker the
/// same way at least this many times for a given (host, target) tuple.
pub const MIN_OBSERVATIONS: u32 = 3;

/// And only when the chosen target accounts for at least this share of
/// resolutions for the host. (3 picks for arc out of 4 total = 0.75 → ok;
/// 3 of 5 = 0.6 → still ambiguous, don't suggest.)
pub const MIN_CONFIDENCE: f32 = 0.7;

/// Dismissed suggestions stay hidden for this many days.
pub const DISMISSAL_TTL_DAYS: u64 = 30;

const MS_PER_DAY: u64 = 86_400_000;

/// One ask-picker resolution. Persisted as a single NDJSON line.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Observation {
    pub timestamp_ms: u64,
    pub host: String,
    /// Source app at the time of the open, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_app: Option<ObservedSourceApp>,
    pub browser_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

/// Display + stable-id pair for the source app. Bundle id (when
/// available) is what we'd key suggestions on; name is for the UI /
/// LLM context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservedSourceApp {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
}

/// Aggregated pattern surfaced to the UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Suggestion {
    pub host: String,
    pub browser_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub observation_count: u32,
    pub confidence: f32,
    pub last_observed_ms: u64,
}

impl Suggestion {
    /// Build a [`BrowserTarget`] ready to plug into `Action::Open` when
    /// the user accepts the suggestion.
    pub fn to_target(&self) -> BrowserTarget {
        let mut target = BrowserTarget::new(BrowserId::new(&self.browser_id));
        if let Some(p) = &self.profile_id {
            target = target.with_profile(p);
        }
        target
    }
}

/// Identity tuple a dismissal entry mutes — same shape we surface in
/// suggestions, minus the metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DismissedSuggestion {
    pub host: String,
    pub browser_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub dismissed_at_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct DismissedFile {
    #[serde(default)]
    entries: Vec<DismissedSuggestion>,
}

/// Persistent store. Holds nothing in memory beyond the path + a
/// write-serializing mutex; observations are streamed from disk on
/// every aggregation pass. The data set stays small (one line per ask
/// resolution, retention-trimmed) so this is cheap.
pub struct ObservationsStore {
    observations_path: PathBuf,
    dismissed_path: PathBuf,
    write_lock: Mutex<()>,
}

impl ObservationsStore {
    pub fn new(observations_path: PathBuf, dismissed_path: PathBuf) -> Self {
        Self {
            observations_path,
            dismissed_path,
            write_lock: Mutex::new(()),
        }
    }

    pub fn observations_path(&self) -> &Path {
        &self.observations_path
    }

    pub fn dismissed_path(&self) -> &Path {
        &self.dismissed_path
    }

    /// Append one observation. The write itself is one short
    /// `writeln!` — well under POSIX `PIPE_BUF`, so the line lands
    /// atomically even if another process appends concurrently.
    pub fn record(&self, obs: &Observation) -> io::Result<()> {
        let _guard = self
            .write_lock
            .lock()
            .expect("observations store mutex poisoned");
        if let Some(parent) = self.observations_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let line = serde_json::to_string(obs).map_err(io::Error::other)?;
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.observations_path)?;
        writeln!(f, "{line}")
    }

    /// Read every observation. Malformed lines are skipped silently —
    /// partial data beats refusing to aggregate when the log is being
    /// appended to concurrently.
    pub fn list_observations(&self) -> io::Result<Vec<Observation>> {
        let f = match File::open(&self.observations_path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut out = Vec::new();
        for line in BufReader::new(f).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(obs) = serde_json::from_str::<Observation>(&line) {
                out.push(obs);
            }
        }
        Ok(out)
    }

    /// Aggregate observations into actionable Suggestions, applying the
    /// dismissal mask.
    pub fn list_suggestions(&self) -> io::Result<Vec<Suggestion>> {
        let observations = self.list_observations()?;
        let dismissed = self.load_dismissed()?;
        let active = active_dismissals(&dismissed.entries, now_ms());
        Ok(aggregate(&observations, &active))
    }

    fn load_dismissed(&self) -> io::Result<DismissedFile> {
        let raw = match std::fs::read_to_string(&self.dismissed_path) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(DismissedFile::default()),
            Err(e) => return Err(e),
        };
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    /// Mute a (host, browser_id, profile_id) tuple for 30 days. Existing
    /// entries for the same tuple are replaced with a fresh timestamp.
    pub fn dismiss(
        &self,
        host: &str,
        browser_id: &str,
        profile_id: Option<&str>,
    ) -> io::Result<()> {
        let _guard = self
            .write_lock
            .lock()
            .expect("observations store mutex poisoned");
        let mut file = self.load_dismissed()?;
        file.entries.retain(|e| {
            !(e.host == host && e.browser_id == browser_id && e.profile_id.as_deref() == profile_id)
        });
        file.entries.push(DismissedSuggestion {
            host: host.to_string(),
            browser_id: browser_id.to_string(),
            profile_id: profile_id.map(str::to_string),
            dismissed_at_ms: now_ms(),
        });
        // Opportunistically prune entries past their TTL so the file
        // stays small even after years of use.
        let cutoff = now_ms().saturating_sub(DISMISSAL_TTL_DAYS * MS_PER_DAY);
        file.entries.retain(|e| e.dismissed_at_ms >= cutoff);
        let bytes = serde_json::to_vec_pretty(&file).map_err(io::Error::other)?;
        write_atomic(&self.dismissed_path, &bytes)
    }

    /// Truncate the observations log. Atomic rename so concurrent
    /// readers always see either the old file or the empty new one,
    /// never torn content.
    pub fn clear_observations(&self) -> io::Result<()> {
        let _guard = self
            .write_lock
            .lock()
            .expect("observations store mutex poisoned");
        if !self.observations_path.exists() {
            return Ok(());
        }
        write_atomic(&self.observations_path, b"")
    }

    /// Drop observations older than `retention_days` and rewrite
    /// atomically. `None` retains forever.
    pub fn retain_within(&self, retention_days: Option<u32>) -> io::Result<()> {
        let Some(days) = retention_days else {
            return Ok(());
        };
        let _guard = self
            .write_lock
            .lock()
            .expect("observations store mutex poisoned");
        let cutoff = now_ms().saturating_sub(u64::from(days) * MS_PER_DAY);
        let observations = self.list_observations()?;
        let kept: Vec<&Observation> = observations
            .iter()
            .filter(|o| o.timestamp_ms >= cutoff)
            .collect();
        if kept.len() == observations.len() {
            return Ok(());
        }
        let mut buf = Vec::with_capacity(kept.len() * 128);
        for obs in &kept {
            let line = serde_json::to_string(obs).map_err(io::Error::other)?;
            buf.extend_from_slice(line.as_bytes());
            buf.push(b'\n');
        }
        write_atomic(&self.observations_path, &buf)
    }

    /// Copy the NDJSON log to `dst`. Surfaces "no observations yet" as
    /// an empty file rather than a 404 so the caller can rely on a
    /// file being present after this returns.
    pub fn export(&self, dst: &Path) -> io::Result<()> {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match std::fs::copy(&self.observations_path, dst) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => std::fs::write(dst, b""),
            Err(e) => Err(e),
        }
    }
}

/// `~/Library/Application Support/LinkPilot/observations.ndjson` on macOS
/// (other platforms follow the same root as [`crate::config::default_config_path`]).
/// Returns the same error variant as the config path resolver when the
/// home / app-support dir can't be located.
pub fn default_observations_path() -> Result<PathBuf, crate::config::store::ConfigError> {
    Ok(crate::config::default_config_path()?.with_file_name("observations.ndjson"))
}

/// `~/Library/Application Support/LinkPilot/observations-dismissed.json`
pub fn default_dismissed_path() -> Result<PathBuf, crate::config::store::ConfigError> {
    Ok(crate::config::default_config_path()?.with_file_name("observations-dismissed.json"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Write to `<path>.tmp` then atomically rename over `<path>`.
fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)
}

fn active_dismissals(
    entries: &[DismissedSuggestion],
    now: u64,
) -> HashSet<(String, String, Option<String>)> {
    let cutoff = now.saturating_sub(DISMISSAL_TTL_DAYS * MS_PER_DAY);
    entries
        .iter()
        .filter(|e| e.dismissed_at_ms >= cutoff)
        .map(|e| (e.host.clone(), e.browser_id.clone(), e.profile_id.clone()))
        .collect()
}

/// Pure aggregation: group observations by (host, browser_id, profile_id),
/// apply thresholds + dismissal mask. Sorted by (confidence desc,
/// observation_count desc, last_observed desc) for stable rendering.
pub fn aggregate(
    observations: &[Observation],
    dismissed: &HashSet<(String, String, Option<String>)>,
) -> Vec<Suggestion> {
    if observations.is_empty() {
        return Vec::new();
    }
    let mut picks: HashMap<(String, String, Option<String>), (u32, u64)> = HashMap::new();
    let mut totals: HashMap<String, u32> = HashMap::new();
    for o in observations {
        let key = (o.host.clone(), o.browser_id.clone(), o.profile_id.clone());
        let entry = picks.entry(key).or_insert((0, 0));
        entry.0 += 1;
        entry.1 = entry.1.max(o.timestamp_ms);
        *totals.entry(o.host.clone()).or_insert(0) += 1;
    }

    let mut out: Vec<Suggestion> = picks
        .into_iter()
        .filter_map(|((host, browser_id, profile_id), (count, last))| {
            if count < MIN_OBSERVATIONS {
                return None;
            }
            let total = *totals.get(&host).unwrap_or(&0);
            if total == 0 {
                return None;
            }
            let confidence = count as f32 / total as f32;
            if confidence < MIN_CONFIDENCE {
                return None;
            }
            if dismissed.contains(&(host.clone(), browser_id.clone(), profile_id.clone())) {
                return None;
            }
            Some(Suggestion {
                host,
                browser_id,
                profile_id,
                observation_count: count,
                confidence,
                last_observed_ms: last,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.observation_count.cmp(&a.observation_count))
            .then(b.last_observed_ms.cmp(&a.last_observed_ms))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(host: &str, browser: &str, profile: Option<&str>, ts: u64) -> Observation {
        Observation {
            timestamp_ms: ts,
            host: host.to_string(),
            source_app: None,
            browser_id: browser.to_string(),
            profile_id: profile.map(str::to_string),
        }
    }

    #[test]
    fn aggregate_drops_under_threshold() {
        let observations = vec![obs("a.com", "arc", None, 1), obs("a.com", "arc", None, 2)];
        assert!(aggregate(&observations, &HashSet::new()).is_empty());
    }

    #[test]
    fn aggregate_surfaces_high_confidence_unanimous() {
        let observations = vec![
            obs("a.com", "arc", Some("Work"), 1),
            obs("a.com", "arc", Some("Work"), 2),
            obs("a.com", "arc", Some("Work"), 3),
        ];
        let out = aggregate(&observations, &HashSet::new());
        assert_eq!(out.len(), 1);
        let s = &out[0];
        assert_eq!(s.host, "a.com");
        assert_eq!(s.browser_id, "arc");
        assert_eq!(s.profile_id.as_deref(), Some("Work"));
        assert_eq!(s.observation_count, 3);
        assert!((s.confidence - 1.0).abs() < 1e-6);
        assert_eq!(s.last_observed_ms, 3);
    }

    #[test]
    fn aggregate_respects_confidence_floor() {
        // 3 of 5 = 0.6 → below 0.7, no suggestion for either side.
        let observations = vec![
            obs("a.com", "arc", None, 1),
            obs("a.com", "arc", None, 2),
            obs("a.com", "arc", None, 3),
            obs("a.com", "chrome", None, 4),
            obs("a.com", "chrome", None, 5),
        ];
        assert!(aggregate(&observations, &HashSet::new()).is_empty());
    }

    #[test]
    fn aggregate_mask_dismissed_pairs() {
        let observations = vec![
            obs("a.com", "arc", None, 1),
            obs("a.com", "arc", None, 2),
            obs("a.com", "arc", None, 3),
        ];
        let mut dismissed = HashSet::new();
        dismissed.insert(("a.com".to_string(), "arc".to_string(), None));
        assert!(aggregate(&observations, &dismissed).is_empty());
    }

    #[test]
    fn aggregate_sorts_by_confidence_then_count() {
        // host A: 4 arc + 1 chrome → arc 0.8 conf, 4 obs
        // host B: 5 chrome → chrome 1.0 conf, 5 obs (should rank first)
        let observations = vec![
            obs("a.com", "arc", None, 1),
            obs("a.com", "arc", None, 2),
            obs("a.com", "arc", None, 3),
            obs("a.com", "arc", None, 4),
            obs("a.com", "chrome", None, 5),
            obs("b.com", "chrome", None, 6),
            obs("b.com", "chrome", None, 7),
            obs("b.com", "chrome", None, 8),
            obs("b.com", "chrome", None, 9),
            obs("b.com", "chrome", None, 10),
        ];
        let out = aggregate(&observations, &HashSet::new());
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].host, "b.com");
        assert_eq!(out[1].host, "a.com");
    }

    #[test]
    fn observation_roundtrips_through_ndjson() {
        let original = Observation {
            timestamp_ms: 1_716_800_000_000,
            host: "github.com".into(),
            source_app: Some(ObservedSourceApp {
                name: "Slack".into(),
                bundle_id: Some("com.tinyspeck.slackmacapp".into()),
            }),
            browser_id: "arc".into(),
            profile_id: Some("Work".into()),
        };
        let line = serde_json::to_string(&original).unwrap();
        let parsed: Observation = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn suggestion_to_target_includes_profile() {
        let s = Suggestion {
            host: "a.com".into(),
            browser_id: "arc".into(),
            profile_id: Some("Work".into()),
            observation_count: 5,
            confidence: 1.0,
            last_observed_ms: 0,
        };
        let target = s.to_target();
        assert_eq!(target.browser.0, "arc");
        assert_eq!(target.profile.as_deref(), Some("Work"));
    }

    #[test]
    fn active_dismissals_expire() {
        let now = 100 * MS_PER_DAY;
        let entries = vec![
            DismissedSuggestion {
                host: "old.com".into(),
                browser_id: "arc".into(),
                profile_id: None,
                dismissed_at_ms: now - (DISMISSAL_TTL_DAYS + 5) * MS_PER_DAY,
            },
            DismissedSuggestion {
                host: "recent.com".into(),
                browser_id: "arc".into(),
                profile_id: None,
                dismissed_at_ms: now - 1,
            },
        ];
        let active = active_dismissals(&entries, now);
        assert!(!active.contains(&("old.com".to_string(), "arc".to_string(), None)));
        assert!(active.contains(&("recent.com".to_string(), "arc".to_string(), None)));
    }
}
