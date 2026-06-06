# Codebase Structure

**Analysis Date:** 2026-06-05

## Directory Layout

```
linkpilot/                          # Cargo workspace root
├── Cargo.toml                      # Workspace manifest (version 0.5.6, edition 2021)
├── Cargo.lock
├── CLAUDE.md                       # Project-level Claude coding rules
├── AGENTS.md                       # Extended project notes for AI agents
├── CHANGELOG.md                    # Release history (bump this on every release)
│
├── crates/                         # Pure-Rust library and binary crates
│   ├── core/                       # Platform-agnostic routing engine + data model
│   │   ├── src/
│   │   │   ├── lib.rs              # Public re-exports
│   │   │   ├── routing.rs          # Router, RoutingContext, RoutingDecision, MatcherEval
│   │   │   ├── rules.rs            # MatcherTree, Action, Rule, RuleId
│   │   │   ├── config/
│   │   │   │   ├── mod.rs          # ConfigDocument, Settings, Workspace, PickerStyle schema
│   │   │   │   └── store.rs        # ConfigStore (load/save/watch, anti-echo token)
│   │   │   ├── daemon.rs           # DaemonRuntime, RequestHandler trait, PID file utils
│   │   │   ├── history.rs          # RouteHistory ring buffer, RouteRecord
│   │   │   ├── protocol.rs         # IPC Request / Response / Event (wire types)
│   │   │   ├── platform.rs         # PlatformProvider trait + StubProvider
│   │   │   ├── browser.rs          # BrowserId, BrowserTarget, InstalledBrowser, BrowserProfile
│   │   │   ├── endpoint.rs         # Endpoint type + default socket path resolution
│   │   │   ├── inventory.rs        # Shared browser inventory helpers
│   │   │   └── observations.rs     # ObservationsStore (behavior log → rule suggestions)
│   │   └── tests/
│   │       └── dsl_roundtrip.rs    # Serde round-trip integration test
│   │
│   ├── platform-mac/               # Real macOS backend (macOS only, #[cfg(target_os = "macos")])
│   │   └── src/
│   │       ├── lib.rs              # MacProvider struct + PlatformProvider impl
│   │       ├── inventory.rs        # LaunchServices browser discovery
│   │       ├── launcher.rs         # NSWorkspace URL open + profile args
│   │       ├── default_browser.rs  # LSCopyDefaultHandlerForURLScheme / set
│   │       ├── autostart.rs        # LaunchAgent plist for open-at-login
│   │       ├── launch_agent.rs     # Daemon LaunchAgent install/uninstall/status
│   │       ├── opener.rs           # NSWorkspace frontmost-app detector
│   │       ├── notifier.rs         # NSUserNotification (macOS toast)
│   │       ├── app_icon.rs         # .icns → PNG extraction + disk cache
│   │       ├── app_picker.rs       # NSOpenPanel "Choose Application" dialog
│   │       └── prompt.rs           # macOS permission prompt helpers
│   │
│   ├── platform-win/               # Windows stub (re-exports StubProvider)
│   │   └── src/lib.rs
│   ├── platform-linux/             # Linux stub (re-exports StubProvider)
│   │   └── src/lib.rs
│   │
│   ├── ipc/                        # Length-prefixed JSON over Unix socket / Named pipe
│   │   ├── src/
│   │   │   ├── lib.rs              # Re-exports protocol types from core
│   │   │   ├── server.rs           # Tokio async IPC listener, serve()
│   │   │   ├── client.rs           # Blocking client::send()
│   │   │   ├── transport.rs        # read_raw_frame / write_frame (u32 BE + JSON)
│   │   │   └── path.rs             # default_endpoint() → Endpoint::UnixSocket
│   │   └── tests/
│   │       ├── route_history_roundtrip.rs
│   │       ├── stale_socket_cleanup.rs
│   │       └── unknown_verb_fallback.rs
│   │
│   ├── native-host/                # Native Messaging Host stdio bridge (v0.3 placeholder)
│   │   └── src/main.rs
│   │
│   ├── cli/                        # `lpt` command-line client binary
│   │   ├── src/main.rs             # All clap subcommands in one file (open, doctor, rules, etc.)
│   │   └── tests/
│   │       ├── daemon_subcommand_smoke.rs
│   │       └── history_subcommand_smoke.rs
│   │
│   └── headless-daemon/            # Reserved for future GUI-less daemon binary
│       └── src/main.rs
│
├── apps/
│   └── desktop/                    # Tauri desktop app
│       ├── package.json            # npm manifest — bump version on every release
│       ├── index.html              # Vite entry HTML
│       ├── src/                    # React + TypeScript frontend
│       │   ├── main.tsx            # React root mount
│       │   ├── App.tsx             # Router / top-level layout
│       │   ├── lib/
│       │   │   ├── ipc.ts          # All typed Tauri command wrappers (THE frontend API layer)
│       │   │   ├── types.ts        # Shared TypeScript types (mirrors Rust structs)
│       │   │   ├── browsers.ts     # Browser-specific display helpers
│       │   │   ├── theme.ts        # Theme utilities
│       │   │   ├── update.ts       # Update check helpers
│       │   │   └── utils.ts        # General utilities (cn, etc.)
│       │   ├── pages/
│       │   │   ├── rules.tsx       # Rules list + editor (main config page)
│       │   │   ├── inspector.tsx   # Route history + explain-why inspector
│       │   │   ├── settings.tsx    # Settings page (launch-at-login, smart routing, etc.)
│       │   │   ├── browsers.tsx    # Browser inventory + custom browser management
│       │   │   ├── workspace.tsx   # Workspace groups management
│       │   │   ├── menu-bar.tsx    # Tray popover window content
│       │   │   └── test-url.tsx    # Test URL simulation panel
│       │   ├── components/
│       │   │   ├── RuleEditor.tsx  # Structured rule condition + action editor
│       │   │   ├── Explanation.tsx # MatcherEval tree renderer (Inspector)
│       │   │   ├── TargetEditor.tsx
│       │   │   ├── WhenDisplay.tsx
│       │   │   ├── BrowserBadge.tsx
│       │   │   ├── AppPickerButton.tsx  # Native "Choose App" dialog trigger
│       │   │   ├── AppIcon.tsx
│       │   │   ├── SuggestionsPanel.tsx
│       │   │   ├── AdvancedJsonEditor.tsx  # Raw JSON escape hatch for config
│       │   │   └── ui/             # Radix UI primitives (button, input, select, etc.)
│       │   ├── picker/
│       │   │   ├── PickerWindow.tsx        # Ask-mode browser picker root
│       │   │   └── halo/
│       │   │       ├── HaloShell.tsx       # Common picker shell (geometry + keyboard)
│       │   │       ├── HaloFrosted.tsx     # Frosted variant (default)
│       │   │       ├── HaloBezel.tsx       # Bezel variant
│       │   │       ├── HaloCrown.tsx       # Crown variant
│       │   │       ├── HaloPreview.tsx     # Live preview used in Settings
│       │   │       ├── geometry.ts         # Wheel sector math
│       │   │       ├── types.ts            # Shared picker types
│       │   │       ├── NumberBadges.tsx    # 1–9 keyboard shortcut badges
│       │   │       ├── PickerStyleChooser.tsx
│       │   │       └── ProfileOrderEditor.tsx  # Drag-to-reorder profile slots
│       │   ├── tray/
│       │   │   └── TrayPopover.tsx         # Menu-bar popover content
│       │   ├── onboarding/
│       │   │   └── OnboardingFlow.tsx      # First-run setup wizard
│       │   ├── i18n/
│       │   │   ├── index.ts               # i18next init
│       │   │   ├── languages.ts           # Language list
│       │   │   └── locales/               # Translation JSON (en, zh-CN, zh-TW, ja-JP)
│       │   └── styles/
│       │       └── app.css                # Global Tailwind + CSS variables
│       │
│       ├── src-tauri/              # Rust Tauri shell
│       │   ├── Cargo.toml
│       │   ├── tauri.conf.json     # App identifier, window config — bump version on release
│       │   ├── build.rs            # Tauri build script
│       │   ├── Info.plist.tmpl     # Template patched by patch-info-plist.sh
│       │   ├── capabilities/
│       │   │   └── default.json    # CSP + allowed Tauri commands for the renderer
│       │   └── src/
│       │       ├── lib.rs          # Tauri app setup: plugins, state init, deep-link, IPC
│       │       ├── main.rs         # Binary entry point (calls lib::run())
│       │       ├── state.rs        # AppState, DaemonMode
│       │       ├── commands/
│       │       │   └── mod.rs      # All #[tauri::command] handlers (Rust ↔ JS boundary)
│       │       ├── dispatch.rs     # Decision → browser launch, Ask thread spawning
│       │       ├── ipc_host.rs     # DaemonHandler (overrides RouteOpen for picker)
│       │       ├── url_handler.rs  # Deep-link callback → Router → dispatch
│       │       ├── picker.rs       # Picker window management, PickerState, show_picker()
│       │       ├── tray.rs         # System tray icon + menu (AppKit via objc2)
│       │       ├── suggestions.rs  # Rule suggestion Tauri commands
│       │       └── nmh_supervisor.rs  # Native Messaging Host process supervisor
│       │
│       ├── scripts/
│       │   └── patch-info-plist.sh  # Post-bundle plist rewrite (CFBundleURLTypes, etc.)
│       └── dmg/
│           ├── dmg-background.svg
│           └── dmg-background.tiff  # HiDPI TIFF for DMG install window
│
├── packages/
│   └── config-dsl/                 # `@linkpilot/config` npm package (TypeScript DSL)
│       ├── package.json            # Bump version on every release (stamps from git tag)
│       └── src/
│
├── packaging/
│   └── homebrew/
│       ├── Formula/                # `lpt` CLI Homebrew formula
│       └── Casks/                  # LinkPilot.app Homebrew cask
│
├── tools/
│   └── icon-padder/                # Dev tool: pads icon images for tray use
│
├── docs/
│   ├── brand/                      # Brand assets, tray icon SVG sources
│   └── release-notes/
│
├── scripts/                        # Repo-level scripts
├── rules/                          # Project rule files (supplementary to CLAUDE.md)
├── .agents/
│   └── skills/
│       └── add-config-capability/
│           └── SKILL.md            # Step-by-step guide for adding new config capabilities
├── .planning/
│   └── codebase/                   # GSD codebase map documents (this directory)
└── .github/
    └── workflows/
        ├── ci.yml                  # Rust check + frontend build + desktop bundle smoke test
        ├── release.yml             # Per-arch DMG builds + publish
        └── npm-publish.yml         # @linkpilot/config npm publish on tag
```

