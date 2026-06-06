# External Integrations

**Analysis Date:** 2026-06-05

## macOS OS-Level Integrations

**LaunchServices — Default Browser Registration:**
- API: `LSSetDefaultHandlerForURLScheme` / `LSCopyDefaultHandlerForURLScheme` (ApplicationServices framework)
- Implementation: `crates/platform-mac/src/default_browser.rs`
- Purpose: Register/query LinkPilot as the system handler for `http` and `https` schemes
- Note: API is deprecated in macOS 12 but still functional on macOS 14/15; async NSWorkspace replacement requires signed bundle

**LaunchServices — Browser Discovery:**
- API: filesystem scan of `/Applications` and `~/Applications` + `NSWorkspace`/`objc2-app-kit`
- Implementation: `crates/platform-mac/src/inventory.rs`
- Purpose: Enumerate installed browsers (Chrome, Edge, Brave, Firefox, Arc, Safari, Opera, Vivaldi, Zen, Orion) and their profiles
- Supports: Chromium profile dirs (`~/Library/Application Support/Google/Chrome`), Firefox profile dirs

**LaunchServices — URL/App Launch:**
- API: `open -b <bundle_id>` (subprocess), direct binary exec for Chromium/Firefox, `open -a <name>` for Arc/Safari
- Implementation: `crates/platform-mac/src/launcher.rs`
- Purpose: Open a URL in a specific browser + profile after routing decision

**LaunchAgents — Daemon Autostart:**
- Path: `~/Library/LaunchAgents/app.linkpilot.daemon.plist`
- Implementation: `crates/platform-mac/src/launch_agent.rs`, `crates/platform-mac/src/autostart.rs`
- Purpose: Register `linkpilot-daemon` with launchd so it starts at login and stays resident
- Managed by: GUI first-run auto-install (`apps/desktop/src-tauri/src/lib.rs`), `lpt daemon install/uninstall`, Settings toggle

**launchctl — Daemon Lifecycle:**
- API: `launchctl load -w <plist>`, `launchctl list <label>`, `launchctl unload -w <plist>`
- Implementation: `crates/platform-mac/src/launch_agent.rs`
- Purpose: Load, query (PID / loaded state), and unload the daemon service

**Gatekeeper / Quarantine:**
- Issue: App is unsigned/unnotarized → macOS quarantine flag blocks first launch
- Mitigation: Homebrew cask postflight runs `xattr -dr com.apple.quarantine LinkPilot.app` — see `packaging/homebrew/Casks/linkpilot.rb`

**Info.plist Patching — Default Browser Registration:**
- Script: `apps/desktop/scripts/patch-info-plist.sh`
- Purpose: Rewrites `CFBundleURLTypes` (Viewer role, `LSHandlerRank=Default`), adds `CFBundleDocumentTypes` for HTML/XHTML, adds `NSUserActivityTypes: NSUserActivityTypeBrowsingWeb`, sets `LSUIElement=true`
- Why needed: `tauri-plugin-deep-link` injects an Editor-role entry that macOS's default browser picker ignores
- Run order: After `tauri build --bundles app`, before DMG creation (or it gets clobbered)

**AppKit (objc2 / window-vibrancy):**
- Implementation: `apps/desktop/src-tauri/src/picker.rs`, `apps/desktop/src-tauri/src/tray.rs`
- Purpose: Frosted-glass vibrancy for picker window (`window-vibrancy`); raw NSWindow messages to set collection behavior (float over full-screen Spaces) and window level above floating apps

**NSWorkspace / NSRunningApplication:**
- Library: `objc2-app-kit` 0.2 (`NSWorkspace`, `NSRunningApplication` features)
- Implementation: `crates/platform-mac/src/opener.rs`, `crates/platform-mac/src/inventory.rs`
- Purpose: Detect which app opened a URL (opener detection), app activation after URL launch

## IPC — Daemon ↔ Clients

**Transport:**
- macOS/Linux: Unix Domain Socket at `~/Library/Application Support/LinkPilot/linkpilot.sock`
- Windows (planned, NYI): Named Pipe
- Socket path resolution: `crates/ipc/src/path.rs` → `linkpilot_ipc::path::default_endpoint()`

**Wire Format:**
- Length-prefixed JSON: 4-byte big-endian `u32` length followed by UTF-8 JSON payload
- Max frame: 4 MB (`MAX_FRAME_BYTES = 4 * 1024 * 1024` in `crates/ipc/src/transport.rs`)
- Protocol version: `PROTOCOL_VERSION = 2` (defined in `crates/core/src/protocol.rs`)

**Message Types** (`crates/core/src/protocol.rs`):

| Request | Response | Purpose |
|---------|----------|---------|
| `RouteEvaluate` | `RouteDecision` | Evaluate URL without side effects |
| `RouteOpen` | `RouteDecision` | Evaluate AND launch target browser |
| `ConfigGet` | `ConfigSnapshot` | Fetch current config document |
| `ConfigReplace` | `Ack` | Atomically replace config |
| `Doctor` | `DoctorReport` | Health check (default browser, socket, browser count) |
| `StatePing` | `Pong` | Liveness probe (returns daemon version) |
| `RouteHistory` | `RouteHistorySnapshot` | Newest-first route log (limit cap) |

