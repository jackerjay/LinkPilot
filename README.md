# LinkPilot

> Route every link to the right browser, profile, and workspace.

LinkPilot is a macOS-first (Windows / Linux to follow) link router: it sits
between the OS, your browsers, and the apps that open URLs, and dispatches
each link to the browser + profile that matches your rules.

## Status

**v0.1 — feature-complete scaffold.** Workspace, Tauri shell with menu-bar
tray, fsnotify-backed config store, route history, all five GUI pages, and an
end-to-end macOS `lp open` flow. See `docs/linkpilot-design-v0.1.md` (PRD)
and the implementation plan saved to your Claude session.

Outstanding for v0.1 polish: real artwork (see `apps/desktop/src-tauri/icons/`),
`Set as Default Browser` Cocoa wiring, IPC server for cross-process CLI talk,
and replacing the JSON-textarea rule editor with a structured form.

## Quick start (macOS)

### CLI — no GUI required

```sh
cargo build -p linkpilot-cli
./target/debug/lp doctor              # writes default config + lists browsers
./target/debug/lp rules list
./target/debug/lp open https://github.com/anthropics/anthropic-cookbook
./target/debug/lp open https://figma.com --dry-run
./target/debug/lp open https://github.com --from-app Slack
```

First run writes a starter config to
`~/Library/Application Support/LinkPilot/linkpilot.config.json` (PRD §22 demo:
github / notion → Chrome Default, figma / youtube → Arc). Edit and re-run.

### Desktop app

```sh
# from repo root
cd apps/desktop
npm install
npm install -g @tauri-apps/cli       # if you don't already have it
cargo tauri dev                      # starts Vite + Tauri together
```

What to try:

- Menu bar icon stays after closing the window — daemon keeps running.
- Open https://github.com via Slack / Terminal → LinkPilot should be in the
  app picker (after you've set it as default in System Settings).
- Drop a fresh config in the JSON editor → atomic rewrite + the file is
  reloaded if you also `vim` it externally (anti-echo token prevents loops).
- Inspector tab shows every decision as it happens via the `route-logged`
  Tauri event.

### Production bundle

```sh
cd apps/desktop
cargo tauri build
open src-tauri/target/release/bundle/macos/LinkPilot.app
```

`cargo tauri icon ~/Downloads/icon.png` regenerates the full icon matrix
before bundling (the repo ships 1×1 placeholders).

## Layout

```
crates/
  core/                # routing engine, rule model, config store, fsnotify,
                       #   route history, platform traits, IPC protocol types
  platform-mac/        # macOS backend (real in v0.1)
  platform-win/        # Windows stub (real in v0.5)
  platform-linux/      # Linux  stub (real in v0.6+)
  ipc/                 # length-prefixed JSON over Unix socket / Named pipe
                       #   (transport ready; server lands in a later slice)
  native-host/         # NMH stdio bridge (v0.3)
  cli/                 # `lp` command-line client
  headless-daemon/     # reserved for a future GUI-less daemon binary
apps/
  desktop/             # Tauri 2 app
    src-tauri/         # Rust: tray, deep-link, commands, fsnotify wiring
    src/               # React + TypeScript frontend
      pages/           # menu-bar, rules, inspector, browsers, settings
      lib/             # typed Tauri command wrappers
  extension/           # MV3 browser extension (v0.3)
packages/
  config-dsl/          # @linkpilot/config TS DSL (v0.2+)
```

## Build (developer)

```sh
cargo check --workspace --exclude linkpilot-desktop   # core stack on any OS
cargo test  -p linkpilot-core                         # routing + history + config
cargo check -p linkpilot-desktop                      # Tauri app on macOS (or
                                                      # Linux with GTK/WebKit
                                                      # dev libs installed)
```

Cross-checking macOS-only Rust from Linux:

```sh
rustup target add x86_64-apple-darwin
cargo check --target x86_64-apple-darwin -p linkpilot-platform-mac -p linkpilot-cli
```

(The Tauri shell can't be cross-checked from Linux because it links real
Cocoa frameworks via cc; build that one on a Mac.)

Frontend:

```sh
cd apps/desktop
npm install
npm run build       # tsc --noEmit + vite build → apps/desktop/dist/
```
