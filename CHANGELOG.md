# Changelog

All notable changes to LinkPilot are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Copy URL from the ask picker.** The URL row now carries two inline
  icon buttons — copy, and copy-and-dismiss — and the same two actions
  are bound to `C` (copy, picker stays open) and `X` (cut: copy then
  close the picker). Both shortcuts ignore Cmd/Ctrl/Alt/Shift modifiers
  and work whether the Halo wheel is open or not. The copy button flips
  to a check briefly on success; the cut button closes the picker as
  its own confirmation. Footer hint row lists both shortcuts. Localized
  across en / zh-CN / zh-TW / ja-JP.

## [0.4.2] — 2026-05-27

### Changed

- **New brand logo.** Refreshed the app icon — a rounded "p" carrying a
  paper-plane *send* mark — across every bundle target (Dock / Launchpad,
  Windows, iOS, Android) and the website favicon, regenerated from a new
  source master through `tools/icon-padder` + `tauri icon`. The macOS
  menu-bar tray template was redrawn to echo the mark (a "p" ring with a
  paper-plane dart) while staying single-colour for `templateImage`
  tinting.

## [0.4.1] — 2026-05-25

### Added

- **Per-browser hide-from-picker toggle.** Each row on the Browsers page now
  has an on/off switch that adds the browser id to
  `Settings.disabled_browsers`. Hidden browsers no longer appear in the ask
  popup picker (and the Settings → preview picker, for parity); they stay
  installed and remain valid as explicit routing targets — a rule that names
  a hidden browser still opens links there. `lpt browsers enable <id>` /
  `lpt browsers disable <id>` mirror the toggle from the CLI.

### Fixed

- **Update check bypasses `api.github.com` rate limits.** The "Check for
  updates" path hit `api.github.com/.../releases/latest`, which returns 403
  from many corporate / data-center egress IPs. The desktop app now resolves
  the latest release via the `github.com/.../releases/latest` redirect and
  synthesizes asset URLs from the release workflow's fixed naming
  convention, so the update check works from networks the API host
  throttles. `release_name` and `published_at` are backfilled from the
  public `releases.atom` feed.

## [0.4.0] — 2026-05-22

### Added

- **Expanded macOS browser inventory.** LinkPilot now auto-detects Vivaldi,
  Opera, Opera GX, Dia, ChatGPT Atlas, Perplexity Comet, Zen Browser,
  Orion, DuckDuckGo Browser, LibreWolf, Waterfox, Floorp, Mullvad Browser,
  Tor Browser, Yandex Browser, and Naver Whale. Chromium and Firefox-family
  additions reuse the existing profile parsers when their profile stores are
  available.
- **Channel variants and productivity browsers.** Inventory also picks up
  Safari Technology Preview, Chrome Beta/Dev/Canary, Edge Beta/Dev/Canary,
  Brave Beta/Nightly, Firefox Developer Edition/Nightly, and the
  productivity-Chromium cohort (SigmaOS, Sidekick, Wavebox, Stack, Min,
  Ulaa, Beam). Each channel keeps its own Chromium profile root, so
  routing to `chrome-canary` won't open a stable Chrome profile.
- **Expanded config DSL browser shortcuts.** `@linkpilot/config` now exposes
  shortcuts for the newly-supported browser ids, including `browser.atlas`,
  `browser.comet`, `browser.dia`, `browser.opera`, and `browser.zen`.
- **Default-target setup prompt.** The Overview page now guides users to pick
  a usable default target when the config still has the unconfigured `system`
  target or points at a browser that is not detected on the current Mac.
- **Onboarding wizard now has five steps.** A dedicated "pick your default
  browser" step lands between the browser scan and the rule templates, so
  first-run users explicitly choose where unmatched links go before they
  opt into starter rules. The Step 2 illustration also gets a refresh —
  the abstract URL token and Compass placeholders become two paired icon
  stacks (URL sources on the left, real installed browsers on the right)
  joined by horizontal arrows.
