use std::path::{Path, PathBuf};
use std::process::Command;

use linkpilot_core::browser::{BrowserId, BrowserKind, BrowserProfile, InstalledBrowser};
use linkpilot_core::inventory::{parse_chromium_profiles, parse_firefox_profiles};
use linkpilot_core::platform::{BrowserInventory, PlatformError, Result};

pub struct MacInventory;

/// Static registry of browsers LinkPilot knows how to drive on macOS.
///
/// Adding a browser is a single row here plus (potentially) a launcher arm.
struct KnownBrowser {
    id: &'static str,
    display_name: &'static str,
    kind: BrowserKind,
    bundle_id: &'static str,
    /// Candidate bundle directory names under `/Applications` or `~/Applications`.
    app_dir_names: &'static [&'static str],
    /// Fallback executable name inside `Contents/MacOS/`; Info.plist wins.
    binary: &'static str,
    /// Candidate profile roots, relative to `~/Library/Application Support/`.
    /// Empty for Safari (no profile concept until macOS 14 introduced its
    /// limited form, which has no usable external API for v0.1).
    profile_roots_rel: &'static [&'static str],
}

const REGISTRY: &[KnownBrowser] = &[
    KnownBrowser {
        id: "chrome",
        display_name: "Google Chrome",
        kind: BrowserKind::Chromium,
        bundle_id: "com.google.Chrome",
        app_dir_names: &["Google Chrome.app"],
        binary: "Google Chrome",
        profile_roots_rel: &["Google/Chrome"],
    },
    KnownBrowser {
        id: "edge",
        display_name: "Microsoft Edge",
        kind: BrowserKind::Chromium,
        bundle_id: "com.microsoft.edgemac",
        app_dir_names: &["Microsoft Edge.app"],
        binary: "Microsoft Edge",
        profile_roots_rel: &["Microsoft Edge"],
    },
    KnownBrowser {
        id: "brave",
        display_name: "Brave Browser",
        kind: BrowserKind::Chromium,
        bundle_id: "com.brave.Browser",
        app_dir_names: &["Brave Browser.app"],
        binary: "Brave Browser",
        profile_roots_rel: &["BraveSoftware/Brave-Browser"],
    },
    KnownBrowser {
        id: "vivaldi",
        display_name: "Vivaldi",
        kind: BrowserKind::Chromium,
        bundle_id: "com.vivaldi.Vivaldi",
        app_dir_names: &["Vivaldi.app"],
        binary: "Vivaldi",
        profile_roots_rel: &["Vivaldi"],
    },
    KnownBrowser {
        id: "opera",
        display_name: "Opera",
        kind: BrowserKind::Chromium,
        bundle_id: "com.operasoftware.Opera",
        app_dir_names: &["Opera.app"],
        binary: "Opera",
        profile_roots_rel: &["com.operasoftware.Opera", "Opera"],
    },
    KnownBrowser {
        id: "opera-gx",
        display_name: "Opera GX",
        kind: BrowserKind::Chromium,
        bundle_id: "com.operasoftware.OperaGX",
        app_dir_names: &["Opera GX.app"],
        binary: "Opera GX",
        profile_roots_rel: &["com.operasoftware.OperaGX", "Opera GX"],
    },
    KnownBrowser {
        id: "dia",
        display_name: "Dia",
        kind: BrowserKind::Chromium,
        bundle_id: "company.thebrowser.dia",
        app_dir_names: &["Dia.app"],
        binary: "Dia",
        profile_roots_rel: &["Dia/User Data"],
    },
    KnownBrowser {
        id: "atlas",
        display_name: "ChatGPT Atlas",
        kind: BrowserKind::Chromium,
        bundle_id: "com.openai.atlas",
        app_dir_names: &["ChatGPT Atlas.app", "Atlas.app"],
        binary: "ChatGPT Atlas",
        profile_roots_rel: &["com.openai.atlas/browser-data/host"],
    },
    KnownBrowser {
        id: "comet",
        display_name: "Comet",
        kind: BrowserKind::Chromium,
        bundle_id: "ai.perplexity.comet",
        app_dir_names: &["Comet.app", "Perplexity Comet.app"],
        binary: "Comet",
        profile_roots_rel: &["Comet", "Perplexity/Comet", "com.perplexity.comet"],
    },
    KnownBrowser {
        id: "arc",
        display_name: "Arc",
        kind: BrowserKind::Arc,
        bundle_id: "company.thebrowser.Browser",
        app_dir_names: &["Arc.app"],
        binary: "Arc",
        profile_roots_rel: &["Arc/User Data"],
    },
    KnownBrowser {
        id: "firefox",
        display_name: "Firefox",
        kind: BrowserKind::Firefox,
        bundle_id: "org.mozilla.firefox",
        app_dir_names: &["Firefox.app"],
        binary: "firefox",
        profile_roots_rel: &["Firefox"],
    },
    KnownBrowser {
        id: "zen",
        display_name: "Zen Browser",
        kind: BrowserKind::Firefox,
        bundle_id: "app.zen-browser.zen",
        app_dir_names: &["Zen Browser.app", "Zen.app"],
        binary: "zen",
        profile_roots_rel: &["zen", "Zen"],
    },
    KnownBrowser {
        id: "librewolf",
        display_name: "LibreWolf",
        kind: BrowserKind::Firefox,
        bundle_id: "io.gitlab.librewolf-community",
        app_dir_names: &["LibreWolf.app"],
        binary: "librewolf",
        profile_roots_rel: &["LibreWolf"],
    },
    KnownBrowser {
        id: "waterfox",
        display_name: "Waterfox",
        kind: BrowserKind::Firefox,
        bundle_id: "net.waterfox.waterfox",
        app_dir_names: &["Waterfox.app"],
        binary: "waterfox",
        profile_roots_rel: &["Waterfox"],
    },
    KnownBrowser {
        id: "floorp",
        display_name: "Floorp",
        kind: BrowserKind::Firefox,
        bundle_id: "one.ablaze.floorp",
        app_dir_names: &["Floorp.app"],
        binary: "floorp",
        profile_roots_rel: &["Floorp"],
    },
    KnownBrowser {
        id: "mullvad-browser",
        display_name: "Mullvad Browser",
        kind: BrowserKind::Firefox,
        bundle_id: "net.mullvad.mullvadbrowser",
        app_dir_names: &["Mullvad Browser.app"],
        binary: "mullvadbrowser",
        profile_roots_rel: &["MullvadBrowser-Data/Browser", "Mullvad Browser"],
    },
    KnownBrowser {
        id: "tor-browser",
        display_name: "Tor Browser",
        kind: BrowserKind::Firefox,
        bundle_id: "org.torproject.torbrowser",
        app_dir_names: &["Tor Browser.app"],
        binary: "firefox",
        profile_roots_rel: &["TorBrowser-Data/Browser"],
    },
    KnownBrowser {
        id: "safari",
        display_name: "Safari",
        kind: BrowserKind::Safari,
        bundle_id: "com.apple.Safari",
        app_dir_names: &["Safari.app"],
        binary: "Safari",
        profile_roots_rel: &[],
    },
    KnownBrowser {
        id: "orion",
        display_name: "Orion",
        kind: BrowserKind::Unknown,
        bundle_id: "com.kagi.kagimacOS",
        app_dir_names: &["Orion.app"],
        binary: "Orion",
        profile_roots_rel: &[],
    },
    KnownBrowser {
        id: "duckduckgo",
        display_name: "DuckDuckGo",
        kind: BrowserKind::Unknown,
        bundle_id: "com.duckduckgo.macos.browser",
        app_dir_names: &["DuckDuckGo.app", "DuckDuckGo Browser.app"],
        binary: "DuckDuckGo",
        profile_roots_rel: &[],
    },
    KnownBrowser {
        id: "yandex",
        display_name: "Yandex Browser",
        kind: BrowserKind::Chromium,
        bundle_id: "ru.yandex.desktop.yandex-browser",
        app_dir_names: &["Yandex.app", "Yandex Browser.app"],
        binary: "Yandex",
        profile_roots_rel: &["Yandex/YandexBrowser", "Yandex Browser"],
    },
    KnownBrowser {
        id: "whale",
        display_name: "Naver Whale",
        kind: BrowserKind::Chromium,
        bundle_id: "com.naver.Whale",
        app_dir_names: &["Whale.app", "Naver Whale.app"],
        binary: "Whale",
        profile_roots_rel: &["Naver/Whale", "Whale"],
    },
    // -------- channel variants of mainstream browsers --------
    // Power users frequently keep stable + a dev channel side by side
    // (debugging extensions, testing CSP behaviour). Profile roots are
    // separate from the stable channel — Chrome Canary stores under
    // `Google/Chrome Canary`, Edge Dev under `Microsoft Edge Dev`, etc.
    // — so routing to a channel-specific id won't accidentally open
    // the stable channel's profile.
    KnownBrowser {
        id: "safari-technology-preview",
        display_name: "Safari Technology Preview",
        kind: BrowserKind::Safari,
        bundle_id: "com.apple.SafariTechnologyPreview",
        app_dir_names: &["Safari Technology Preview.app"],
        binary: "Safari Technology Preview",
        profile_roots_rel: &[],
    },
    KnownBrowser {
        id: "chrome-beta",
        display_name: "Google Chrome Beta",
        kind: BrowserKind::Chromium,
        bundle_id: "com.google.Chrome.beta",
        app_dir_names: &["Google Chrome Beta.app"],
        binary: "Google Chrome Beta",
        profile_roots_rel: &["Google/Chrome Beta"],
    },
    KnownBrowser {
        id: "chrome-dev",
        display_name: "Google Chrome Dev",
        kind: BrowserKind::Chromium,
        bundle_id: "com.google.Chrome.dev",
        app_dir_names: &["Google Chrome Dev.app"],
        binary: "Google Chrome Dev",
        profile_roots_rel: &["Google/Chrome Dev"],
    },
    KnownBrowser {
        id: "chrome-canary",
        display_name: "Google Chrome Canary",
        kind: BrowserKind::Chromium,
        bundle_id: "com.google.Chrome.canary",
        app_dir_names: &["Google Chrome Canary.app"],
        binary: "Google Chrome Canary",
        profile_roots_rel: &["Google/Chrome Canary"],
    },
    KnownBrowser {
        id: "edge-beta",
        display_name: "Microsoft Edge Beta",
        kind: BrowserKind::Chromium,
        bundle_id: "com.microsoft.edgemac.Beta",
        app_dir_names: &["Microsoft Edge Beta.app"],
        binary: "Microsoft Edge Beta",
        profile_roots_rel: &["Microsoft Edge Beta"],
    },
    KnownBrowser {
        id: "edge-dev",
        display_name: "Microsoft Edge Dev",
        kind: BrowserKind::Chromium,
        bundle_id: "com.microsoft.edgemac.Dev",
        app_dir_names: &["Microsoft Edge Dev.app"],
        binary: "Microsoft Edge Dev",
        profile_roots_rel: &["Microsoft Edge Dev"],
    },
    KnownBrowser {
        id: "edge-canary",
        display_name: "Microsoft Edge Canary",
        kind: BrowserKind::Chromium,
        bundle_id: "com.microsoft.edgemac.Canary",
        app_dir_names: &["Microsoft Edge Canary.app"],
        binary: "Microsoft Edge Canary",
        profile_roots_rel: &["Microsoft Edge Canary"],
    },
    KnownBrowser {
        id: "brave-beta",
        display_name: "Brave Browser Beta",
        kind: BrowserKind::Chromium,
        bundle_id: "com.brave.Browser.beta",
        app_dir_names: &["Brave Browser Beta.app"],
        binary: "Brave Browser Beta",
        profile_roots_rel: &["BraveSoftware/Brave-Browser-Beta"],
    },
    KnownBrowser {
        id: "brave-nightly",
        display_name: "Brave Browser Nightly",
        kind: BrowserKind::Chromium,
        bundle_id: "com.brave.Browser.nightly",
        app_dir_names: &["Brave Browser Nightly.app"],
        binary: "Brave Browser Nightly",
        profile_roots_rel: &["BraveSoftware/Brave-Browser-Nightly"],
    },
    KnownBrowser {
        id: "firefox-developer-edition",
        display_name: "Firefox Developer Edition",
        kind: BrowserKind::Firefox,
        bundle_id: "org.mozilla.firefoxdeveloperedition",
        app_dir_names: &["Firefox Developer Edition.app"],
        // The Developer Edition .app ships with the standard `firefox`
        // CFBundleExecutable, but the user's `profiles.ini` lives at the
        // same path as stable Firefox — the channel is encoded in the
        // profile name (`*.dev-edition-default`) rather than a separate
        // root. We point at the shared root so profile parsing surfaces
        // both stable + dev profiles; the user picks the right one.
        binary: "firefox",
        profile_roots_rel: &["Firefox"],
    },
    KnownBrowser {
        id: "firefox-nightly",
        display_name: "Firefox Nightly",
        kind: BrowserKind::Firefox,
        bundle_id: "org.mozilla.nightly",
        app_dir_names: &["Firefox Nightly.app"],
        binary: "firefox",
        profile_roots_rel: &["Firefox"],
    },
    // -------- productivity / opinionated Chromium browsers --------
    // Smaller distribution than mainstream channels but a clearly
    // different audience (Arc-adjacent productivity surfaces). Bundle
    // ids and profile roots are heuristic — verified against vendor
    // documentation where possible; Info.plist's CFBundleIdentifier
    // still wins at runtime so a vendor rename only loses one boot
    // cycle's worth of detection rather than breaking permanently.
    KnownBrowser {
        id: "sigmaos",
        display_name: "SigmaOS",
        kind: BrowserKind::Chromium,
        bundle_id: "app.sigmaos.SigmaOS",
        app_dir_names: &["SigmaOS.app"],
        binary: "SigmaOS",
        profile_roots_rel: &["SigmaOS"],
    },
    KnownBrowser {
        id: "sidekick",
        display_name: "Sidekick",
        kind: BrowserKind::Chromium,
        bundle_id: "com.pushplaylabs.sidekick",
        app_dir_names: &["Sidekick.app"],
        binary: "Sidekick",
        profile_roots_rel: &["Sidekick"],
    },
    KnownBrowser {
        id: "wavebox",
        display_name: "Wavebox",
        kind: BrowserKind::Chromium,
        bundle_id: "io.wavebox.WaveboxApp",
        app_dir_names: &["Wavebox.app"],
        binary: "Wavebox",
        // Wavebox stores Chromium User Data under WaveboxApp; older
        // builds shipped under "Wavebox" — try both before giving up.
        profile_roots_rel: &["WaveboxApp", "Wavebox"],
    },
    KnownBrowser {
        id: "stack",
        display_name: "Stack",
        kind: BrowserKind::Chromium,
        bundle_id: "com.getstack.Stack",
        app_dir_names: &["Stack.app"],
        binary: "Stack",
        profile_roots_rel: &["Stack"],
    },
    KnownBrowser {
        id: "min",
        display_name: "Min",
        kind: BrowserKind::Chromium,
        bundle_id: "com.electron.min",
        app_dir_names: &["Min.app"],
        binary: "Min",
        // Min is Electron-based with no Chromium User Data layout we
        // can parse; profile listing therefore returns empty and the
        // user routes by browser id alone.
        profile_roots_rel: &[],
    },
    KnownBrowser {
        id: "ulaa",
        display_name: "Ulaa",
        kind: BrowserKind::Chromium,
        bundle_id: "com.zohocorp.Ulaa",
        app_dir_names: &["Ulaa.app"],
        binary: "Ulaa",
        profile_roots_rel: &["Ulaa", "Ulaa/User Data"],
    },
    KnownBrowser {
        id: "beam",
        display_name: "Beam",
        kind: BrowserKind::Chromium,
        bundle_id: "co.beamapp.macos",
        app_dir_names: &["Beam.app"],
        binary: "Beam",
        // Beam keeps its session store in a custom path (not Chromium
        // User Data); list profiles falls through to empty.
        profile_roots_rel: &[],
    },
];

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join("Applications"));
    }
    dirs
}

