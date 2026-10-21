# App Store edition

LinkPilot uses one codebase with two distribution builds. The direct download
build keeps its existing profile routing, CLI, daemon and update flow. The Store
build is a sandboxed, browser-only application with its own container and bundle
identifier, `app.linkpilot.store` (local product name: `LinkPilot Store`).

This is a local development-signed Store candidate, not an App Store upload or
an assurance of review acceptance. No Store record, provisioning profile,
distribution package or submission is created by the local build command.

## Build locally

From `apps/desktop` on macOS with Rust, Node, Xcode Command Line Tools and a local
Apple Development signing identity:

```sh
npm run bundle:store             # release
npm run bundle:store -- --debug  # faster local iteration
```

With multiple local identities, set `APPLE_SIGNING_IDENTITY` to the intended
identity's name or SHA-1. The script requires a signing identity; it does not
silently fall back to an unsigned build. It builds no CLI/daemon sidecars, patches
Info.plist before signing, then verifies the completed bundle.

Outputs use a separate Cargo directory:

```text
target/store/release/bundle/macos/LinkPilot Store.app
target/store/debug/bundle/macos/LinkPilot Store.app
```

The build script sets both `VITE_DISTRIBUTION=app-store` and the Rust `app-store`
feature. Tauri receives `--no-default-features` as a Cargo argument. Combining
`app-store` with `direct-distribution` is a compilation error. The Store overlay
also sets `macOSPrivateApi: false`, an explicit capability set and a CSP. Opaque
picker/tray windows replace the direct build's transparent WebKit windows.
At runtime the Store edition checks its own sandbox entitlement before creating
windows or resolving data paths, and rejects the direct edition's bundle ID.
A bare `cargo run` cannot accidentally operate on direct-edition data.

Private window API features are selected in Cargo, rather than through Tauri's
automatic manifest-feature rewriting. The main static window is opaque; dynamic
transparent windows remain enabled by the direct edition's default Cargo
feature. This also keeps plain `cargo check -p linkpilot-desktop` working.

`npm run bundle:mac` and the existing GitHub Release workflow continue to build
the direct edition. Store builds do not replace `/Applications/LinkPilot.app`.

## Supported behavior

- The shared browser registry is resolved through LaunchServices/NSWorkspace,
  without shelling out or reading browsers' data directories.
- Host/path/source-application rules, rule groups, ask, block and browser targets
  use the existing routing engine. Source-application detection retains the
  direct edition's foreground-app heuristic.
- The native URL-opening API opens HTTP/HTTPS links in the selected browser.
- Source-app selection and app icons use public AppKit APIs.
- Configuration import/export uses system JSON file panels. The selected file's
  security scope is active for the operation; no bookmark is persisted. Export
  uses Foundation's atomic write. Invalid/incompatible imports leave the saved
  configuration unchanged.
- Configuration and local observations live inside the Store container. A new
  Store install starts with empty rules and external updates disabled. Existing
  direct-edition configuration is not automatically migrated.

The expected local configuration is:

```text
~/Library/Containers/app.linkpilot.store/Data/Library/Application Support/LinkPilot/linkpilot.config.json
```

## Distribution restrictions

The UI omits unsupported options, and native configuration/launch boundaries
enforce the same rules. Imported or manually edited configurations cannot
silently enable them:

- No profile selection/enumeration, private mode or new-window launch flags.
- No source-browser/source-profile matchers or keep-source action, which require
  browser integration. No browser extension integration is added.
- Custom browser entries are deferred; the shared known-browser registry is
  supported. No per-browser profile data access is requested.
- No LaunchAgent, external daemon/IPC server, CLI installation or launch-at-login
  setting. The application must remain running to provide the menu-bar UI; URL
  handler registration lets macOS launch it when selected by the user.
- No GitHub update checks or DMG downloads; Store updates belong to App Store.

Entitlements are App Sandbox, network client (required for WKWebView even when
displaying local assets), and user-selected read/write files for explicit config
import/export. There is no full-disk access, browser-data entitlement, temporary
sandbox exception, automation entitlement or unsandboxed helper.

## Verification

The desktop/native Store tests check target restrictions, nested unsupported
matchers, Unicode bridging and preservation of configuration after a rejected
write. CI builds both frontend variants and both app bundles, and signs the
Store smoke-test bundle ad hoc solely to verify its sandbox seal.

Local UI checks must distinguish a successful launch callback from an actual
browser tab. Test a default target and a different browser selected by a rule,
match the unique test URL in the destination browser, preview the ask chooser,
round-trip a disposable JSON configuration, and quit/reopen the same app to
verify its configuration persists. Do not change the production default browser
or production LinkPilot configuration as part of these local checks.

Verified locally on 2026-10-21, macOS 27.0.1, Apple Silicon:

- The development-signed release app launches under the sandbox with its own
  container and discovers Chrome, Brave and Safari.