## Key File Locations

**Entry Points:**
- `apps/desktop/src-tauri/src/lib.rs`: Tauri app setup (deep-link hook, IPC server, state init)
- `apps/desktop/src-tauri/src/main.rs`: Binary entry point for the desktop app
- `crates/cli/src/main.rs`: `lpt` CLI entry point
- `crates/headless-daemon/src/main.rs`: Future headless daemon entry point
- `crates/native-host/src/main.rs`: NMH bridge placeholder

**Configuration Schema (the most-edited files):**
- `crates/core/src/config/mod.rs`: `ConfigDocument`, `Settings`, `Workspace`, `PickerStyle`, `LanguagePref`
- `crates/core/src/rules.rs`: `MatcherTree`, `Action`, `Rule`
- `crates/core/src/browser.rs`: `InstalledBrowser`, `BrowserProfile`, `BrowserTarget`

**Routing Engine:**
- `crates/core/src/routing.rs`: `Router::evaluate_explained()`, `eval_tree()`, `MatcherEval`
- `crates/core/src/daemon.rs`: `DaemonRuntime`, `evaluate_and_log()`

**IPC & Transport:**
- `crates/core/src/protocol.rs`: Wire format types (`Request`, `Response`, `Event`)
- `crates/ipc/src/transport.rs`: `read_raw_frame`, `write_frame` (u32 BE + UTF-8 JSON)
- `crates/ipc/src/server.rs`: Tokio async listener
- `crates/ipc/src/client.rs`: Blocking sync client