fn application_support_root() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support"))
}

fn find_app_path(entry: &KnownBrowser) -> Option<PathBuf> {
    for base in search_dirs() {
        for app_dir in entry.app_dir_names {
            let path = base.join(app_dir);
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

fn read_info_plist_key(app_path: &Path, key: &str) -> Option<String> {
    let info_plist = app_path.join("Contents").join("Info.plist");
    let out = Command::new("/usr/bin/plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(info_plist)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn executable_path(app_path: &Path, entry: &KnownBrowser) -> PathBuf {
    let binary =
        read_info_plist_key(app_path, "CFBundleExecutable").unwrap_or_else(|| entry.binary.into());
    app_path.join("Contents").join("MacOS").join(binary)
}

fn bundle_id(app_path: &Path, entry: &KnownBrowser) -> String {
    read_info_plist_key(app_path, "CFBundleIdentifier").unwrap_or_else(|| entry.bundle_id.into())
}

fn profile_root(entry: &KnownBrowser, asroot: Option<&Path>) -> Option<PathBuf> {
    let asroot = asroot?;
    let mut candidates = entry
        .profile_roots_rel
        .iter()
        .map(|rel| asroot.join(rel))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    candidates
        .iter()
        .find(|path| path.exists())
        .cloned()
        .or_else(|| candidates.drain(..1).next())
}

fn lookup(id: &BrowserId) -> Option<&'static KnownBrowser> {
    REGISTRY.iter().find(|e| e.id == id.0.as_str())
}

impl BrowserInventory for MacInventory {
    fn installed_browsers(&self) -> Result<Vec<InstalledBrowser>> {
        let asroot = application_support_root();
        let mut out = Vec::new();
        for entry in REGISTRY {
            let Some(app_path) = find_app_path(entry) else {
                continue;
            };
            out.push(InstalledBrowser {
                id: BrowserId::new(entry.id),
                display_name: entry.display_name.to_string(),
                kind: entry.kind,
                executable: executable_path(&app_path, entry),
                platform_app_id: Some(bundle_id(&app_path, entry)),
                profile_root: profile_root(entry, asroot.as_deref()),
            });
        }
        Ok(out)
    }

    fn profiles(&self, browser: &BrowserId) -> Result<Vec<BrowserProfile>> {
        let Some(entry) = lookup(browser) else {
            return Ok(Vec::new());
        };
        let Some(root) = profile_root(entry, application_support_root().as_deref()) else {
            return Ok(Vec::new());
        };
        if !root.exists() {
            return Ok(Vec::new());
        }
        match entry.kind {
            BrowserKind::Chromium | BrowserKind::Arc => {
                parse_chromium_profiles(&root).map_err(PlatformError::Io)
            }
            BrowserKind::Firefox => {
                let ini = root.join("profiles.ini");
                if !ini.exists() {
                    return Ok(Vec::new());
                }
                parse_firefox_profiles(&ini).map_err(PlatformError::Io)
            }
            BrowserKind::Safari | BrowserKind::Unknown => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn registry_ids_are_unique() {
        let mut seen = BTreeSet::new();
        for entry in REGISTRY {
            assert!(seen.insert(entry.id), "duplicate browser id: {}", entry.id);
        }
    }

    #[test]
    fn registry_includes_expanded_browser_set() {
        let ids = REGISTRY
            .iter()
            .map(|entry| entry.id)
            .collect::<BTreeSet<_>>();
        for expected in [
            "vivaldi",
            "opera",
            "opera-gx",
            "dia",
            "atlas",
            "comet",
            "zen",
            "orion",
            "duckduckgo",
            "librewolf",
            "waterfox",
            "floorp",
            "mullvad-browser",
            "tor-browser",
            "yandex",
            "whale",
        ] {
            assert!(ids.contains(expected), "missing browser id: {expected}");
        }
    }
}