**IPC Server** (`crates/ipc/src/server.rs`):
- Hosted either by `linkpilot-daemon` binary or by the Tauri desktop app (in-process mode)
- Returns `Error { code: "unknown-verb" }` for unrecognised request types (v2+ protocol)

**IPC Client** (`crates/ipc/src/client.rs`):
- Used by `lpt` CLI (`crates/cli/`) and by the desktop GUI for external daemon detection

**Daemon Coexistence:**
- On startup the GUI probes the socket with `StatePing`
- If a running daemon is found: GUI runs in "client mode" (skips IPC server bind)
- If no daemon: GUI hosts the daemon in-process
- Logic: `apps/desktop/src-tauri/src/lib.rs` (`probe_existing_daemon`, `DaemonMode`)

## Native Messaging Host (NMH) — Browser Extension Bridge

**Status:** Placeholder in v0.1; real bridge ships in v0.3
- Binary: `crates/native-host/src/main.rs` → `linkpilot-native-host`
- Protocol: Browser NMH stdio format (4-byte length prefix, JSON) bridged to Unix socket
- Purpose: Manifest V3 browser extension (`apps/extension/`) sends URLs to the daemon for re-routing
- Target browsers: Chrome, Arc, Edge (v0.3), Firefox (v0.4)

## Config File on Disk

**Location (macOS):** `~/Library/Application Support/LinkPilot/linkpilot.config.json`
**Location (Linux):** `$XDG_CONFIG_HOME/linkpilot/linkpilot.config.json`
**Location (Windows):** `%APPDATA%\LinkPilot\linkpilot.config.json`

**Write discipline:**
- Atomic rewrite (temp file + rename) via `ConfigStore` in `crates/core/src/config/store.rs`
- Anti-echo token (UUID per write): prevents fsnotify from re-broadcasting the writer's own change
- `WriterId` enum (`Gui`, `Cli`, `External`) distinguishes origin in `Event::ConfigChanged`

**Live reload:**
- `notify` 6 filesystem watcher (`crates/core/src/config/store.rs`, `ConfigStore::watch`)
- Tauri desktop app emits `config-changed` event to frontend on every external or GUI write
- Frontend subscribes via `onConfigChanged` in `apps/desktop/src/lib/ipc.ts`

**Observations log (behavior log):**
- Path: `~/Library/Application Support/LinkPilot/observations.ndjson`
- Dismissed log: `~/Library/Application Support/LinkPilot/observations-dismissed.json`
- Managed by: `linkpilot_core::observations::ObservationsStore`

## Tauri Commands — JS ↔ Rust Bridge

**Boundary:**
- Frontend invokes Rust via `invoke()` from `@tauri-apps/api/core`
- Typed wrappers: `apps/desktop/src/lib/ipc.ts`
- Rust handlers: `apps/desktop/src-tauri/src/commands/mod.rs`
- Registration: `apps/desktop/src-tauri/src/lib.rs` (`invoke_handler!` macro)

**Key command groups:**
- Config: `config_get`, `config_replace`, `rule_upsert`, `rule_delete`, `workspace_*`
- Routing: `route_open`, `route_evaluate`, `route_history`
- Browsers: `list_browsers`, `add_custom_browser`, `browser_set_enabled`, `list_profiles`
- Platform: `is_default_browser`, `request_set_default_browser`, `daemon_service_*`, `cli_install_*`
- Updates: `update_fetch_metadata`, `update_download` (self-update via GitHub Releases API)

**Events (Rust → Frontend):**
- `config-changed` — broadcast on every config write (origin label: `"external"` or `"echo"`)
- `route-logged` — broadcast after each routing decision; payload is `RouteRecord`

## Self-Update

**Mechanism:** `update_fetch_metadata` command hits the GitHub Releases API to check for newer versions; `update_download` downloads and verifies a DMG by SHA-256 against `checksums.txt`
- Implementation: `apps/desktop/src-tauri/src/commands/` + `apps/desktop/src/lib/update.ts`
- Auth: No authentication — uses public GitHub Releases API

## Homebrew Distribution

**Custom tap:** `jackerjay/homebrew-linkpilot` (must be created by maintainer)

**Cask — GUI bundle:**
- File: `packaging/homebrew/Casks/linkpilot.rb`
- Install: `brew install --cask jackerjay/linkpilot/linkpilot`
- Source: Per-arch DMG from GitHub Releases (`LinkPilot_<version>_aarch64.dmg` / `..._x86_64.dmg`)
- Quarantine handling: `postflight` strips `com.apple.quarantine` xattr automatically
- Uninstall: stops `app.linkpilot.daemon` launchd service, removes LaunchAgent plist
- Zap (deep clean): removes `~/Library/Application Support/LinkPilot`, caches, preferences, saved state

