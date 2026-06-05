# Homebrew packaging

Source files for the `jackerjay/homebrew-linkpilot` tap. This directory
mirrors a Homebrew tap repo's layout so publishing is a plain `cp -R` into
the tap. The recipes here are the source of truth; the tap repo is a mirror.

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

## Local install

This is how M5.3 verification runs — never publishes anything, just
exercises the on-disk recipe against a real Homebrew prefix.

**Cask** still installs from a raw file path:

```sh
# GUI (.app dropped into /Applications via DMG)
brew install --cask ./packaging/homebrew/Casks/linkpilot.rb
```

**Formula** can no longer be installed from a bare path — Homebrew 5.x
rejects it ("Homebrew requires formulae to be in a tap"). Drop it into a
throwaway tap first:

```sh
TAP="$(brew --repository)/Library/Taps/lptest/homebrew-lptest"
mkdir -p "$TAP/Formula" && (cd "$TAP" && git init -q && git commit -q --allow-empty -m init)
cp packaging/homebrew/Formula/linkpilot-cli.rb "$TAP/Formula/"
brew install --formula lptest/lptest/linkpilot-cli
```

The cask carries a `postflight` that strips the quarantine flag (the
build is unsigned). Installing it triggers a one-time tap-trust prompt;
set `HOMEBREW_NO_REQUIRE_TAP_TRUST=1` for unattended runs, and pass
`--appdir=/tmp/...` to avoid the `/Applications` sudo prompt while testing.

Uninstall with the canonical Homebrew commands:

```sh
brew uninstall linkpilot-cli
brew uninstall --cask linkpilot          # leaves config + history intact
brew uninstall --zap --cask linkpilot    # also nukes ~/Library/{Application Support,Logs}/LinkPilot
```

## Publishing a release to the tap

The recipes are pinned to placeholder shas until a release exists. After a
`vX.Y.Z` tag has built and `release.yml` has published the GitHub Release,
fill the recipes from that release's `checksums.txt` and push them to the tap.

1. **Pull the shas** from the release's `checksums.txt`. You need six:
   - DMGs (2): `LinkPilot_<v>_aarch64.dmg`, `LinkPilot_<v>_x86_64.dmg`
   - CLI + daemon tarballs (4): `lpt-macos-{aarch64,x86_64}.tar.gz`,
     `linkpilot-daemon-macos-{aarch64,x86_64}.tar.gz`

2. **Edit the recipes in this repo** (the source of truth):
   - `Casks/linkpilot.rb` — set `version`, and replace `sha256 :no_check`
     with the two per-arch DMG shas:
     ```ruby
     sha256 arm:   "<aarch64 dmg sha>",
            intel: "<x86_64 dmg sha>"
     ```
   - `Formula/linkpilot-cli.rb` — set `version`, and replace the four
     `0…0` placeholders: arm `lpt`, arm daemon, intel `lpt`, intel daemon.

   There is no `bump-tap.sh` helper — do it by hand; the per-release shas are
   the only moving part (the DMG/tarball URLs roll automatically off
   `#{version}`).

3. **Commit on `main`, then mirror into the tap repo:**
   ```sh
   git add packaging/homebrew/ && git commit -m "release: bump Homebrew recipes to vX.Y.Z" && git push
   git clone git@github.com:jackerjay/homebrew-linkpilot.git /tmp/hb && cd /tmp/hb
   mkdir -p Formula Casks
   cp <repo>/packaging/homebrew/Formula/linkpilot-cli.rb Formula/
   cp <repo>/packaging/homebrew/Casks/linkpilot.rb        Casks/
   git add -A && git commit -m "linkpilot vX.Y.Z" && git push
   ```
   Tap repos need no tag — `brew update` picks up the next commit.

4. **Verify end to end:**
   ```sh
   brew tap jackerjay/linkpilot
   brew install jackerjay/linkpilot/linkpilot-cli && lpt --version
   brew install --cask jackerjay/linkpilot/linkpilot   # postflight strips quarantine
   brew uninstall --cask linkpilot && brew uninstall linkpilot-cli && brew untap jackerjay/linkpilot
   ```
   The cask's `postflight` runs arbitrary shell (the `xattr` de-quarantine),
   so Homebrew 5.x shows a one-time tap-trust prompt; unattended runs need
   `HOMEBREW_NO_REQUIRE_TAP_TRUST=1`. A bad sha is fixed by pushing a
   corrected commit to the tap repo — no re-tag needed.

## Why two recipes?

Splits the audience that doesn't need 80MB of bundled Tauri WebKit from
the audience that does. Both ship the same `lpt` binary; the formula is
the headless path (CLI + `lpt daemon install` for a LaunchAgent), the
cask is the GUI path (which also gets `lpt` symlinked via the Settings
"Install lpt on PATH" button).
