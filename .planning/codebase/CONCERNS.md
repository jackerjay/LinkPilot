# Codebase Concerns

**Analysis Date:** 2026-06-05

---

## Tech Debt

**CLI `main.rs` monolith (2276 lines):**
- Issue: All 34+ subcommand handlers live in a single file with no module splitting.
- Files: `crates/cli/src/main.rs`
- Impact: Every change to any CLI command requires reading the full file for context; high merge-conflict risk; violates the project's own 800-line file limit.
- Fix approach: Split into per-subcommand modules under `crates/cli/src/cmd/` (e.g., `rules.rs`, `workspaces.rs`, `daemon.rs`).

**Tauri commands monolith (1388 lines, 34 commands):**
- Issue: All Tauri `#[tauri::command]` handlers live in a single `commands/mod.rs` with no subdirectories.
- Files: `apps/desktop/src-tauri/src/commands/mod.rs`
- Impact: Same as CLI: high cognitive overhead per change; hard to navigate.
- Fix approach: Split by domain under `src-tauri/src/commands/` (e.g., `rules.rs`, `browsers.rs`, `settings.rs`, `history.rs`).

**Large React pages with no component extraction:**
- Issue: `OnboardingFlow.tsx` (1345 lines), `rules.tsx` (1267 lines), `HaloPreview.tsx` (1008 lines), `settings.tsx` (850 lines) are all over-limit.
- Files: `apps/desktop/src/onboarding/OnboardingFlow.tsx`, `apps/desktop/src/pages/rules.tsx`, `apps/desktop/src/picker/halo/HaloPreview.tsx`, `apps/desktop/src/pages/settings.tsx`
- Impact: Hard to review; any diff touches hundreds of lines; tests impossible without splitting.
- Fix approach: Extract step components for `OnboardingFlow`, extract `RuleRow`/`MatcherEditor` from `rules.tsx`, split halo geometry rendering from interaction logic in `HaloPreview.tsx`.

**`RouteHistory` is memory-only with no IPC endpoint:**
- Issue: `RouteHistory` lives only in the daemon's in-process ring buffer (capped at 1000 entries, no persistence). The IPC protocol has a `RouteHistory` verb but `lpt history` subcommand is explicitly absent because there is no IPC endpoint wired.
- Files: `crates/core/src/history.rs`, `crates/core/src/protocol.rs` (line 53 — verb defined but not served in CLI), `CLAUDE.md` (documents as intentionally absent)
- Impact: History is invisible to CLI users; daemon restart wipes all routing history; the `RouteHistorySnapshot` response type exists but the CLI path is a dead end.
- Fix approach: Wire the `Request::RouteHistory` verb to the daemon's `RouteHistory::recent()` in `DaemonRuntime::handle()`, then add `lpt history` in `crates/cli/src/main.rs`.

**Version bump touches six files with no tooling:**
- Issue: Releasing a new version requires manually editing `Cargo.toml`, `Cargo.lock` (via `cargo update --workspace`), `apps/desktop/package.json`, `apps/desktop/package-lock.json`, `apps/desktop/src-tauri/tauri.conf.json`, and `packages/config-dsl/package.json`. Missing any one of these has caused release failures (config-dsl wedged npm publish through v0.5.1).
- Files: All six listed above; documented in `AGENTS.md` "Bumping the version" section.
- Impact: Error-prone; easy to ship a mismatched version string.
- Fix approach: A small `scripts/bump-version.sh` or a `cargo xtask bump-version` that updates all six atomically.

**`config-dsl` DSL roundtrip test is skippable:**
- Issue: `crates/core/tests/dsl_roundtrip.rs` silently skips when `bun` is not on PATH, meaning CI on environments without Bun passes without actually testing the DSL.
- Files: `crates/core/tests/dsl_roundtrip.rs` (line 44 — explicit skip message)
- Impact: DSL compile bugs are not caught on Linux CI (frontend job) which has no Bun installed.
- Fix approach: Add a `bun` installation step to the CI `rust` job, or use a Node-based runner.

