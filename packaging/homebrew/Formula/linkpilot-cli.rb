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
  version "0.5.0"
  license "MIT"

  depends_on :macos

  # The release.yml matrix builds CLI + daemon natively per arch and
  # publishes one tarball each. No more lipo — Apple-Silicon users get
  # the aarch64 binary, Intel users get x86_64.
  #
  # TODO: when v0.5.0 actually ships, replace each `"0" * 64`
  # placeholder with the real SHA from `dist/release/checksums.txt`.
  on_arm do
    url "https://github.com/jackerjay/LinkPilot/releases/download/v#{version}/lpt-macos-aarch64.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"

    resource "daemon" do
      url "https://github.com/jackerjay/LinkPilot/releases/download/v#{version}/linkpilot-daemon-macos-aarch64.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  on_intel do
    url "https://github.com/jackerjay/LinkPilot/releases/download/v#{version}/lpt-macos-x86_64.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"

    resource "daemon" do
      url "https://github.com/jackerjay/LinkPilot/releases/download/v#{version}/linkpilot-daemon-macos-x86_64.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
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
      LinkPilot ships unsigned. Strip the quarantine flag on first run:
        xattr -dr com.apple.quarantine #{bin}/lpt #{bin}/linkpilot-daemon

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