**Formula — CLI-only:**
- File: `packaging/homebrew/Formula/linkpilot-cli.rb`
- Install: `brew install jackerjay/linkpilot/linkpilot-cli`
- Ships two binaries: `lpt` and `linkpilot-daemon` (from per-arch tarballs on GitHub Releases)
- No quarantine issue (Homebrew formula downloads don't quarantine)

**Version bumping:** `release_tag` local variable in formula must be updated manually on each release; `version` stanza sets the displayed version

## npm Package — `@linkpilot/config` DSL

**Package:** `@linkpilot/config` (published to npmjs.org under `@linkpilot` scope)
- Source: `packages/config-dsl/`
- Purpose: TypeScript DSL for authoring `linkpilot.config.ts` — compiles to the JSON the daemon reads
- Build tool: `tsc` (pure TypeScript, ESM output)
- Exports: `dist/index.js` + `dist/index.d.ts`

**Publishing:**
- Triggered by `v*.*.*` git tags (same trigger as `release.yml`)
- Workflow: `.github/workflows/npm-publish.yml`
- Version stamp: derived from git tag (`npm version $TAG_VERSION`), not from committed `package.json`
- Stable releases: published as `latest` dist-tag
- Pre-releases (tags with `-`): published as `next` dist-tag
- Auth: `NPM_TOKEN` GitHub repo secret

## GitHub Actions CI/CD

**`ci.yml`** — triggers on every PR and `main` push:

| Job | Runner | What it does |
|-----|--------|-------------|
| `rust` | `macos-latest` | `cargo fmt --check` + `clippy -D warnings` + `cargo test` + `cargo check -p linkpilot-desktop` |
| `frontend` | `ubuntu-latest` | `tsc --noEmit` + `vite build` |
| `desktop-bundle` | `macos-latest` | `tauri build --debug --bundles app` smoke test |

- Rust cache: `Swatinem/rust-cache@v2`
- Node setup: `actions/setup-node@v6`

**`release.yml`** — triggers on `v*.*.*` tags:

- Matrix: `aarch64` on `macos-14` (Apple Silicon runner) + `x86_64` on `macos-15-intel` (Intel runner; `macos-13` retired 2025-12)
- Each leg builds: `lpt` CLI + `linkpilot-daemon` + Tauri `.app`, embeds `lpt` and `linkpilot-daemon` into `LinkPilot.app/Contents/MacOS/`, patches `Info.plist`, creates per-arch DMG via `create-dmg` (brew-installed in CI)
- Artifacts: `LinkPilot_<v>_<arch>.dmg`, `lpt-macos-<arch>` + `.tar.gz`, `linkpilot-daemon-macos-<arch>` + `.tar.gz`, per-arch `checksums-<arch>.txt`
- `publish` job (ubuntu, no checkout): downloads both arch bundles, merges to `dist/release/`, regenerates unified `checksums.txt`, creates draft GitHub Release (via `softprops/action-gh-release@v3`), publishes release (via `gh release edit`)
- Release is immutable once published (GitHub immutable releases setting)
- Releases are unsigned/unnotarized

**`npm-publish.yml`** — triggers on `v*.*.*` tags:
- Publishes `@linkpilot/config` to npm from `packages/config-dsl/`
- Uses bun 1.3 (`oven-sh/setup-bun@v2`) to install deps and run tests before publishing
- Requires `NPM_TOKEN` repo secret

## Data Storage

**Databases:** None — config is stored as a single JSON file on disk

**File Storage:** Local filesystem only
- Config: `~/Library/Application Support/LinkPilot/linkpilot.config.json`
- Observations: `~/Library/Application Support/LinkPilot/observations.ndjson`
- Dismissed observations: `~/Library/Application Support/LinkPilot/observations-dismissed.json`
- IPC socket: `~/Library/Application Support/LinkPilot/linkpilot.sock`
- LaunchAgent plist: `~/Library/LaunchAgents/app.linkpilot.daemon.plist`
- Logs (daemon): `~/Library/Logs/LinkPilot/` (created by `launch_agent.rs`)

**Caching:** None — no in-memory cache beyond the `RouteHistory` ring buffer (100 records default, daemon process memory only, not persisted across restarts)

## Authentication & Identity

**Auth Provider:** None — local app only, no user accounts or external auth

## Monitoring & Observability

**Error Tracking:** None — no external service

**Logs:**
- Rust: `tracing` + `tracing-subscriber` with `env-filter`; written to stderr in all binaries
- Daemon: stderr redirected to `~/Library/Logs/LinkPilot/daemon.out.log` / `daemon.err.log` via LaunchAgent `StandardOutPath` / `StandardErrorPath`
- Log level controlled by `RUST_LOG` env var

## Environment Configuration

**Required env vars (production):** None — app is self-contained

**Optional env vars:**
- `RUST_LOG` — tracing filter (e.g. `info,linkpilot=debug`)
- `HOME` — required by config path resolution on macOS/Linux
- `XDG_CONFIG_HOME` — Linux config path override
- `APPDATA` — Windows config path

**Secrets (CI only):**
- `NPM_TOKEN` — npm publish scope for `@linkpilot` org (`npm-publish.yml`)
- `GITHUB_TOKEN` — built-in; used by `gh release edit` in `release.yml` publish job

---

*Integration audit: 2026-06-05*