---

## Known Bugs

**CPU spike in Trae editor — open investigation:**
- Symptoms: Opening the linkpilot project in the Trae editor causes a sustained CPU spike. The extension host restarts every ~3 minutes (`19:52:10`, `19:55:18`, `19:58:29`, `20:01:36`). Each restart logs `SyntaxError: Unexpected end of JSON input`, then reactivates GitLens and ESLint.
- Files: `debug-linkpilot-cpu-spike.md` (open investigation document at repo root); root cause is a stale git worktree at `.claude/worktrees/gifted-sutherland-eee2d1` which repeatedly emits `ENOENT: .git/worktrees/gifted-sutherland-eee2d1/refs/remotes/origin/main`.
- Trigger: Open the project in Trae while the `.claude/worktrees/gifted-sutherland-eee2d1` worktree directory exists.
- Workaround: Remove the stale worktree directory (`git worktree prune` or manually `rm -rf .claude/worktrees/gifted-sutherland-eee2d1`). The `.gitignore` excludes `worktrees` but the directory is physically present on disk, causing the git extension to repeatedly try and fail to resolve it.

**Opener detection is a polling heuristic, not an Apple Events source:**
- Symptoms: The source-app shown in route history / ask-picker for a URL may occasionally be wrong (shows the previous frontmost app rather than the true opener) when two apps open URLs in rapid succession, or when the user does not briefly focus the opening app.
- Files: `crates/platform-mac/src/opener.rs` (polling interval: 750ms, stale threshold: 30s)
- Trigger: Two apps open URLs within 750ms; or a background app (e.g., a notification click) opens a URL without becoming frontmost.
- Workaround: None at user level; this is a known limitation of the Apple Events architecture (sender is dropped before the URL arrives in the deep-link handler). Documented in the `opener.rs` module comment.

---

## Security Considerations

**Unsigned, unnotarized release binaries:**
- Risk: `LinkPilot.app`, `lpt`, and `linkpilot-daemon` are not code-signed or notarized. macOS Gatekeeper quarantines them on download. The Homebrew cask's `postflight` silently strips `com.apple.quarantine` for cask installs; direct DMG downloads require `xattr -dr com.apple.quarantine LinkPilot.app` by hand.
- Files: `packaging/homebrew/Casks/linkpilot.rb` (postflight strips quarantine); `AGENTS.md` and `CLAUDE.md` document the manual step; `.github/workflows/release.yml` produces unsigned artifacts.
- Current mitigation: Homebrew cask postflight handles the quarantine removal for cask users and shows a caveat. Direct DMG users see a "damaged app" dialog on first launch.
- Recommendations: Obtain an Apple Developer ID Certificate and codesign + notarize via `xcrun notarytool` in `release.yml`. Until then, the cask's postflight is mandatory and its `caveats` block is the only user communication.

**IPC socket is world-accessible (Unix permissions):**
- Risk: The daemon IPC socket (`~/Library/Application Support/LinkPilot/linkpilot.sock`) uses the OS default socket permissions. Any local process running as the same user — or any process with access to the user's home directory — can issue arbitrary IPC commands (open URLs as the user, modify config, read routing history).
- Files: `crates/ipc/src/server.rs` (socket creation), `crates/core/src/endpoint.rs`
- Current mitigation: The socket is under `~/Library/...` which is not world-readable by default on macOS, but there is no explicit `chmod 0600` applied after `bind()`.
- Recommendations: Set restrictive permissions on the socket file after binding (mode 0600) to prevent other local users on a shared machine from connecting. Low priority for single-user installs but relevant in enterprise/shared-mac scenarios.

**No secrets or API keys detected in source code:**
- Files scanned: all `*.rs`, `*.ts`, `*.tsx`, `*.json` under `crates/`, `apps/`, `packages/`
- Result: No hardcoded credentials found. The `NPM_TOKEN` GitHub Actions secret is referenced by name only in `.github/workflows/npm-publish.yml`, never inlined.

---

## Performance Bottlenecks

