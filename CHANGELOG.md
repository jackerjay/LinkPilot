# Changelog

All notable changes to LinkPilot are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
  `picker-style {frosted|bezel|crown}` subcommands.

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

[Unreleased]: https://github.com/jackerjay/LinkPilot/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/jackerjay/LinkPilot/releases/tag/v0.2.0
