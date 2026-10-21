# macOS release signing

GitHub Releases can host signed macOS downloads. GitHub does not sign or
notarize them: the Release workflow does that before uploading any assets.
An Apple Development certificate works for local development but does not
replace a Developer ID Application certificate for public distribution.

## GitHub Actions secrets

In `jackerjay/LinkPilot` → Settings → Secrets and variables → Actions, set:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE_BASE64` | Base64 of a password-protected `.p12` export containing the Developer ID Application certificate, its private key, and the complete Apple certificate chain |
| `APPLE_CERTIFICATE_PASSWORD` | Password protecting that `.p12` export |
| `APPLE_ID` | Apple Account email used for notarization |
| `APPLE_PASSWORD` | App-specific password for that account, not its login password |
| `APPLE_TEAM_ID` | Developer team ID that owns the certificate |

Create the Developer ID Application certificate in your own Apple Developer
account and export that identity from Keychain Access. Never use another
team's certificate. Keep the `.p12`, its password, and notarization credentials
out of Git, Release assets, chat messages, and workflow logs. Base64 encodes
the certificate; GitHub Secrets provides the secret storage.

Include the Apple Developer ID intermediate and Apple Root CA certificates
in the export. An incomplete chain can make `codesign` fail even when the
identity appears valid. The runner adds its temporary keychain to the search
list before looking up the identity and signing.

The Release workflow requires all five secrets and exactly one valid
Developer ID Application identity in the `.p12`. It fails before building if
credentials are missing or invalid; there is no unsigned fallback. PR builds
use no signing secrets. Do not trigger a release tag until secrets are ready.

## Release order

1. Build the app, CLI, and daemon; embed the helpers and patch `Info.plist`.
2. Sign helpers and the complete app with hardened runtime and a secure
   timestamp. Copy the signed helpers back for standalone downloads.
3. Notarize an archive of the app, including both helpers. Require Apple's
   `Accepted` status, print its diagnostics, and staple and validate the app's
   ticket. Verify Gatekeeper accepts the app.
4. Build a DMG from that final app, sign and notarize the DMG, staple and
   validate its ticket, and verify Gatekeeper accepts the DMG.
5. Package standalone binaries and calculate checksums only after signing
   and notarization. Upload assets, then publish the GitHub Release.

The temporary runner keychain is deleted even on failure. Standalone CLI and
daemon downloads contain the same signed bytes notarized inside the app.
Apple does not support stapling tickets to standalone executables, so their
Gatekeeper verification may require network access.

Existing published releases (including v0.5.8) remain unsigned. Immutable
GitHub Release assets cannot be replaced; ship a new version and tag to
distribute the fix. Update Homebrew versions and checksums to that new release
after publication. The current v0.5.8 cask still handles its unsigned assets.

`npm run bundle:mac` remains a separate local build: it can select your Apple
Development identity and does not submit anything to Apple.

## References

- [Apple: Developer ID certificates](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/)
- [Apple: Notarizing macOS software before distribution](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- [Apple: Customizing the notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow)
- [GitHub: Installing an Apple certificate on macOS runners](https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications)