**Blocking `std::thread::sleep` in fsnotify callback:**
- Problem: The fsnotify watcher fires in a background thread; the event handler calls `std::thread::sleep(Duration::from_millis(30))` before reading the file, to let the atomic-rename settle. This blocks the notify thread for every file event — including events for unrelated files in the same directory.
- Files: `crates/core/src/config/store.rs` (line 222)
- Cause: Notify can fire before the writer finishes on an atomic rename; the sleep is a manual debounce. The watcher uses `RecursiveMode::NonRecursive` to limit scope but still watches the whole config directory.
- Improvement path: Use a channel + a separate debouncer thread (e.g., `notify-debouncer-mini` crate) rather than sleeping inside the callback.

**Foreground-app polling loop (750ms):**
- Problem: `MacOpenerDetector` runs an infinite `std::thread::sleep(POLL_INTERVAL)` loop at 750ms to sample `frontmostApplication`. This is a background thread that runs for the entire lifetime of the process.
- Files: `crates/platform-mac/src/opener.rs` (lines 68–82)
- Cause: Apple Events drop the sender; polling is the only available heuristic.
- Improvement path: Use `NSWorkspace` notifications (`NSWorkspaceDidActivateApplicationNotification`) instead of polling. This is event-driven and eliminates the background thread.

**`with_document` closure holds the `ConfigStore` mutex:**
- Problem: `ConfigStore::with_document()` runs an arbitrary closure while holding the store's `Mutex<State>`. The comment explicitly warns that every other `document()` / `replace()` call blocks while the closure runs. On the hot URL-dispatch path this is called synchronously.
- Files: `crates/core/src/config/store.rs` (lines 151–154)
- Cause: Performance optimization to avoid cloning the full `ConfigDocument` on every URL open.
- Improvement path: An `RwLock` would allow multiple concurrent readers on the hot path; the closure pattern stays but contention drops.

---

## Fragile Areas

**Info.plist patch dance — easily broken by naive Tauri usage:**
- Files: `apps/desktop/scripts/patch-info-plist.sh`, `.github/workflows/release.yml` (lines 84–97)
- Why fragile: `tauri build --bundles dmg` would re-bundle the `.app` from scratch, clobbering the Info.plist patches required for macOS "Default web browser" registration. This has already burned the project once. The release workflow deliberately avoids `--bundles dmg` and uses `create-dmg` instead.
- Safe modification: Never pass `--bundles dmg` directly. The only safe DMG path is: `tauri build --bundles app` → `patch-info-plist.sh` → `create-dmg`. Local `bundle:mac` script (`apps/desktop/package.json`) correctly follows this order.
- Test coverage: No automated check verifies that the Info.plist contains the required keys post-patch. Any change to `patch-info-plist.sh` must be manually verified with `plutil -p LinkPilot.app/Contents/Info.plist`.

**Anti-echo token can misclassify External writes as Echo:**
- Files: `crates/core/src/config/store.rs` (lines 287–293)
- Why fragile: The anti-echo logic compares the UUID token stamped on the last `persist()` call with the token found in the freshly-read file. If two rapid writes happen (CLI + GUI within the debounce window), the second write may see the first write's token and misclassify the second change as `Echo`, silently dropping the `config-changed` event.
- Safe modification: Any code path that calls `ConfigStore::replace()` or `persist()` must do so through the store — never write the config file directly from outside the store.
- Test coverage: 5 unit tests in `crates/core/src/config/store.rs` cover the basic token logic, but the race scenario (two concurrent writers) is not tested.

**Release tag immutability — workflow fix on `main` never helps an in-flight tag:**
- Files: `.github/workflows/release.yml`
- Why fragile: GitHub reads the workflow file from the tagged commit. A fix pushed to `main` after a tag has been pushed will not be used by a re-run of that tag's release job. Recovery requires finishing the release by hand (`gh release edit <tag> --draft=false`) and rolling forward with a new patch tag for the workflow fix.
- Safe modification: Always verify the release workflow locally on a test tag before pushing the real tag. Keep the "Release-pipeline gotchas" section in `AGENTS.md` current.

