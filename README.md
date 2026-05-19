<p align="center">
  <img src="docs/brand/icon.png" alt="LinkPilot" width="128" height="128">
</p>

<h1 align="center">LinkPilot</h1>

<p align="center">
  <em>Route every link to the right browser, profile, and workspace.</em>
</p>

<p align="center">
  English | <a href="README.zh.md">简体中文</a>
</p>

LinkPilot is a macOS-first (Windows / Linux to follow) link router: it sits
between the OS, your browsers, and the apps that open URLs, and dispatches
each link to the browser + profile that matches your rules.

## Status

**v0.1 — feature-complete.** Workspace, Tauri shell with menu-bar tray,
fsnotify-backed config store, route history, all five GUI pages, and an
end-to-end macOS `lpt open` flow. Real brand artwork shipped (full icon
matrix from `docs/brand/icon.png` + a single-color menu-bar template from
`docs/brand/tray-template.svg`); structured rule editor replaces the
JSON-textarea fallback (still available under “Advanced: raw JSON”).
`Set as Default Browser` (LaunchServices), `Launch at Login` (LaunchAgent
plist), and the daemon's Unix-socket IPC server are all wired.

See `docs/linkpilot-design-v0.1.md` (PRD) for the design.

## Install

Two install paths from the same release. Pick whichever (or both).

### CLI only

Headless — useful for terminal workflows, scripts, or alongside the
GUI from a different release. The `lpt` binary is a single static-ish
executable; no daemon required (`lpt open` does local routing when no
daemon is running, and talks to the GUI's daemon over Unix socket
when both are installed).

```sh
# From a release artifact:
curl -L https://github.com/jackerjay/LinkPilot/releases/latest/download/lpt-macos.tar.gz \
  | tar -xz -C ~/.local/bin
chmod +x ~/.local/bin/lpt
# Add ~/.local/bin to PATH if it isn't already.
```

### GUI + CLI

Install the `.app` and the bundled `lpt` binary comes along. After
launching LinkPilot, open Settings → Command-line tool and click
**Install to ~/.local/bin** to symlink `lpt` onto your PATH (idempotent;
re-run after a version upgrade). The bundled binary lives at
`/Applications/LinkPilot.app/Contents/MacOS/lp` — you can also add
that directory to PATH directly instead of symlinking.

```sh
curl -L https://github.com/jackerjay/LinkPilot/releases/latest/download/LinkPilot_<version>_universal.dmg -o LinkPilot.dmg
hdiutil attach LinkPilot.dmg
cp -R "/Volumes/LinkPilot/LinkPilot.app" /Applications/
hdiutil detach "/Volumes/LinkPilot"
xattr -dr com.apple.quarantine /Applications/LinkPilot.app   # unsigned build
open /Applications/LinkPilot.app
```

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

`lpt` talks to the running daemon over a Unix socket
(`~/Library/Application Support/LinkPilot/linkpilot.sock`) when one is up,
and falls back to local execution otherwise. Force the local path with
`--local`. Writes always go through the local file; the daemon's fsnotify
watcher picks them up via the anti-echo token, so a running GUI refreshes
within a frame.

First run writes a starter config to
`~/Library/Application Support/LinkPilot/linkpilot.config.json` (PRD §22 demo:
github / notion → Chrome Default, figma / youtube → Arc). Edit and re-run.

The CLI mirrors everything the GUI can configure — see `lpt <command> --help`
for the full surface:

