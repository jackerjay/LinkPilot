# LinkPilot

> Route every link to the right browser, profile, and workspace.

LinkPilot is a macOS-first (Windows / Linux to follow) link router: it sits
between the OS, your browsers, and the apps that open URLs, and dispatches
each link to the browser + profile that matches your rules.

## Status

**v0.1, step 1 of 10 — workspace scaffold.** The Cargo workspace, Tauri shell,
platform-trait skeleton, and minimal CLI all compile; no end-to-end routing
yet. See `docs/linkpilot-design-v0.1.md` (PRD) and `/root/.claude/plans/jolly-sauteeing-bunny.md` (implementation plan).

## Layout

```
crates/
  core/                # routing engine, rule model, config, platform traits
  platform-mac/        # macOS backend (the only real implementation in v0.1)
  platform-win/        # Windows stub (real in v0.5)
  platform-linux/      # Linux  stub (real in v0.6+)
  ipc/                 # length-prefixed JSON over Unix socket / Named pipe
  native-host/         # NMH stdio bridge (v0.3)
  cli/                 # `lp` command-line client
  headless-daemon/     # reserved for a future GUI-less daemon binary
apps/
  desktop/             # Tauri app: menu bar, config UI, hosts the daemon
  extension/           # MV3 browser extension (v0.3)
packages/
  config-dsl/          # @linkpilot/config TS DSL (v0.2+)
```

## Build (developer)

```sh
cargo check --workspace --exclude linkpilot-desktop   # core stack on any OS
cargo check -p linkpilot-desktop                      # Tauri app; macOS or
                                                      # webkit2gtk-4.1-dev on
                                                      # Linux
```

The Tauri shell requires WebKit on Linux and Xcode CLT on macOS. Bundling
(`cargo tauri build`) is intended to be done on macOS for v0.1.
