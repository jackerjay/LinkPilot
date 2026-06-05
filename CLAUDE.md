# Coding Behavior Contract (12 Rules)

## Core (Karpathy via Forrest Chang)

1. Think before coding. State your assumptions. Surface tradeoffs.
   Ask before guessing. Push back when a simpler approach exists.
2. Simplicity first. Minimum code that solves the problem. No
   speculative features. No abstractions for single-use code.
3. Surgical changes. Touch only what is asked. Do not "improve"
   adjacent code, comments, or formatting. Match existing style.
4. Goal-driven execution. Define success criteria. Loop until
   verified. Do not narrate steps; tell me what success looks like.

## Extended (Mnimiy, May 2026)

5. Do not make the model do non-language work. Retry policies,
   routing, escalation thresholds belong in deterministic code.
6. Hard token budgets, no exceptions. Stop and ask if a task is
   trending past its budget.
7. Surface conflicts, do not average them. If two parts of the
   codebase disagree, flag the disagreement and ask which to follow.
8. Read before you write. Understand adjacent code (the file and
   nearby siblings) before adding new code.
9. Tests are required but are not the goal. A passing test that
   tests nothing useful is a failure. Tests must check behavior.
10. Long-running operations require checkpoints. After every
    significant step, summarize what was done and confirm before
    proceeding.
11. Convention beats novelty. In an established codebase, match
    the existing pattern even if a "better" one exists.
12. Fail visibly, not silently. Surface every skipped record,
    every rolled-back transaction, every constraint violation.
    Never report success when something was bypassed.

## Project-specific rules below this line

## UI interaction feedback

- Profile Halo configuration controls must stay attached to the wheel's
  actual sectors: adding belongs to a reserved `+` sector, deletion belongs
  near the active profile sector, and reordering should use direct sector
  drag. Avoid detached side panels or global controls when they make the
  action target ambiguous.
- Halo drag editing must show a real drag state: active sector, pointer-following
  profile ghost, and live slot reflow while dragging. Do not rely on a
  click-then-button move model or on commit-only reordering feedback.

# LinkPilot — Claude Code project notes

Per-link router that sits between macOS, browsers, and apps that open URLs,
and dispatches each link to the browser + profile that matches user rules.
v0.1 is **macOS-only**; `platform-win` / `platform-linux` are stubs that
re-export `core::platform::StubProvider` (win/linux remain stubs; no committed cross-platform milestone yet).

## Layout cheat-sheet

```
crates/
  core/                 # routing engine, rule model, ConfigStore, fsnotify,
                        # RouteHistory, PlatformProvider trait, IPC protocol
  platform-mac/         # real macOS backend (LaunchServices, LaunchAgent, icons)
  platform-{win,linux}/ # stubs
  ipc/                  # length-prefixed JSON over Unix socket / Named pipe
  native-host/          # NMH stdio bridge (v0.3)
  cli/                  # `lpt` command-line client (talks to daemon over IPC)
  headless-daemon/      # reserved for a future GUI-less daemon
apps/desktop/
  src-tauri/            # Rust: tray, deep-link, commands, fsnotify wiring,
                        # picker.rs (Cmd-Tab-style browser chooser)
  src/                  # React + TS frontend (menu-bar, rules, inspector,
                        # browsers, settings pages)
  scripts/              # patch-info-plist.sh runs after bundling
```

## Common commands

```sh
# Workspace check / test (no Tauri shell — runs anywhere)
cargo check --workspace --exclude linkpilot-desktop
cargo test  -p linkpilot-core

# Tauri shell — needs to run on macOS (links real Cocoa frameworks)
cargo check -p linkpilot-desktop

# CLI release build
cargo build --release -p linkpilot-cli   # → target/release/lp

# CLI surface — covers everything in the GUI config:
#   open / doctor / rules / workspaces / config / settings / browsers
#   / default-browser. Writes always go through the local config file
#   (atomic rewrite + anti-echo token); a running daemon's fsnotify
#   watcher picks them up automatically. `lpt history` is intentionally
#   absent — RouteHistory only lives in the daemon's memory and the
#   IPC protocol has no endpoint for it yet.

# Desktop dev
cd apps/desktop
npm install
npx tauri dev           # Vite + Tauri together
npm run build           # tsc --noEmit + vite build (no Tauri)
npm run bundle:mac      # tauri build + Info.plist patch on the .app
```

