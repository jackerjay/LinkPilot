# LinkPilot

> Route every link to the right browser, profile, and workspace.

LinkPilot is a macOS-first (Windows / Linux to follow) link router: it sits
between the OS, your browsers, and the apps that open URLs, and dispatches
each link to the browser + profile that matches your rules.

## Status

**v0.1, steps 1–5 of 10 — CLI smoke test on macOS.** The Cargo workspace,
Tauri shell, platform traits, and CLI all compile. On macOS, `lp open <url>`
loads the config, evaluates the router, and launches the matched browser /
profile directly — no daemon or IPC server yet. See
`docs/linkpilot-design-v0.1.md` (PRD) and
`/root/.claude/plans/jolly-sauteeing-bunny.md` (implementation plan).

## Quick start (macOS)

```sh
cargo build -p linkpilot-cli
./target/debug/lp doctor              # writes default config + lists browsers
./target/debug/lp rules list
./target/debug/lp open https://github.com/anthropics/anthropic-cookbook
./target/debug/lp open https://figma.com --dry-run    # decision only
./target/debug/lp open https://github.com --from-app Slack
```

The first run writes a starter config to
`~/Library/Application Support/LinkPilot/linkpilot.config.json` (PRD §22 demo:
github / notion → Chrome Default, figma / youtube → Arc, otherwise Arc).
Edit that file and re-run.

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
cargo test  -p linkpilot-core                         # routing + config tests
cargo check -p linkpilot-desktop                      # Tauri app; macOS or
                                                      # webkit2gtk-4.1-dev on
                                                      # Linux
```

The Tauri shell requires WebKit on Linux and Xcode CLT on macOS. Bundling
(`cargo tauri build`) is intended to be done on macOS for v0.1.

Cross-checking macOS-only code from Linux:

```sh
rustup target add x86_64-apple-darwin
cargo check --target x86_64-apple-darwin -p linkpilot-platform-mac -p linkpilot-cli
```
