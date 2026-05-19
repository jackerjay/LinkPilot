# Homebrew formula for the LinkPilot CLI.
#
# Local install (development only):
#   brew install --formula ./packaging/homebrew/Formula/linkpilot-cli.rb
#
# Public install (after M6 pushes this file to jackerjay/homebrew-linkpilot):
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
  # The release.yml universal-binary pipeline lipos x86_64 + aarch64 into
  # one Mach-O. One tarball serves both Apple-Silicon and Intel hosts.
  url "https://github.com/jackerjay/LinkPilot/releases/download/v0.2.0-alpha.3/lpt-macos.tar.gz"
  version "0.2.0-alpha.3"
  sha256 "257620b1ff016bbc4fe8dd95f7fc279524b65a6959d90de6c308f3a341832558"
  # The repo dual-licenses MIT/Apache-2.0; Homebrew wants this expressed
  # as an `any_of:` array rather than the SPDX "MIT OR Apache-2.0" string.
  license any_of: ["MIT", "Apache-2.0"]

  depends_on :macos

  resource "daemon" do
    url "https://github.com/jackerjay/LinkPilot/releases/download/v0.2.0-alpha.3/linkpilot-daemon-macos.tar.gz"
    sha256 "09821a8580bbb1e146a3a751c8f8835bbda34dafd5554e4ad6a450c9a3e4fba8"
  end

  def install
    # The tarball's payload is named `lpt-macos`; the daemon resource's is
    # `linkpilot-daemon` (the M3.2 fix in release.yml stages the binary
    # under that final name before tarring, so the file extracts with the
    # name we want here without rename gymnastics).
    bin.install "lpt-macos" => "lpt"
    resource("daemon").stage do
      bin.install "linkpilot-daemon"
    end
  end

  def caveats
    <<~EOS
      LinkPilot ships unsigned. Strip the quarantine flag on first run:
        xattr -dr com.apple.quarantine #{bin}/lp #{bin}/linkpilot-daemon

      To register `lpt` as the system default browser handler you need the
      .app (which carries an Info.plist with the right Launch Services
      keys). Install it with:
        brew install --cask jackerjay/linkpilot/linkpilot

      To run the daemon at login (so `lpt open` works system-wide):
        lpt daemon install
    EOS
  end

  test do
    # Pinned to what every v0.2 release artifact reports — `--version`
    # is the only surface we can rely on across patch bumps. Subcommand
    # assertions belong in the cargo test suite, not here.
    assert_match "lpt #{version}", shell_output("#{bin}/lp --version")
    assert_match "linkpilot-daemon #{version}",
                 shell_output("#{bin}/linkpilot-daemon --version")
  end
end
