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
  license "MIT"

  depends_on :macos

  # `#{version}` is NOT in scope inside a `resource` block — there it resolves
  # to the resource's own (nil) version and renders a `…/download/v/…` 404.
  # Capture the release tag in a local the nested blocks close over so every
  # URL — formula and resource alike — uses one source of truth. Homebrew
  # infers the formula version from this release URL, so bump this tag on
  # every release rather than declaring the version a second time.
  release_tag = "v0.5.11"

  # The release.yml matrix builds CLI + daemon natively per arch and
  # publishes one tarball each. No more lipo — Apple-Silicon users get
  # the aarch64 binary, Intel users get x86_64.
  #
  # SHAs are the per-arch tarball hashes from the v0.5.11 release
  # `checksums.txt` (main `lpt` tarball + the `daemon` resource each).
  # `brew style` (FormulaAudit/ComponentsOrder) forbids bare url/sha256
  # directly inside top-level on_arm/on_intel — they must be nested under
  # on_macos. The per-arch daemon resource rides along inside each arch
  # block so its URL tracks the same arch as the main `lpt` tarball.
  on_macos do
    on_arm do
      url "https://github.com/jackerjay/LinkPilot/releases/download/#{release_tag}/lpt-macos-aarch64.tar.gz"
      sha256 "21eef1fc10427f3096b4bf9e7d9008920fdc1705cbf3ccb2bdc1c18b2655728c"

      resource "daemon" do
        url "https://github.com/jackerjay/LinkPilot/releases/download/#{release_tag}/linkpilot-daemon-macos-aarch64.tar.gz"
        sha256 "faaa56a6e9741b6efc39f5f64aaf641d6c0930d04471d36fdbe0898b29c7d1d8"
      end
    end

    on_intel do
      url "https://github.com/jackerjay/LinkPilot/releases/download/#{release_tag}/lpt-macos-x86_64.tar.gz"
      sha256 "e48e38c91828a7dc1ebc8ffad3668fe348d13f6fb0b9609c3ac8ac182af9a6e2"

      resource "daemon" do
        url "https://github.com/jackerjay/LinkPilot/releases/download/#{release_tag}/linkpilot-daemon-macos-x86_64.tar.gz"
        sha256 "c3bc1d1447684ef18907ac5ca3113000dc1fcafd5be9aad23e5cb0c9a469e349"
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
