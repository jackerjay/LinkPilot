# Homebrew cask for the LinkPilot GUI.
#
# Local install (development only):
#   brew install --cask ./packaging/homebrew/Casks/linkpilot.rb
#
# Public install (after this file is pushed to jackerjay/homebrew-linkpilot):
#   brew install --cask jackerjay/linkpilot/linkpilot
#
# Ships the .app bundle from the release DMG. The .app carries `lpt` and
# `linkpilot-daemon` embedded in Contents/MacOS — the GUI's first-run
# hook writes the LaunchAgent plist pointing into the bundle, and the
# Settings "Install lpt on PATH" button symlinks the embedded `lpt` to
# ~/.local/bin. Users who want `lpt` on PATH without the GUI install the
# separate `linkpilot-cli` formula instead.

cask "linkpilot" do
  # Map Homebrew's :arm/:intel symbols to our DMG asset naming.
  # The release.yml matrix builds two DMGs per tag — one for each arch —
  # named `LinkPilot_<version>_aarch64.dmg` and `..._x86_64.dmg`.
  arch arm: "aarch64", intel: "x86_64"

  # TODO: when v0.5.0 actually ships, replace `:no_check` with the real
  # per-arch hashes from `dist/release/checksums.txt`:
  #   sha256 arm:   "<aarch64 dmg sha>",
  #          intel: "<x86_64 dmg sha>"
  version "0.5.0"
  sha256 :no_check

  url "https://github.com/jackerjay/LinkPilot/releases/download/v#{version}/LinkPilot_#{version}_#{arch}.dmg"
  name "LinkPilot"
  desc "Route every link to the right browser, profile, and workspace"
  homepage "https://github.com/jackerjay/LinkPilot"

  # `livecheck` lets `brew livecheck` and the autobumper notice new
  # releases. The regex strips the `v` prefix on tags so it matches
  # the `version` stanza format. `strategy :github_latest` would
  # ignore pre-releases — we want them while still on alpha/beta, so
  # do the regex extract manually.
  livecheck do
    url :url
    regex(/v?(\d+(?:\.\d+)+(?:-(?:alpha|beta|rc)(?:\.\d+)?)?)/i)
    strategy :github_releases
  end

  # Unsigned build. We can't notarize without an Apple Developer ID, so
  # the postflight below strips the quarantine flag for the user (see the
  # caveats — the removal is intentionally surfaced, not silent). This is
  # why the recipe can only live in our own tap: `brew audit` rejects
  # Gatekeeper-bypass behaviour, and the official homebrew-cask never
  # accepts it.
  auto_updates false
  # Matches Info.plist LSMinimumSystemVersion (12.0 = Monterey). Cask's
  # idiomatic form is the symbol alone — implicitly "this or newer".
  depends_on macos: :monterey

  app "LinkPilot.app"

  # The .app is unsigned, so Homebrew's default quarantine would make
  # macOS block it on first launch ("LinkPilot is damaged"). Strip the
  # quarantine xattr here instead of making every user run `xattr -dr`
  # by hand. Runs as the user, after the bundle is in place.
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine", "#{appdir}/LinkPilot.app"]
  end

  uninstall launchctl: "app.linkpilot.daemon",
            quit:      "app.linkpilot.desktop",
            delete:    "~/Library/LaunchAgents/app.linkpilot.daemon.plist"

  # `zap` is opt-in deep clean (brew uninstall --zap). Keeps these on a
  # plain uninstall so a reinstall preserves the user's rules, history,
  # and routing logs.
  zap trash: [
    "~/Library/Application Support/LinkPilot",
    "~/Library/Caches/app.linkpilot.desktop",
    "~/Library/Logs/LinkPilot",
    "~/Library/Preferences/app.linkpilot.desktop.plist",
    "~/Library/Saved Application State/app.linkpilot.desktop.savedState",
  ]

  caveats <<~EOS
    LinkPilot ships unsigned (no Apple Developer ID / notarization yet).
    This cask already removed the macOS quarantine flag from
    LinkPilot.app during install so it launches without a Gatekeeper
    prompt. If you later move or re-download the app outside Homebrew,
    re-run:
      xattr -dr com.apple.quarantine /Applications/LinkPilot.app

    On first launch the GUI writes ~/Library/LaunchAgents/
    app.linkpilot.daemon.plist pointing at the daemon binary bundled
    inside the .app. From then on, `lpt open <url>` works system-wide
    (even from headless contexts) through that LaunchAgent.

    To uninstall the LaunchAgent (and stop the daemon) without removing
    the .app itself:
      lpt daemon uninstall
  EOS
end