**macOS Platform:**
- `crates/platform-mac/src/lib.rs`: `MacProvider` constructor + `PlatformProvider` impl
- `crates/platform-mac/src/inventory.rs`: LaunchServices browser scan
- `crates/platform-mac/src/launcher.rs`: URL open + profile targeting
- `crates/platform-mac/src/launch_agent.rs`: Daemon LaunchAgent (install/uninstall/status)
- `crates/platform-mac/src/opener.rs`: Frontmost-app detection for source attribution

**Frontend API Layer:**
- `apps/desktop/src/lib/ipc.ts`: ALL typed wrappers for Tauri commands (single source of truth for frontend ↔ Rust boundary)
- `apps/desktop/src/lib/types.ts`: TypeScript type definitions mirroring core Rust structs

**Tauri Command Handlers:**
- `apps/desktop/src-tauri/src/commands/mod.rs`: All `#[tauri::command]` functions
- `apps/desktop/src-tauri/capabilities/default.json`: Permissions list (every command must appear here)

**Core UI Pages:**
- `apps/desktop/src/pages/rules.tsx`: Primary config UI
- `apps/desktop/src/pages/inspector.tsx`: Route history + explain-why
- `apps/desktop/src/pages/settings.tsx`: Global settings + default browser + CLI install
- `apps/desktop/src/pages/browsers.tsx`: Browser inventory management

