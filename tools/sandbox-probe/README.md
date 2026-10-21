# macOS sandbox profile probe

Local feasibility app for the App Store distribution investigation. It uses an
independent bundle identifier, app container and local development signature:
`app.linkpilot.sandbox-probe`.

**Local result: profile reading and bookmark restoration passed; profile-specific
launch through `NSWorkspace.OpenConfiguration.arguments` failed.** Ordinary
HTTPS URL delivery to Chrome and Brave passed through the native URL-opening
API. See [RESULTS.md](RESULTS.md) for the evidence and remaining decision.

The native AppKit shell links the existing `linkpilot-core` Chromium profile
parser as a static library. Profile reading occurs in the sandboxed process;
there is no unsandboxed helper, shell process or temporary filesystem exception.
The bridge returns profile IDs and display names, excluding account email and
avatar metadata. Browser files are never written.

## Build

Requirements: macOS, Rust, Node and Xcode Command Line Tools. The same local
Apple Development identity used by the desktop build is selected automatically
when there is exactly one. With multiple identities, set
`APPLE_SIGNING_IDENTITY` explicitly. This build is for local testing only.

```sh
cargo test --locked -p linkpilot-sandbox-probe
cargo clippy --locked -p linkpilot-sandbox-probe --all-targets -- -D warnings
bash tools/sandbox-probe/build.sh
open "target/sandbox-probe/LinkPilot Sandbox Probe.app"
```

The app is built beneath Cargo's configured target directory. It does not
install into Applications, register a URL handler, replace the installed
LinkPilot app or change its LaunchAgent/configuration.

## Runtime proof

1. On launch, the app attempts to read the public `ungranted.txt` fixture next
   to its bundle. `sandbox-control-denied` must appear. Any other outcome
   disables authorization and browser launch, so an ineffective sandbox cannot
   produce a false positive.
2. For Chrome or Brave, select **Authorize Local State** and choose that
   browser's exact `Local State` file. The file picker grants read-only access;
   the app creates a read-only, app-scoped security bookmark. No parent folder
   or whole-disk entitlement is requested.
3. Confirm `profiles-read` shows the expected count and profile IDs. Errors
   remain visible and allow another authorization attempt.
4. Quit and reopen the same signed app without rebuilding or changing its
   bundle identifier. A new process must report `profiles-read` with
   `restored: true`, without showing another file picker. Stale bookmarks are
   refreshed only after the existing grant is successfully activated.
5. Confirm `profile-launch-unsupported` appears. Apple's API contract says that
   `NSWorkspace.OpenConfiguration.arguments` is ignored for sandboxed callers.
   `createsNewApplicationInstance = true` does not remove that restriction.
   The original launch experiment produced a successful callback but no test
   tab, so the current probe explicitly reports this limitation.
6. Click **验证浏览器打开** for each browser. The public
   `NSWorkspace.open(_:withApplicationAt:configuration:completionHandler:)`
   API sends an ordinary HTTPS example.com URL with a unique fragment.
   Inspect the actual browser tab and match its URL to `browser-open-request`.
   `browser-open-submitted` alone is not delivery proof. The profile list is
   available for inspection; selecting an entry does not target that profile.

Chrome file:
`~/Library/Application Support/Google/Chrome/Local State`

Brave file:
`~/Library/Application Support/BraveSoftware/Brave-Browser/Local State`

The picker locates the real account directory through the system account
database. Foundation's home-directory APIs return the app container inside a
sandbox. Locating the path does not grant file access; the exact selected file
must still pass path validation and activate its security scope before reading.

Each process writes `run-<pid>.json` inside its container's
`Library/Application Support/LinkPilotSandboxProbe/` directory. Reports contain
metadata paths, counts, profile IDs and URL-opening requests, not account email,
bookmark bytes or browser file contents. Keep these machine-specific reports
local. Bookmarks stay in the app's sandboxed preferences.
Quitting/reopening tests process restart; a full macOS reboot is a separate
acceptance check.

## Limits

This probe tests profile access and browser launch under the real App Sandbox.
Profile enumeration does not establish profile-specific URL routing. The
`NSWorkspace` launch-argument path is unsupported for sandboxed callers; an
alternative browser-supported integration needs its own feasibility check.
It is not an App Store build of the full Tauri application. It does not prove
Tauri window compatibility, background service lifecycle, configuration
migration, receipt handling or App Store review acceptance. In particular, a
local development signature does not replace an App Store distribution
certificate and provisioning profile.

Authorizing this app is separate from the installed LinkPilot app's existing
Chrome/Brave privacy permissions. Scope any authorization to this probe and
the two metadata files explicitly selected by the user.

## Apple API references

- [Sandbox file access](https://developer.apple.com/documentation/security/accessing-files-from-the-macos-app-sandbox)
- [Read-only security bookmarks](https://developer.apple.com/documentation/foundation/nsurl/bookmarkcreationoptions/securityscopeallowonlyreadaccess)
- [New application instances](https://developer.apple.com/documentation/appkit/nsworkspace/openconfiguration/createsnewapplicationinstance)
- [Launch arguments and the sandbox restriction](https://developer.apple.com/documentation/appkit/nsworkspace/openconfiguration/arguments)
- [Opening URLs in a specified application](https://developer.apple.com/documentation/appkit/nsworkspace/open(_:withapplicationat:configuration:completionhandler:))
