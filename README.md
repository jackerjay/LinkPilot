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

LinkPilot is a macOS-first link router. It sits between macOS, your browsers,
and the apps that open URLs, then sends each link to the browser and profile
that best match your rules.

LinkPilot is not a browser. It is a small routing layer for people who split
work across Chrome profiles, Arc, Safari, Firefox, workspaces, and source apps
such as Slack, Lark, Terminal, or IDEs.

## Status

LinkPilot is currently focused on macOS.

- macOS desktop app: active.
- CLI and background daemon: active.
- Windows and Linux platform crates: present as stubs.
- Browser extension: reserved for a later milestone.

The current app includes:

- Rule-based URL routing by host, path, source app, source browser, and source
  profile.
- Browser/profile inventory for Chrome-family browsers, Arc, Firefox, Safari,
  and custom browser entries.
- Ask picker with Halo profile wheel, keyboard shortcuts, profile ordering,
  dark mode, and a Settings test URL flow that opens real browsers.
- Background daemon with Unix socket IPC so routing keeps working after the
  main window is closed.
- Menu-bar tray, inspector, test URL simulator, browser manager, settings, and
  onboarding.
- `lpt` CLI for opening URLs, managing rules, inspecting config, installing
  the daemon, and checking default-browser state.

## Install

Release artifacts are unsigned and macOS may quarantine them on first launch.
If needed, remove the quarantine flag after installing.

Homebrew is not a supported install path yet. Use the DMG or CLI tarball below
until the tap is published and verified.

### GUI app

Download the universal DMG from the latest GitHub release, copy
`LinkPilot.app` to `/Applications`, then open it:

```sh
curl -L https://github.com/jackerjay/LinkPilot/releases/latest/download/LinkPilot_<version>_universal.dmg -o LinkPilot.dmg
hdiutil attach LinkPilot.dmg
cp -R "/Volumes/LinkPilot/LinkPilot.app" /Applications/
hdiutil detach "/Volumes/LinkPilot"
xattr -dr com.apple.quarantine /Applications/LinkPilot.app
open /Applications/LinkPilot.app
```

After first launch, use the onboarding or Settings page to:

1. Register LinkPilot as the system default browser.
2. Install the background daemon LaunchAgent.
3. Install the bundled `lpt` command to `~/.local/bin`.

### CLI only

The CLI tarball ships `lpt` and `linkpilot-daemon`.

```sh
curl -L https://github.com/jackerjay/LinkPilot/releases/latest/download/lpt-macos.tar.gz \
  | tar -xz -C ~/.local/bin
chmod +x ~/.local/bin/lpt
```

Use the CLI-only path when you want terminal automation or a daemon without
opening the GUI.

## Quick Start

### Desktop

1. Open LinkPilot.
2. In onboarding or Settings, make LinkPilot the default browser.
3. Add or edit rules in the Rules page.
4. Use Test URL to dry-run a URL against the live routing engine.
5. Use Inspector to watch real routing decisions as they happen.

For Ask rules, LinkPilot opens a picker window. Hold Option over a
multi-profile browser to summon the Halo wheel, aim at a profile, then release
Option to open it. The Settings page includes a picker style test URL so you
can verify focus, profile targeting, and visual style without creating a real
rule.

### CLI

```sh
cargo build -p linkpilot-cli
./target/debug/lpt doctor
./target/debug/lpt open https://github.com
./target/debug/lpt open https://figma.com --dry-run
./target/debug/lpt open https://github.com --from-app Slack
```

Useful commands:

```sh
# Rules
lpt rules list --all
lpt rules add --host "*.figma.com" --target arc --priority 20
lpt rules add --host github.com --path "/oauth/*" --keep-source --priority 50
lpt rules add --from-app Slack --ask
lpt rules disable <id-prefix>
lpt rules delete <id-prefix>

# Workspaces
lpt workspaces add work --name Work
lpt workspaces disable work

# Config
lpt config show
lpt config path
lpt config set-default-target chrome --profile Default
lpt config export ./linkpilot.backup.json
lpt config import ./linkpilot.backup.json

# Settings
lpt settings show
lpt settings smart-routing off
lpt settings launch-at-login on
lpt settings history-retention 30

# Browsers
lpt browsers list
lpt browsers profiles chrome
lpt browsers custom add --id devbuild --name "Chrome Canary" \
  --kind chromium --exec /Applications/Google\ Chrome\ Canary.app

# Default browser and daemon
lpt default-browser status
lpt default-browser set
lpt daemon status
lpt daemon install
lpt daemon logs --follow
```

`lpt` talks to the running daemon over:

```text
~/Library/Application Support/LinkPilot/linkpilot.sock
```

When no daemon is available, commands that can run locally fall back to local
execution. Configuration is stored at:

```text
~/Library/Application Support/LinkPilot/linkpilot.config.json
```

## Development

Requirements:

- Rust 1.80+
- Node.js 22 recommended
- npm
- macOS for the full Tauri desktop app

From the repository root:

```sh
cargo check --workspace --exclude linkpilot-desktop
cargo test -p linkpilot-core
cargo check -p linkpilot-desktop
```

Frontend:

```sh
cd apps/desktop
npm install
npm run build
npx tauri dev
```

Production bundle:

```sh
cd apps/desktop
npm run bundle:mac
open ../../target/release/bundle/macos/LinkPilot.app
```

`bundle:mac` runs `tauri build` and then patches the generated
`Info.plist` so macOS recognizes LinkPilot as an HTTP/HTTPS browser handler.

## Repository Layout

```text
crates/
  core/                 # routing engine, rule model, ConfigStore, history, IPC protocol types
  platform-mac/         # macOS backend: browser inventory, launcher, default-browser hooks
  platform-win/         # Windows stub
  platform-linux/       # Linux stub
  ipc/                  # length-prefixed JSON over Unix socket / named pipe
  cli/                  # lpt command-line client
  headless-daemon/      # background daemon binary
  native-host/          # native messaging bridge reserved for extension work
apps/
  desktop/              # Tauri 2 desktop app
    src-tauri/          # Rust shell, tray, deep links, commands, picker
    src/                # React + TypeScript UI
  extension/            # browser extension placeholder
packages/
  config-dsl/           # TypeScript DSL for linkpilot.config.ts
packaging/
  homebrew/             # unpublished Formula/cask templates; not an install path yet
```

## Release

Maintainers publish releases by pushing a semver tag:

```sh
git tag v0.2.0
git push origin v0.2.0
```

The release workflow builds universal macOS binaries for the CLI, daemon, and
desktop app, patches the app bundle, wraps it in a DMG, and uploads release
artifacts with checksums.

## License

LinkPilot is licensed under the MIT License. See [LICENSE](LICENSE).
