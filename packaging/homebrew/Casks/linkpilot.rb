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

  # Per-arch DMG hashes from the v0.5.10 release `checksums.txt`.
  version "0.5.10"
  sha256 arm:   "ae9f7d05062a8eecea386b7992c84f85b25b9122c6dad8480be7d28b40714cb7",
         intel: "b6087654001e499e5b197293850c382eccdc2201e314ea72906413281337daeb"

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

  auto_updates false
  # Matches Info.plist LSMinimumSystemVersion (12.0 = Monterey). Cask's
  # idiomatic form is the symbol alone — implicitly "this or newer".
  depends_on macos: :monterey

  app "LinkPilot.app"

  # launchctl also removes the user's LaunchAgent plist without sudo.
  uninstall launchctl: "app.linkpilot.daemon",
            quit:      "app.linkpilot.desktop"

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
    On first launch the GUI writes ~/Library/LaunchAgents/
    app.linkpilot.daemon.plist pointing at the daemon binary bundled
    inside the .app. From then on, `lpt open <url>` works system-wide
    (even from headless contexts) through that LaunchAgent.

    To uninstall the LaunchAgent (and stop the daemon) without removing
    the .app itself:
      lpt daemon uninstall
  EOS
end
