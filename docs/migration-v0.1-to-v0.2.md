# Migrating from LinkPilot v0.1 to v0.2

v0.2 keeps the same on-disk config schema as v0.1, but it changes how
the daemon is hosted, what the CLI binary is called, and what the .app
looks like on macOS. This page walks through the upgrade.

## TL;DR

```sh
# 1) Stop the v0.1 daemon (if you ran one)
lp daemon uninstall || true     # only if you had v0.1's launchctl plist

# 2) Replace the .app
rm -rf /Applications/LinkPilot.app
open LinkPilot_0.2.0_universal.dmg
# drag LinkPilot.app to /Applications
xattr -dr com.apple.quarantine /Applications/LinkPilot.app

# 3) Open the new .app; it installs the v0.2 LaunchAgent automatically
open /Applications/LinkPilot.app

# 4) Update any shell aliases / scripts from `lp` to `lpt`
```

Your existing `linkpilot.config.json` keeps working — no schema change.

## What changed

### 1. The CLI is now `lpt`, not `lp`

macOS ships `/usr/bin/lp` (CUPS line-printer). Putting our `lp` on
`$PATH` shadows the system command, which Homebrew warned about every
install. v0.2 renames the binary to `lpt`.

**You need to update**:
- Any shell aliases (`alias lp=…`)
- Any scripts that call `lp open …` / `lp daemon …` / etc.
- Your shell history won't tab-complete the old name anymore

The new `lpt` is feature-equivalent — every subcommand, flag, and
output format from v0.1 still works under the new name. Run
`lpt --help` to confirm.

### 2. Daemon split out of the GUI

In v0.1 the daemon lived inside the Tauri app process; if the GUI
crashed or you force-quit it, the routing socket went down. In v0.2:

- `linkpilot-daemon` is a separate binary bundled in
  `LinkPilot.app/Contents/MacOS/`.
- The GUI's first launch writes `~/Library/LaunchAgents/
  app.linkpilot.daemon.plist`, which launchd uses to keep the daemon
  alive across logouts and crashes.
- When the GUI starts and a daemon is already serving, the GUI runs
  in **client mode** (no second daemon process) and talks to the
  external one over the same Unix socket.

**You don't have to do anything** — the GUI's first launch on v0.2
installs the LaunchAgent automatically. If you prefer manual control:

```sh
lpt daemon install      # writes plist, loads via launchctl
lpt daemon uninstall    # unload + remove plist
lpt daemon status       # current state
```

### 3. The .app is menubar-only

`LSUIElement = true` is now set in `Info.plist`. The GUI behaves like
Raycast / Alfred:

- No Dock icon
- Not in Cmd+Tab
- No main menu at the top of the screen
- Tray icon + main window (from the tray) are the only UI

Quit via the tray menu's **Quit LinkPilot** item.

This fixes v0.1's occasional focus-stealing on macOS: an Accessory
app can't promote itself to the foreground without the user clicking,
so background routing decisions don't yank focus from your editor.

### 4. New CLI subcommands

| Command | What it does |
|---|---|
| `lpt daemon start/stop/restart` | Manual daemon lifecycle when not using launchd |
| `lpt daemon install/uninstall` | LaunchAgent plist management |
| `lpt daemon status [--json]` | Liveness + PID + LaunchAgent state |
| `lpt daemon logs [-f] [-n N]` | Tail `~/Library/Logs/LinkPilot/daemon.{out,err}.log` |
| `lpt history [--limit N] [--json]` | Recent route decisions from the daemon's in-memory log |
| `lpt config compile <file.ts> [--to PATH]` | Compile a TypeScript config (see §5) |

### 5. TypeScript config DSL

You can keep writing JSON rules in the GUI. You can also now author
rules in TypeScript:

```ts
// linkpilot.config.ts
import { browser, defineConfig, printConfig, route } from "@linkpilot/config";

printConfig(defineConfig({
  defaultTarget: browser.arc(),
  rules: [
    route.host("github.com").to(browser.chrome.profile("Work")),
    route.path("/oauth/*").keepSource(),
    route.fromApp("Slack").to(browser.chrome.profile("Work")),
  ],
}));
```

Then:

```sh
brew install bun  # one-time
npm i -D @linkpilot/config
lpt config compile linkpilot.config.ts
```

The compiled rules show up in the GUI tagged **compiled** — read-only
there; edit them in your `.ts` file and re-run `lpt config compile`.
A "Copy to GUI" button on each compiled rule lets you fork one as an
editable GUI-source rule when you need an exception.

### 6. IPC protocol bumped (1 → 2)

If you ever run mixed-version components (e.g. v0.2 CLI against an
old v0.1 daemon), the new verbs (`route-history`) won't be recognised.
The CLI now surfaces a clear upgrade prompt instead of a cryptic
transport error. Upgrade both sides to v0.2 to clear the message.

## Things that did NOT change

- The on-disk JSON config (`linkpilot.config.json`) — schema is identical
- The Tauri command surface used by the GUI — your saved rules / workspaces
  / settings all read back the same way
- macOS default-browser registration — CFBundleURLTypes /
  CFBundleDocumentTypes are unchanged

## Rolling back

Re-install the v0.1 DMG over /Applications, then:

```sh
launchctl unload ~/Library/LaunchAgents/app.linkpilot.daemon.plist
rm -f ~/Library/LaunchAgents/app.linkpilot.daemon.plist
```

Your config file stays compatible going the other way (v0.2 didn't
add any required fields).
