# Homebrew cask for the LinkPilot GUI.
#
# Local install (development only):
#   brew install --cask ./packaging/homebrew/Casks/linkpilot.rb
#
# Public install (after M6 pushes this file to jackerjay/homebrew-linkpilot):
#   brew install --cask jackerjay/linkpilot/linkpilot
#
# Ships the .app bundle from the release DMG. The .app carries `lp` and
# `linkpilot-daemon` embedded in Contents/MacOS — the GUI's first-run
# hook writes the LaunchAgent plist pointing into the bundle, and the
# Settings "Install lp on PATH" button symlinks the embedded `lp` to
# ~/.local/bin. Users who want `lp` on PATH without the GUI install the
# separate `linkpilot-cli` formula instead.

cask "linkpilot" do
  version "0.2.0-alpha.3"
  sha256 "b443fddeb2996383ada8c50dc332ed298dcfe3e3bf96af48616b7db5e6dfea21"

  url "https://github.com/jackerjay/LinkPilot/releases/download/v#{version}/LinkPilot_#{version}_universal.dmg"
  name "LinkPilot"
  desc "Route every link to the right browser, profile, and workspace"
  homepage "https://github.com/jackerjay/LinkPilot"

  # Unsigned build for v0.2-alpha; Gatekeeper would otherwise quarantine
  # the .app on first launch. Cask flips the quarantine flag at install.
  auto_updates false
  depends_on macos: ">= :ventura"

  app "LinkPilot.app"

  uninstall launchctl: "app.linkpilot.daemon",
            quit:      "app.linkpilot.desktop",
            delete:    "~/Library/LaunchAgents/app.linkpilot.daemon.plist"

  # `zap` is opt-in deep clean (brew uninstall --zap). Keeps these on a
  # plain uninstall so a reinstall preserves the user's rules, history,
  # and routing logs.
  zap trash: [
    "~/Library/Application Support/LinkPilot",
    "~/Library/Logs/LinkPilot",
    "~/Library/Caches/app.linkpilot.desktop",
    "~/Library/Preferences/app.linkpilot.desktop.plist",
    "~/Library/Saved Application State/app.linkpilot.desktop.savedState",
  ]

  caveats <<~EOS
    LinkPilot ships unsigned in v0.2-alpha. macOS may prompt you to
    confirm the developer on first launch. If you'd rather skip the
    prompt:
      xattr -dr com.apple.quarantine /Applications/LinkPilot.app

    On first launch the GUI writes ~/Library/LaunchAgents/
    app.linkpilot.daemon.plist pointing at the daemon binary bundled
    inside the .app. From then on, `lp open <url>` works system-wide
    (even from headless contexts) through that LaunchAgent.

    To uninstall the LaunchAgent (and stop the daemon) without removing
    the .app itself:
      lp daemon uninstall
  EOS
end