- Chrome receives the fallback-target URL; a saved `example.com` rule sends a
  different test URL to Brave; the browser chooser sends a third URL to Safari.
  All three were verified in the actual destination browser UI.
- Native JSON export and compatible import succeed. An import with a profile
  target displays an error and preserves the original config byte-for-byte;
  retrying with compatible JSON succeeds.
- Quitting and reopening the same release app preserves the fallback browser and
  saved rule. No additional file grants are requested on launch.
- Store desktop tests (23), native bridge tests (3), core tests (51 unit plus
  one integration), frontend builds and code-signature verification pass. The
  direct desktop Cargo check also passes. The Store feature graph contains no
  `macos-private-api` feature.

The local Store container retains the one synthetic `example.com → Brave`
verification rule and Chrome fallback for inspection. The signed app was not
copied into Applications and the system default browser was not changed. CI
changes are source configuration; a remote CI run and Store submission have not
been performed by these local checks.

## Before uploading

1. Confirm the public app identity/name and register `app.linkpilot.store` in
   Apple Developer and App Store Connect.
2. Obtain a Mac App Store application distribution certificate and a matching
   **Mac App Store Connect** provisioning profile. Developer ID signing and
   notarization credentials used by GitHub Releases do not replace these.
3. Embed the profile as `Contents/embedded.provisionprofile`, and add its exact
   `com.apple.application-identifier` and `com.apple.developer.team-identifier`
   values to the final distribution entitlements. Sign after all bundle edits.
4. Review the privacy manifest/required-reason API declarations, privacy policy,
   support URL, screenshots, pricing and App Store listing. The Store Info.plist
   declares no non-exempt encryption; confirm the export-compliance answers
   against the final shipped dependencies before submission.
5. Package the distribution-signed release app with `productbuild`, using a
   **Mac Installer Distribution** identity. Upload the signed `.pkg` through
   Transporter or Apple's authenticated upload tooling, then validate through
   TestFlight/App Review. Keep private keys and API credentials out of Git.

Only the current local architecture is built by `bundle:store`; Intel/universal
Store packaging and distribution signing remain separate validation steps.
Changing the system default browser, cold-start URL delivery, login behavior,
full Mac reboot and App Store validation require their own acceptance checks.

References:

- [Tauri App Store distribution](https://v2.tauri.app/distribute/app-store/)
- [Apple App Sandbox configuration](https://developer.apple.com/documentation/xcode/configuring-the-macos-app-sandbox)
- [NSWorkspace launch arguments sandbox restriction](https://developer.apple.com/documentation/appkit/nsworkspace/openconfiguration/arguments)
- [Earlier profile-access probe](../tools/sandbox-probe/RESULTS.md)


## Store distribution package

The Store record is [LinkPilot - Browser Router](https://appstoreconnect.apple.com/apps/6825348952/distribution),
Apple ID `6825348952`, bundle ID `app.linkpilot.store`, team `348D74H42G`.
Its first candidate is version `0.5.10`, build `1`, for Apple Silicon and macOS 12+.
The shorter name `LinkPilot` is already registered by another App Store app.

After `npm run bundle:store`, package a copy of the tested bundle with:

```sh
npm run package:store -- \
  --app '/absolute/path/to/LinkPilot Store.app' \
  --profile '/secure/path/LinkPilot-Store.provisionprofile' \
  --application-identity '<40-character certificate SHA-1>' \
  --installer-identity '<40-character certificate SHA-1>' \
  --keychain '/secure/path/store-signing.keychain-db' \
  --build-number 1 \
  --output '/absolute/path/to/new/submission-directory'
```

The package script validates the distribution profile's platform, expiry, team,
bundle ID and application certificate. It rejects device-bound, debug-enabled
and Developer ID profiles. It verifies both Store signing identities, embeds the
profile, sets the build number, applies the minimal validated entitlements, signs
the copied app, creates a signed installer, verifies the signatures and writes a
SHA-256 checksum. It never replaces an existing submission directory or the
local development bundle. The optional keychain argument keeps signing keys in
a dedicated local keychain without changing the user's keychain search list.

The Store-only privacy manifest declares no tracking or transmitted collected
data. File timestamp APIs are used for files in the app container/bundle. The
[privacy policy](privacy.md) describes local routing observations and explicit
configuration import/export. Keep the public policy synchronized with behavior.

The manual `App Store upload` workflow downloads one signed package from a draft
release named `app-store-candidate-<version>-<build>`, verifies its expected
SHA-256, asks Apple to validate it, then uploads it with the existing `APPLE_ID`
and `APPLE_PASSWORD` Actions secrets. It does not export Store private keys to
GitHub or publish the draft as a public release. Use a fresh build number and
candidate tag for each subsequent submission. A successful upload still needs
App Store Connect processing, metadata completion and App Review submission.