**`headless-daemon` is production-ready but `native-host` is a placeholder:**
- Files: `crates/native-host/src/main.rs` (the entire binary logs one info message and exits), `crates/headless-daemon/src/main.rs` (fully implemented)
- Why fragile: `native-host` is compiled and shipped in the workspace. A caller that tries to use it as a real NMH bridge (e.g., by wiring it in a browser extension manifest) will get a process that immediately exits cleanly with no data exchanged.
- Safe modification: Do not reference `linkpilot-native-host` in browser extension manifests until v0.3 is implemented.

---

## Scaling Limits

**`RouteHistory` in-memory ring buffer (hard cap 1000):**
- Current capacity: 1000 entries (`DEFAULT_CAPACITY` in `crates/core/src/history.rs` line 12)
- Limit: History is lost on daemon restart. For power users routing hundreds of URLs per day, the ring rolls over in under a day.
- Scaling path: Persist history to a NDJSON file similar to `observations.ndjson`; load the last N records on startup.

**`observations.ndjson` unbounded append:**
- Current capacity: Default is `None` (retain forever). `retain_within` is called at startup only when `behavior_log_retention_days` is non-None in settings. Power users who never set a retention limit will accumulate observations indefinitely.
- Files: `crates/core/src/observations.rs` (lines 244–269), `apps/desktop/src-tauri/src/lib.rs` (lines 108–116)
- Limit: The startup sweep reads the full file into memory to filter; a multi-year log could cause a noticeable startup delay.
- Scaling path: Make the default retention non-`None` (e.g., 90 days) or add a startup log-size guard.

---

## Dependencies at Risk

**`platform-win` and `platform-linux` are permanent stubs:**
- Risk: Both crates are compiled in the workspace (gated by `#[cfg(target_os = ...)]`) but contain only a `pub use linkpilot_core::platform::StubProvider` re-export. There is no committed cross-platform milestone.
- Files: `crates/platform-win/src/lib.rs`, `crates/platform-linux/src/lib.rs`
- Impact: Any consumer that builds for Win/Linux gets a `StubProvider` that does nothing (no browser launch, no browser inventory, no LaunchAgent). This is expected behavior but could surprise contributors who test on Linux.
- Migration plan: Add real Win (`ShellExecute` / `start` command) and Linux (`xdg-open`) implementations when a cross-platform milestone is scoped.