**Picker (Ask-mode wheel):**
- `apps/desktop/src-tauri/src/picker.rs`: Rust picker state + `show_picker()` blocking call
- `apps/desktop/src/picker/PickerWindow.tsx`: React picker window root
- `apps/desktop/src/picker/halo/HaloShell.tsx`: Common geometry + keyboard shortcuts
- `apps/desktop/src/picker/halo/geometry.ts`: Sector math

**Build / Release:**
- `apps/desktop/scripts/patch-info-plist.sh`: Must run after `tauri build --bundles app`, before DMG
- `.github/workflows/release.yml`: Per-arch DMG matrix build
- `.github/workflows/ci.yml`: PR/main CI (rust + frontend + bundle smoke)

## Naming Conventions

**Files (Rust):**
- `snake_case.rs` for all Rust source files
- Modules named after the domain concept they own (e.g. `routing.rs`, `browser.rs`, `history.rs`)
- Test files as integration tests under `crates/<crate>/tests/`; unit tests inline in `#[cfg(test)]` mod at the bottom of the source file

**Files (TypeScript/React):**
- `PascalCase.tsx` for React components
- `camelCase.ts` for non-component modules (`ipc.ts`, `types.ts`, `utils.ts`)
- Pages named after their route/concept in `kebab-case.tsx` (`rules.tsx`, `test-url.tsx`, `menu-bar.tsx`)

**Directories:**
- Rust crates: `kebab-case` matching their `Cargo.toml` `name` field
- React directories: `camelCase` for feature groups (`picker/`, `halo/`, `tray/`, `onboarding/`)
- i18n locales: BCP-47 format (`en`, `zh-CN`, `zh-TW`, `ja-JP`)

**Rust types:**
- `PascalCase` for structs, enums, traits
- `snake_case` for functions, methods, fields, modules
- Enums with `#[serde(rename_all = "kebab-case")]` for JSON wire format
- Newtype wrappers for IDs: `BrowserId(String)`, `RuleId(Uuid)` — prevents accidental mixing

