# Direct-distribution updates

Starting with 0.5.12, the GitHub/Homebrew app checks on startup and every six
hours while running. The existing `settings.auto_check_updates` preference
controls download and silent installation. It defaults to true. Manual Check
downloads only; Install and restart explicitly retries a failed installation.
The App Store edition neither links the installer nor permits these commands.

The first upgrade from an older version must still be installed manually.
No new signing key, update server or release asset is required: releases keep
shipping the signed, notarized architecture-specific DMGs and checksums.

## Trust and installation

The downloader checks the published SHA-256. Before mounting the DMG, the
installer independently requires Apple's Developer ID policy, team
`348D74H42G`, and Gatekeeper assessment. The copied app must additionally
have identifier `app.linkpilot.desktop`, the exact requested stable version,
a newer semantic version, and the native architecture. Modified or unsigned
payloads are refused. Validation does not trust a renderer-provided hash alone.

The installer creates a private workspace beside the installed app. A copied
`lpt` worker waits for the old GUI to exit, swaps the bundles atomically on the
same filesystem, and launches the new executable in the background. A ready
receipt must match the job token, version and exact child PID within 30 seconds.
Only then is the old bundle removed. Rules, profiles and user config stay in
their existing Application Support directory.

An active Ask picker postpones installation. Turning automatic updates off
during preparation cancels the handoff. The worker restarts the background
daemon only when a loaded LaunchAgent points inside the updated bundle; a
separate CLI daemon and unloaded services are preserved.

## Failure and recovery

No administrator prompt is used. A read-only app directory, failed signature
check or staging failure leaves the installed app untouched. Settings retains
the DMG fallback. A launch failure, early process exit, missing receipt or
daemon failure swaps back and restarts the previous version. Automatic retries
of the failed version pause until an explicit retry or a different release.

Worker diagnostics and the last outcome are stored beside the config under
`updates/worker.log` and `updates/outcome.json`. Failed worker/rollback files
are retained in `.linkpilot-update-*` beside the app. A failed rollback names
the recovery location; do not delete that folder before restoring the old app.
Atomic swap keeps the installation path present if the worker or machine is
interrupted, but rollback after a worker crash or power loss is not automatic.

## Verification

`cargo test -p linkpilot-updater` exercises real filesystem swaps and child
processes for readiness, timeout, rollback, concurrency and job validation.
`cargo test -p linkpilot-cli --test update_worker` exercises the copied worker
after its fixture parent exits, signature refusal and recovery launch.
`npm run test:updates` in `apps/desktop` checks automatic/manual/off flows.
To verify a real release without launching it or changing the installed app:

```sh
LINKPILOT_TEST_DMG=/absolute/path/LinkPilot_0.5.11_aarch64.dmg \
  cargo test -p linkpilot-updater signed_release_authentication_and_tamper_rejection -- --ignored
```