**macOS-only CI for Rust tests:**
- Risk: The `rust` CI job runs on `macos-latest` only. Linux-only bugs (e.g., in `default_config_path`'s Linux branch, or the `platform-linux` stub) are never caught in CI.
- Files: `.github/workflows/ci.yml` (rust job `runs-on: macos-latest`)
- Impact: The Linux config path (`$XDG_CONFIG_HOME/...`) is code-path that is written but never run under CI.
- Migration plan: Add a `runs-on: ubuntu-latest` matrix leg that runs `cargo test --workspace --exclude linkpilot-desktop`.

---

## Missing Critical Features

**No `lpt history` subcommand:**
- Problem: `RouteHistory` exists in the daemon and is exposed via the IPC protocol (`Request::RouteHistory`), but the CLI has no handler for it. Users cannot inspect routing decisions from the terminal.
- Blocks: Debugging misrouted URLs without opening the GUI inspector.

**Browser extension is unreleased (v0.3 placeholder):**
- Problem: `apps/extension/` contains only a README; `crates/native-host/src/main.rs` is a no-op placeholder. The NMH bridge that would allow browser extensions to communicate back to the daemon has not been implemented.
- Blocks: In-browser routing corrections; the Chromium extension milestone.

---

## Test Coverage Gaps

**Zero frontend tests:**
- What's not tested: The entire React frontend (`apps/desktop/src/`) has no test suite — no vitest, no Playwright, no jest config. The `package.json` has no `test` script.
- Files: `apps/desktop/src/` (entire directory, ~12,700 lines of TS/TSX)
- Risk: Routing logic reproduced in the UI (e.g., picker display, rule ordering, workspace resolution) can silently diverge from the Rust truth.
- Priority: High for `HaloPreview.tsx`, `rules.tsx`, and `OnboardingFlow.tsx` (the highest-complexity, most user-visible components).

**`platform-mac` macOS-native code largely untested:**
- What's not tested: `opener.rs` (poll loop, stale-app detection), `app_icon.rs`, `default_browser.rs`, `launcher.rs`, `notifier.rs`, `app_picker.rs` — none have `#[test]` blocks. Only `launch_agent.rs` (6 tests) and `inventory.rs` (2 tests) have coverage.
- Files: `crates/platform-mac/src/opener.rs`, `crates/platform-mac/src/default_browser.rs`, `crates/platform-mac/src/launcher.rs`
- Risk: LaunchServices API wrappers and opener detection are core to the product's value proposition; regressions go undetected until user reports.
- Priority: High — add integration tests that mock `NSWorkspace` via a `MockPlatformProvider` at the `PlatformProvider` trait boundary.

**`rules.rs` has zero in-module tests:**
- What's not tested: The `Rule`, `MatcherTree`, and `Action` model types in `crates/core/src/rules.rs` have no inline `#[cfg(test)]` module. Rule behavior is tested indirectly through `routing.rs` tests (12 tests) but model-level validation (e.g., serialization round-trips, enabled/disabled edge cases) is uncovered.
- Files: `crates/core/src/rules.rs`
- Risk: A `serde` attribute change or a new field could silently break config round-trips.
- Priority: Medium.

**`config/mod.rs` has zero tests:**
- What's not tested: `ConfigDocument`, `Settings`, `Workspace`, and the `Meta` type in `crates/core/src/config/mod.rs` have no test module. Default values and migration logic not covered by `store.rs` tests are untested.
- Files: `crates/core/src/config/mod.rs`
- Risk: Default-value regressions or breaking serde changes silently ship.
- Priority: Medium.

---

## Uncommitted / Stale Files

**Experimental site variants checked out but untracked:**
- What they are: `site/index-bold.html`, `site/index-editorial.html`, `site/index-native.html`, `site/index-terminal.html`, `site/_halo_crown.svg.png` — four design-variant landing pages and a stray asset, all untracked by git (`??` in `git status`). `site/index.html` is tracked but has uncommitted modifications.
- Files: `site/index-bold.html`, `site/index-editorial.html`, `site/index-native.html`, `site/index-terminal.html`, `site/_halo_crown.svg.png`, `site/index.html`
- Risk: Design variants are living as local filesystem state only; any `git clean -fd` would lose them. Contributors cloning the repo do not see the variant candidates.
- Fix: Either commit the chosen variant (replacing `index.html`) and delete the others, or add `site/index-*.html` to `.gitignore` explicitly.

**Stale git worktree triggering editor CPU spike:**
- What it is: `.claude/worktrees/gifted-sutherland-eee2d1/` is a physical directory on disk but its backing git worktree is missing (`refs/remotes/origin/main` absent), causing the git extension in Trae to loop.
- Files: `.claude/worktrees/gifted-sutherland-eee2d1/` (physical directory); `.gitignore` excludes `worktrees` so this is not tracked.
- Risk: As documented in `debug-linkpilot-cpu-spike.md`, this causes repeated extension-host restarts and sustained CPU usage in Trae.
- Fix: `git worktree prune` to clean up the stale registration, then `rm -rf .claude/worktrees/gifted-sutherland-eee2d1`.

**Open debug investigation file at repo root:**
- What it is: `debug-linkpilot-cpu-spike.md` is an untracked investigation document at the repo root. Status: `OPEN`.
- Files: `debug-linkpilot-cpu-spike.md`
- Risk: Investigation artifacts accumulate at the root; unclear ownership of when they are resolved and removed.
- Fix: Move to `.dbg/` or delete once the underlying worktree issue is resolved.

---

*Concerns audit: 2026-06-05*
