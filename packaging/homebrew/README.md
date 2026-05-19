# Homebrew packaging

Source files for the `jackerjay/homebrew-linkpilot` tap that ships in
**M6**. M5 lands these locally so they can be exercised against
v0.2.0-alpha.3 release artifacts before they ever get pushed to a tap.

## Layout

The directory mirrors a Homebrew tap repo's expected layout so that the
M6 release step can do nothing more than a `cp -R`:

```
packaging/homebrew/
├── Formula/
│   └── linkpilot-cli.rb        # CLI only — ships `lpt` + `linkpilot-daemon`
└── Casks/
    └── linkpilot.rb            # GUI .app from the release DMG
```

## Local install (no tap required)

`brew install` accepts a raw file path for both formulae and casks.
That's how M5.3 verification runs — never publishes anything, just
exercises the on-disk recipe against a real Homebrew prefix.

```sh
# CLI (lpt + linkpilot-daemon under $(brew --prefix)/bin)
brew install --formula ./packaging/homebrew/Formula/linkpilot-cli.rb

# GUI (.app dropped into /Applications via DMG)
brew install --cask ./packaging/homebrew/Casks/linkpilot.rb
```

Uninstall with the canonical Homebrew commands:

```sh
brew uninstall linkpilot-cli
brew uninstall --cask linkpilot          # leaves config + history intact
brew uninstall --zap --cask linkpilot    # also nukes ~/Library/{Application Support,Logs}/LinkPilot
```

## Bump for a new release (M6 + later)

When v0.2.0 ships, bump three things in `Formula/linkpilot-cli.rb` and
two in `Casks/linkpilot.rb`:

| File | Bump |
|---|---|
| `linkpilot-cli.rb` | `version`, `url`, `sha256` (main + `daemon` resource) |
| `linkpilot.rb` | `version`, `sha256` (the DMG url uses `#{version}` so it auto-rolls) |

The release.yml workflow's `checksums.txt` artifact carries the sha256
for every file in one place — copy from there. A `scripts/bump-tap.sh`
helper to automate this lands as part of M6.

## Why two recipes?

Splits the audience that doesn't need 80MB of bundled Tauri WebKit from
the audience that does. Both ship the same `lpt` binary; the formula is
the headless path (CLI + `lpt daemon install` for a LaunchAgent), the
cask is the GUI path (which also gets `lpt` symlinked via the Settings
"Install lpt on PATH" button).