- **Syntax-highlighted JSON editor.** Rules → Advanced → "raw JSON" now
  uses CodeMirror 6 with `@codemirror/lang-json`, `jsonParseLinter`, and
  a lint gutter. Errors surface inline (red underline + gutter mark) and
  textually ("Line X, col Y: …"); Save disables while parsing fails.
  The editor is lazy-imported so the ~140 KB gzipped chunk only loads
  when the panel is expanded.
- **Adaptive starter templates.** The onboarding wizard's rule templates
  now resolve their target browser dynamically from a priority list
  against the detected inventory. "Work → Chrome / Work" becomes
  "Work → Edge / Work" on a Mac without Chrome; templates whose
  candidate list has no installed match are hidden instead of creating
  rules that point at non-existent browsers.

### Changed

- **Fresh configs start without an implicit browser default.** The bundled demo
  rules still demonstrate Chrome/Arc routing, but the fallback target now stays
  unconfigured until the user chooses one.
- **Update check moves to the Rust side.** Both the GitHub Releases API call
  and the `checksums.txt` fetch now run via `/usr/bin/curl` from a Tauri
  command instead of from the renderer. This sidesteps the CORS rejection
  that fired in `tauri dev` (origin `http://localhost:5173`) and keeps the
  fetch outside of any future WebView CSP tightening, with the
  `UpdateCheckResult` shape preserved for the renderer.

### Fixed

- **Unconfigured or missing default targets fall back to Ask.** When no rule
  matches and the default target is still `system`, routing returns Ask instead
  of trying to launch a fake browser id. If a rule/default points at a browser
  LinkPilot cannot detect, the desktop launcher falls back to the Ask picker
  instead of failing the open.
- **Onboarding `tmpl-work` no longer creates a dead rule.** Its target was
  hard-coded to a `google-chrome` id that never existed in the inventory
  (correct id is `chrome`); enabling the template silently produced a rule
  pointing at a phantom browser. The dynamic-template work above fixes this
  by walking a Chromium-family candidate list.

## [0.3.0] — 2026-05-21

### Added

- **Localized UI.** LinkPilot now ships English, Simplified Chinese,
  Traditional Chinese, and Japanese resources across the main app,
  menu-bar tray, picker window, onboarding, rules, workspaces, inspector,
  browser manager, test URL simulator, and settings.
- **Language preference.** Settings → Appearance includes a language
  selector with `system`, `en`, `zh-CN`, `zh-TW`, and `ja-JP`. The same
  preference is available through `lpt settings language`.
- **Localized README files.** Traditional Chinese and Japanese README
  files now sit beside the existing English and Simplified Chinese docs,
  with cross-links between all supported languages.
- **Automatic update downloads.** The desktop app now checks GitHub Releases
  on startup (toggle: Settings → General → Updates) and, when a newer macOS
  DMG is available, downloads it to the local update cache. A primary
  "Open installer" button appears in Settings once the download finishes.
- **Picker style chooser.** Three Halo variants — `frosted`, `bezel`, and
  `crown` — can be selected in Settings → Appearance, with a live preview
  per style. The picker window reads the chosen style at open time.
  Also exposed via `lpt settings picker-style {frosted|bezel|crown}`.
- **Profile order editor.** Settings → Appearance now includes a per-browser
  profile-order editor. Drag profiles in the Halo wheel to reorder, remove
  the active sector to hide a profile, or use the `+` slot to add a
  detected-but-hidden profile. The saved order drives Halo positions
  (which in turn map to keyboard shortcuts 1–9).
- **Unplaced-profile signal.** When the browser exposes a new profile
  (e.g. you create a new Chrome profile after customizing the Halo
  ordering), Settings shows a banner and a per-card `+N new` chip so the
  newly-detected profile is never silently hidden.
