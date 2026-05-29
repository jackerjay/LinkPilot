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

# LinkPilot — Codex project notes

Per-link router that sits between macOS, browsers, and apps that open URLs,
and dispatches each link to the browser + profile that matches user rules.
v0.1 is **macOS-only**; `platform-win` / `platform-linux` are stubs that
re-export `core::platform::StubProvider` until v0.5 / v0.6+.

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

Three workflows in `.github/workflows/`:

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
    unified `checksums.txt`, creates the Release as a **draft with all
    assets** (immutable-release setting forbids editing assets after
    publish), then flips it to published.
- **`npm-publish.yml`** (on `v*.*.*` tag) — builds + tests
  `packages/config-dsl` and publishes `@linkpilot/config` to npm. The
  published version is **stamped from the tag** (`npm version <tag>
  --no-git-tag-version`), not read from the committed `package.json`.

Release pipeline is **unsigned/unnotarized** — users hit
`xattr -dr com.apple.quarantine LinkPilot.app` on first launch.

### Release-pipeline gotchas (learned the hard way at v0.5.0–v0.5.2)

The three bugs below were chained — each one had to be fixed before the
next surfaced, because a failing job blocks everything downstream:

1. **macOS Intel runner.** GitHub retired `macos-13` (deprecation
   2025-09, fully unsupported 2025-12) with intermittent brownouts that
   leave jobs *queued forever* instead of failing fast. Intel x86_64
   builds must use `macos-15-intel` (GitHub's migration label, supported
   until the macos-15 image retires ~Fall 2027). A leg stuck `queued`
   for hours with an empty `runner_name` = retired-runner symptom, not a
   slow build.
2. **`publish` job has no checkout.** It only `download-artifact`s, so
   the working dir has no `.git`. Any `gh` call that infers the repo
   from a git remote (e.g. `gh release edit`) dies with "not a git
   repository" — set `GH_REPO: ${{ github.repository }}` on the step.
3. **Tags are immutable; the workflow file is read from the tagged
   commit.** Re-running a failed release run replays the *old* workflow,
   so a fix on `main` never helps an already-pushed tag. To recover a
   half-finished release, finish it by hand (e.g. publish the draft via
   `gh release edit <tag> --draft=false`); the workflow fix only helps
   the *next* tag. Same logic blocks retroactively re-publishing a
   skipped version — roll forward to a new patch tag instead.

**Bumping the version** touches every version file, not just Cargo:
`Cargo.toml` + `Cargo.lock` (run `cargo update --workspace` after
editing `Cargo.toml`), `apps/desktop/package.json` + `package-lock.json`,
`apps/desktop/src-tauri/tauri.conf.json`, `packages/config-dsl/package.json`,
and a `CHANGELOG.md` entry. Forgetting `config-dsl` is what wedged npm
publish from v0.4.1 through v0.5.1.

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

- Workspace `version = "0.2.0"`, `edition = "2021"`, `rust-version = "1.80"`,
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

`.Codex/skills/` ships Codex skills scoped to this repo:

- **`add-config-capability`** — invoke when extending the user-configurable
  surface (new `MatcherTree` variant, `Action` variant, `Settings` field,
  `Workspace` property, browser metadata field, etc.). Encodes the layered
  architecture (core schema → daemon Tauri command → `lib/tauri.ts` wrapper
  → React page → `lpt` CLI subcommand) so capabilities don't ship half-wired.
