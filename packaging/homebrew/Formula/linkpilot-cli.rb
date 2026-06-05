# Homebrew formula for the LinkPilot CLI.
#
# Local install (development only):
#   brew install --formula ./packaging/homebrew/Formula/linkpilot-cli.rb
#
# Public install (after this file is pushed to jackerjay/homebrew-linkpilot):
#   brew install jackerjay/linkpilot/linkpilot-cli
#
# Ships two binaries — `lpt` and `linkpilot-daemon`. `lpt` alone would
# work for local-mode commands, but `lpt daemon install` writes a
# LaunchAgent plist whose ProgramArguments points at a daemon binary on
# disk. Bundling both means a CLI-only user can transition between
# local mode and a system-wide launchd-managed daemon without any
# additional download. (The .app cask is the alternative for GUI
# users; it ships the same binaries embedded inside the bundle.)

class LinkpilotCli < Formula
  desc "Per-link router — dispatches URLs to the right browser, profile, workspace"
  homepage "https://github.com/jackerjay/LinkPilot"
  version "0.5.6"
  license "MIT"

  depends_on :macos

  # `#{version}` is NOT in scope inside a `resource` block — there it resolves
  # to the resource's own (nil) version and renders a `…/download/v/…` 404.
  # Capture the release tag in a local the nested blocks close over so every
  # URL — formula and resource alike — uses one source of truth. Hardcoded
  # (not "v#{version}") because `version` isn't reliably readable at class-body
  # scope; bump this together with `version` above on every release.
  release_tag = "v0.5.6"

  # The release.yml matrix builds CLI + daemon natively per arch and
  # publishes one tarball each. No more lipo — Apple-Silicon users get
  # the aarch64 binary, Intel users get x86_64.
  #
  # SHAs are the per-arch tarball hashes from the v0.5.6 release
  # `checksums.txt` (main `lpt` tarball + the `daemon` resource each).
  # `brew style` (FormulaAudit/ComponentsOrder) forbids bare url/sha256
  # directly inside top-level on_arm/on_intel — they must be nested under
  # on_macos. The per-arch daemon resource rides along inside each arch
  # block so its URL tracks the same arch as the main `lpt` tarball.
  on_macos do
    on_arm do
      url "https://github.com/jackerjay/LinkPilot/releases/download/#{release_tag}/lpt-macos-aarch64.tar.gz"
      sha256 "01272d548f77936f0d23a9ba3608ff6fcfe1a92caa617dffe6eb67e66d0a2082"

      resource "daemon" do
        url "https://github.com/jackerjay/LinkPilot/releases/download/#{release_tag}/linkpilot-daemon-macos-aarch64.tar.gz"
        sha256 "1438aa9248da83298ff4b26a6a37915384daa020469ffb11b3cc83f21bb16816"
      end
    end

    on_intel do
      url "https://github.com/jackerjay/LinkPilot/releases/download/#{release_tag}/lpt-macos-x86_64.tar.gz"
      sha256 "5ad76521cf2469b3b3267d8593e8f2b5e902fadc11f1dd40e97af421d2d15730"

      resource "daemon" do
        url "https://github.com/jackerjay/LinkPilot/releases/download/#{release_tag}/linkpilot-daemon-macos-x86_64.tar.gz"
        sha256 "d0830405a88c68e8fbd47ebfec2fe09f79a8398b2b725a9dc7e9fc0021e837bf"
      end
    end
  end

  def install
    # The tarball's top-level entry is named `lpt`; the daemon
    # resource's is `linkpilot-daemon` (release.yml stages each binary
    # under its final name before tarring, so files extract with the
    # name we want here without rename gymnastics).
    bin.install "lpt"
    resource("daemon").stage do
      bin.install "linkpilot-daemon"
    end
  end

  def caveats
    <<~EOS
      LinkPilot ships unsigned, but Homebrew's formula path doesn't
      quarantine downloads, so `lpt` and `linkpilot-daemon` run as-is —
      no `xattr` dance needed (that's only for the .app cask).

      To register LinkPilot as the system default browser handler you
      need the .app (which carries an Info.plist with the right Launch
      Services keys). Install it with:
        brew install --cask jackerjay/linkpilot/linkpilot

      To run the daemon at login (so `lpt open` works system-wide):
        lpt daemon install
    EOS
  end

  test do
    # Pinned to what every release artifact reports — `--version` is
    # the only surface we can rely on across patch bumps. Subcommand
    # assertions belong in the cargo test suite, not here.
    assert_match "lpt #{version}", shell_output("#{bin}/lpt --version")
    assert_match "linkpilot-daemon #{version}",
                 shell_output("#{bin}/linkpilot-daemon --version")
  end
end