## CI / Release

Two workflows in `.github/workflows/`:

- **`ci.yml`** (every PR + `main` push)
  - `rust`: fmt + clippy + test + `cargo check -p linkpilot-desktop` on `macos-latest`
  - `frontend`: tsc + vite build on `ubuntu-latest`
  - `desktop-bundle`: `tauri build --debug --bundles app` smoke test on `macos-latest`
- **`release.yml`** (on `v*.*.*` tag)
  - Matrix over `{aarch64-apple-darwin on macos-14, x86_64-apple-darwin on macos-15-intel}`.
    Each leg runs natively — no `lipo`, no `universal-apple-darwin` target.
  - Per leg: builds `lpt` + `linkpilot-daemon` + Tauri
    `--target <arch> --bundles app`, embeds the CLI and daemon into
    `LinkPilot.app/Contents/MacOS/`, runs `patch-info-plist.sh`, then
    `hdiutil`s a per-arch DMG `LinkPilot_<version>_<arch>.dmg`.
  - Each leg uploads its `LinkPilot_<v>_<arch>.dmg`, `lpt-macos-<arch>` +
    `.tar.gz`, `linkpilot-daemon-macos-<arch>` + `.tar.gz`, and a per-arch
    checksums file as an `actions/upload-artifact` bundle.
  - A `publish` job (depends on both matrix legs) downloads both bundles,
    flattens them into a single `dist/release/` directory, regenerates a
    unified `checksums.txt`, then creates the draft Release and publishes.

Release pipeline is **unsigned/unnotarized** — users hit
`xattr -dr com.apple.quarantine LinkPilot.app` on first launch.

There is also an **`npm-publish.yml`** on the same tag trigger that ships
`@linkpilot/config` (version stamped from the tag). For release-pipeline
gotchas — retired Intel runners, the checkout-less `publish` job, tag
immutability, and the full version-bump file list — see the
"Release-pipeline gotchas" section in `AGENTS.md`.

## Plist patch — why the dance

`tauri-plugin-deep-link` auto-injects a `CFBundleURLTypes` entry with
`CFBundleTypeRole=Editor` and an internal-looking URL name. macOS's
"Default web browser" picker silently ignores apps unless:

- `CFBundleURLTypes[0]` has `Viewer` role + `LSHandlerRank=Default`
- `CFBundleDocumentTypes` declares HTML/XHTML viewing
- `NSUserActivityTypes` includes `NSUserActivityTypeBrowsingWeb`

`apps/desktop/scripts/patch-info-plist.sh` rewrites those after
`tauri build` produces the `.app`. In CI the patch must run between
`--bundles app` and DMG creation; a second `tauri build --bundles dmg`
would re-bundle the `.app` and clobber the patched plist, which is why
the release workflow uses `hdiutil create` instead.

## Conventions

- Workspace `version = "0.5.6"`, `edition = "2021"`, `rust-version = "1.80"`,
  `license = "MIT"`.
- Platform-specific Rust code is gated via `#[cfg(target_os = "macos")]` /
  `#[cfg(not(target_os = "macos"))]`. The Tauri shell's `picker.rs` and
  `tray.rs` use AppKit via `objc2` and `window-vibrancy`.
- Frontend uses Tailwind v4 + Radix UI primitives. State flows
  React → `lib/tauri.ts` wrappers → `commands::*` Rust handlers.
- Config lives at
  `~/Library/Application Support/LinkPilot/linkpilot.config.json`;
  daemon IPC socket at
  `~/Library/Application Support/LinkPilot/linkpilot.sock`.

## Build deps

- macOS: just Rust + Node 22.
- Linux dev (workspace check only — no Tauri shell):
  `libwebkit2gtk-4.1-dev libsoup-3.0-dev libgtk-3-dev librsvg2-dev libssl-dev pkg-config`.

## Project skills

`.claude/skills/` ships Claude Code skills scoped to this repo:

- **`add-config-capability`** — invoke when extending the user-configurable
  surface (new `MatcherTree` variant, `Action` variant, `Settings` field,
  `Workspace` property, browser metadata field, etc.). Encodes the layered
  architecture (core schema → daemon Tauri command → `lib/tauri.ts` wrapper
  → React page → `lpt` CLI subcommand) so capabilities don't ship half-wired.