- **Source-app bundle-id matching.** Rules authored from the Test URL flow
  carry a `bundle_id`, and the matcher now prefers it over the localized
  display name — "Lark" and "飞书" both resolve to `com.electron.lark`.
  When the routing context has no bundle id (e.g. the Test URL simulator)
  the matcher falls back to name matching so simulated runs still hit.
- **Profile accent colors.** Each Chromium and Firefox profile gets a
  deterministic palette color derived from its id (stable across Rust
  toolchain upgrades; never repaints on relaunch).
- **`picker_preview` IPC command** — opens the picker against your real
  browser inventory with a test URL, so you can verify focus handoff and
  profile routing before relying on a real Ask rule.

### Changed

- **Picker window geometry** grew from 560×280 to 720×520 to host the Halo
  wheel's portaled paint zone. The window stays fully transparent; only
  the central popover is visible.
- **Settings → Appearance** consolidates theme, picker style, and profile
  order. The Updates section moved to Settings → General.
- **`lpt settings`** gains `auto-updates {on|off}` and
  `picker-style {frosted|bezel|crown}` subcommands, plus
  `language {system|en|zh-CN|zh-TW|ja-JP}`.

### Security

- **Update downloads now require a SHA-256 match.** The renderer fetches
  the release's `checksums.txt`, extracts the SHA-256 for the macOS DMG,
  and passes it to the native side. The daemon verifies the freshly
  downloaded file before atomically renaming it into the updates dir; on
  mismatch the temporary file is deleted and the previous good
  download (if any) stays untouched. Releases without `checksums.txt`
  refuse to auto-download — the user can still grab the installer
  manually from the release page.
- **Update downloads are pinned to the official release path** —
  `github.com/jackerjay/LinkPilot/releases/download/`. Earlier builds
  only checked `host == github.com`, which would have allowed forks.
- **Picker source-app matching no longer falls through to name match
  when both rule and context provide a bundle id.** A rogue app
  impersonating Slack's name can't cross a strict bundle-id mismatch
  to win the rule.

### Fixed

- **Concurrent Ask flows no longer race.** When two URL opens trigger
  Ask back-to-back (or a `picker_preview` collides with a real Ask),
  `show_picker` now atomically claims the picker slot. The losing
  caller cancels cleanly instead of silently corrupting the first
  caller's pending channel.
- **Picker / tray theme regressions.** Picker and tray follow system
  appearance again; only the main window applies the user's persisted
  theme override.
- **Picker language sync.** The independent picker window now reads the
  persisted language preference before mounting, so it no longer falls
  back to the default detected language or briefly flashes English.
- **Tray language sync and layout.** The tray popover and native tray menu
  now apply the saved language preference. The tray status line no longer
  wraps awkwardly under Chinese text, and sidebar menu items have slightly
  more vertical breathing room for localized labels.
- **Atomic update rename.** The destination is no longer removed
  before `rename(2)` — POSIX rename atomically replaces, so the
  pre-delete only opened a window where the file briefly didn't exist.
- **`set_profile_order` dedup.** Duplicate ids in a saved order are
  now removed before persisting (kept first-seen order).
- **`crate-type`** restored to `["staticlib", "cdylib", "rlib"]`.

### Performance

- **Ask hot path no longer deep-clones the full `ConfigDocument`.**
  `ConfigStore::with_document` lets dispatch and the picker project
  out just the small fields they need (`default_target`,
  `picker_style`, `profile_orders`) under the store mutex. For users
  with many rules and high Ask volume this cuts both allocation cost
  and mutex hold time.

## [0.2.0] — 2026-05-XX

Initial public release. macOS desktop app, `lpt` CLI, background daemon,
rule-based routing, Cmd-Tab-style Ask picker, browser inventory, and
config DSL.

[Unreleased]: https://github.com/jackerjay/LinkPilot/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/jackerjay/LinkPilot/releases/tag/v0.3.0
[0.2.0]: https://github.com/jackerjay/LinkPilot/releases/tag/v0.2.0
