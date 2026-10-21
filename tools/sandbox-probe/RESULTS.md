# Sandbox feasibility result

Verified locally on 2026-10-21, Apple Silicon, macOS 27.0.1 (26A434), using the
independent `app.linkpilot.sandbox-probe` app with a local Apple Development
signature. These results do not establish Mac App Store review acceptance.

## Outcome

| Capability | Result | Evidence |
| --- | --- | --- |
| Actual App Sandbox isolation | Passed | Reading an ungranted sibling fixture failed with Cocoa error 257 on each launch. |
| Chrome profile metadata | Passed | The production Rust parser read 10 profiles after authorization of the exact `Local State` file. |
| Brave profile metadata | Passed | The same parser read 1 profile after authorization of Brave's exact `Local State` file. |
| Authorization after process restart | Passed | Process 13613 saved both bookmarks; process 15410 restored both without another picker or rebuild. |
| Stale bookmark renewal | Observed working | Process 19199 reported a stale Chrome bookmark, activated the existing grant, read profiles, and saved a renewed bookmark without another picker. |
| Open an HTTPS URL in Chrome | Passed | Process 19855 sent a unique URL through `NSWorkspace.open`; the matching URL appeared in a Chrome tab. |
| Open an HTTPS URL in Brave | Passed | The same probe sent a distinct URL to Brave; the matching URL appeared in Brave's Example Domain window. |
| Select a browser profile using launch arguments | Failed / unsupported API path | The original `openApplication` experiment returned a callback but no test tab. Apple's API documentation explicitly says sandboxed callers' `arguments` are ignored. |

Per-process reports remain local under:

```text
~/Library/Containers/app.linkpilot.sandbox-probe/Data/Library/Application Support/LinkPilotSandboxProbe/run-<pid>.json
```

Raw browser files, account names, emails and security bookmarks are not copied
into this repository. The Rust bridge returns only profile IDs and display
names; runtime reports omit display names and account metadata.

## Findings

The first file selection failed the probe's own exact-path check because
Foundation resolved the user's home directory to the sandbox container. The
probe now uses the system account directory to locate and validate the selected
file. This does not expand filesystem access: user-selected read-only access and
an active security scope remain required. The negative control still fails.

The launch experiment used `createsNewApplicationInstance = true` and passed
`--profile-directory`, a version page and a unique HTTPS URL as `arguments`.
Neither the profile flag nor the URLs could be relied on: the sandbox ignores
that property. A launch callback is therefore insufficient proof of routing.

The current probe uses the native URL-opening API for the browser-only baseline
and visibly reports `profile-launch-unsupported`. This is not a substitute for
the original selected-profile acceptance condition. Passing both an internal
`chrome://version` URL and an HTTPS URL in the native URL API also failed in the
local Chrome test; the baseline is deliberately restricted to ordinary HTTPS.

## Scope and next decision

Chrome and Brave are two Chromium-family representatives. A production sandbox
target should share the existing browser registry and family parsers. Firefox's
`profiles.ini` access and launch behavior, other Chromium products/channels,
custom browsers and Arc-specific routing remain unverified.

The tested file-access design is viable. An equivalent App Store edition still
needs a different, browser-supported path for selected-profile routing. Browser
extension cooperation is a candidate for a separate feasibility check, not a
verified solution. A browser-only edition would reduce existing product
functionality and requires an explicit product decision.

A full Mac reboot, full Tauri sandbox build, configuration migration, background
service behavior and App Store distribution signing/review were not tested.

## Checks

- Rust bridge: 4 behavior tests passed.
- Clippy with warnings denied, workspace formatting and shellcheck passed.
- Native Swift build and deep/strict code-signature verification passed.
- Embedded entitlements are limited to App Sandbox, user-selected read-only
  files and app-scoped bookmarks.

Reference: Apple's [launch arguments documentation](https://developer.apple.com/documentation/appkit/nsworkspace/openconfiguration/arguments)
states: “If the calling process is sandboxed, the system ignores the value of
this property.”