**TypeScript types:**
- Interfaces named after their Rust equivalent (e.g. `ConfigDocument`, `Rule`, `InstalledBrowser`)
- IPC response types defined in `lib/ipc.ts` if command-specific, or `lib/types.ts` if shared

## Where to Add New Code

**New rule matcher (e.g. time-of-day):**
1. Variant in `MatcherTree` enum → `crates/core/src/rules.rs`
2. `eval_tree()` branch + `MatcherEval` variant → `crates/core/src/routing.rs`
3. Tauri command (likely no new command needed; `rule_upsert` covers it) → `apps/desktop/src-tauri/src/commands/mod.rs`
4. Frontend input control in `RuleEditor.tsx` → `apps/desktop/src/components/RuleEditor.tsx`
5. CLI flag in `lpt rules add` → `crates/cli/src/main.rs`

**New settings field:**
1. Field with `#[serde(default)]` + `Default` impl → `crates/core/src/config/mod.rs` in `Settings`
2. Dedicated command if it has a side effect (see `set_launch_at_login` pattern), otherwise `config_replace` suffices → `apps/desktop/src-tauri/src/commands/mod.rs`
3. Register in `lib.rs` `generate_handler![]` and `capabilities/default.json`
4. Typed wrapper → `apps/desktop/src/lib/ipc.ts`
5. Toggle/control in `apps/desktop/src/pages/settings.tsx`
6. `lpt settings <name>` subcommand → `crates/cli/src/main.rs`

**New page:**
1. New file in `apps/desktop/src/pages/<name>.tsx`
2. Add route to `apps/desktop/src/App.tsx`
3. Navigation entry in sidebar

**New shared UI component:**
- Domain component: `apps/desktop/src/components/MyComponent.tsx`
- Generic primitive: `apps/desktop/src/components/ui/my-component.tsx` (Radix-based)

**New utility function (Rust):**
- If pure and domain-agnostic: inline or add to the relevant core module
- Never add to `platform-mac/` unless it calls macOS APIs

**New Tauri command:**
1. `#[tauri::command] pub fn my_command(state: State<'_, AppState>, ...) -> Result<T, String>` in `commands/mod.rs`
2. Register in `lib.rs` `generate_handler![..., commands::my_command]`
3. Add to `capabilities/default.json` `permissions` array
4. Add typed wrapper in `lib/ipc.ts`

**New IPC verb (rare — only for daemon-side-only operations):**
1. Add variant to `Request` and `Response` in `crates/core/src/protocol.rs`
2. Bump `PROTOCOL_VERSION` if it changes existing semantics
3. Handle in `DaemonRuntime::handle()` in `crates/core/src/daemon.rs`
4. If the GUI needs picker behavior: intercept in `DaemonHandler::handle()` in `apps/desktop/src-tauri/src/ipc_host.rs`

## Special Directories

**`dist/`:**
- Purpose: Local release staging artifacts (DMG staging, local builds)
- Generated: Yes (by release scripts and local builds)
- Committed: Partially (some staging dirs are committed, build outputs are not)

**`target/`:**
- Purpose: Cargo build output
- Generated: Yes
- Committed: No (gitignored)

**`apps/desktop/dist/`:**
- Purpose: Vite frontend build output
- Generated: Yes (by `npm run build`)
- Committed: Partially (for embedded builds)

**`.planning/codebase/`:**
- Purpose: GSD codebase map documents (this file and peers)
- Generated: By gsd-map-codebase
- Committed: Yes

**`.agents/skills/`:**
- Purpose: Claude Code skills with step-by-step guides for common repo tasks
- Committed: Yes (read at task start by skilled Claude runs)

**`apps/desktop/src-tauri/gen/schemas/`:**
- Purpose: Generated Tauri ACL/capability schemas
- Generated: Yes (by Tauri build)
- Committed: Yes (needed for `capabilities/default.json` validation)

---

*Structure analysis: 2026-06-05*