```sh
# Rules
lpt rules add --host "*.figma.com" --target arc --priority 20
lpt rules add --host github.com --path "/oauth/*" --keep-source --priority 50
lpt rules add --from-app Slack --ask
lpt rules list --all                      # include disabled rules
lpt rules disable <id-prefix>             # 8-char prefix is enough
lpt rules set-priority <id-prefix> 99
lpt rules delete <id-prefix>
lpt rules add --when-json '{"op":"any","of":[...]}' --then-json '{"kind":"block"}'

# Workspaces (batch on/off groups of rules)
lpt workspaces add work --name Work
lpt workspaces disable work               # all `workspace_id=work` rules skipped

# Config inspection + import/export
lpt config show                           # whole document as JSON
lpt config path
lpt config set-default-target arc --profile Personal
lpt config export ./backup.json
lpt config import ./backup.json

# Settings
lpt settings show
lpt settings smart-routing off            # master kill-switch
lpt settings launch-at-login on
lpt settings history-retention 30         # or `clear` for unlimited

# Browsers
lpt browsers list                         # auto-detected + custom, merged
lpt browsers profiles chrome
lpt browsers custom add --id devbuild --name "Chrome Canary" \
    --kind chromium --exec /Applications/Google\ Chrome\ Canary.app

# Default-browser registration
lpt default-browser status
lpt default-browser set                   # triggers the macOS confirm prompt
```

### Desktop app

```sh
# from repo root
cd apps/desktop
npm install                          # installs @tauri-apps/cli locally
npx tauri dev                        # starts Vite + Tauri together
```

> **Note:** use `npx tauri …` (or `npm run tauri -- …`). The Tauri CLI is
> a npm devDependency — `cargo tauri …` only works if you also
> `cargo install tauri-cli`, which this repo does **not** require.

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
npm run bundle:mac      # tauri build + patch-info-plist.sh on the .app
open ../../target/release/bundle/macos/LinkPilot.app
```

`bundle:mac` runs `tauri build` and then patches the bundled `.app`'s
Info.plist so the macOS "Default web browser" picker recognises LinkPilot
(see `apps/desktop/scripts/patch-info-plist.sh` for details).
The DMG it produces alongside is _not_ patched — for a DMG with the
plist patch baked in, push a `v*.*.*` tag and use the
`release.yml`-produced artifact instead (see "Releases" below).

Real brand artwork is already shipped in `apps/desktop/src-tauri/icons/`
(generated from `docs/brand/icon.png` + `docs/brand/tray-template.svg`).
To re-generate after editing source art, see
`apps/desktop/src-tauri/icons/README.md`.

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
  cli/                 # `lpt` command-line client
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

## Contributing

LinkPilot is open source under MIT OR Apache-2.0. See `CONTRIBUTING.md` for
local setup, PR expectations, and the release process. Please report security
issues privately; see `SECURITY.md`.

## Releases

Maintainers publish releases by pushing a semver tag:

```sh
git tag v0.1.0
git push origin v0.1.0
```

The tag triggers `.github/workflows/release.yml`, which on `macos-latest`:

1. Builds the `lpt` CLI for both `x86_64-apple-darwin` and
   `aarch64-apple-darwin` and `lipo`s them into one universal binary.
2. Builds the Tauri shell with `--target universal-apple-darwin --bundles app`
   so the `.app` is also a universal binary.
3. Runs `apps/desktop/scripts/patch-info-plist.sh` against the bundled `.app`
   — rewrites the `tauri-plugin-deep-link` auto-injected `CFBundleURLTypes`
   entry to Viewer/`Default` and adds `CFBundleDocumentTypes` for HTML so
   the macOS "Default web browser" picker actually surfaces LinkPilot.
4. Wraps the patched `.app` in a vanilla `LinkPilot_<version>_universal.dmg`
   via `hdiutil` (a second `tauri build --bundles dmg` would re-bundle the
   `.app` and overwrite the plist patch).
5. Uploads `lpt-macos`, `lpt-macos.tar.gz`, the DMG, and `checksums.txt`
   to a GitHub Release.

A single universal DMG works on both Apple Silicon and Intel Macs.

Every PR also runs a `desktop-bundle` smoke job on `macos-latest`
(`tauri build --debug --bundles app`) so packaging regressions show up
before a tag is cut.

Current release artifacts are unsigned. On macOS, unsigned builds may require
removing quarantine before first launch:

```sh
xattr -dr com.apple.quarantine LinkPilot.app
```
