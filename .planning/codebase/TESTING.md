# Testing Patterns

**Analysis Date:** 2026-06-05

---

## Overview

Testing is **thin on the frontend** and **meaningful on the Rust side**. The Rust test suite has genuine behavior coverage; the frontend has zero test files (no Vitest, no Jest, no Playwright). The CI pipeline validates frontend correctness only through `tsc --noEmit` + `vite build`.

---

## Rust Test Framework

**Runner:** Cargo's built-in test harness (no third-party test runner)

**Assertion style:** Standard `assert!`, `assert_eq!`, `assert!(...contains(...))`, `panic!(...)` — no assertion library

**Run Commands:**
```bash
cargo test -p linkpilot-core              # Core routing engine tests (recommended dev loop)
cargo test --workspace --exclude linkpilot-desktop  # All non-GUI crates (CI command)
cargo build -p linkpilot-cli && cargo test -p linkpilot-cli  # CLI smoke tests
```

---

## Rust Test File Organization

**Two patterns are used:**

### 1. Inline unit tests (inside `#[cfg(test)] mod tests`)

Tests live at the bottom of the same file as the implementation. Used for unit-level behavior of a single module.

Location pattern: bottom of `crates/core/src/*.rs`, e.g.:
- `crates/core/src/routing.rs` — inline `mod tests { ... }` with 11 tests
- `crates/core/src/history.rs` — inline `mod tests { ... }`
- `crates/core/src/protocol.rs` — inline `mod tests { ... }`
- `crates/core/src/config/store.rs` — inline `mod tests { ... }`

```rust
// Pattern from crates/core/src/routing.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::BrowserId;
    use crate::rules::{Action, MatcherTree, Rule, RuleSource};

    fn rule(host: &str, target_browser: &str) -> Rule { ... }  // builder helper
    fn ctx(url: &str) -> RoutingContext { ... }                 // context builder helper

    #[test]
    fn falls_back_to_default_target() { ... }

    #[test]
    fn matches_host_rule() { ... }
}
```

### 2. Integration tests (separate `tests/` directory)

Tests live in `crates/<name>/tests/*.rs`. Used for cross-crate / end-to-end behavior.

Location pattern:
- `crates/core/tests/dsl_roundtrip.rs` — cross-language contract test (Rust ↔ TypeScript DSL)
- `crates/ipc/tests/route_history_roundtrip.rs` — IPC server behavior end-to-end
- `crates/ipc/tests/stale_socket_cleanup.rs` — IPC server resilience
- `crates/ipc/tests/unknown_verb_fallback.rs` — IPC protocol forward-compat
- `crates/cli/tests/daemon_subcommand_smoke.rs` — CLI surface smoke test
- `crates/cli/tests/history_subcommand_smoke.rs` — CLI history subcommand smoke test

---

## Test Structure Patterns

### Unit test builder helpers

Tests define small inline builder functions rather than shared fixtures. This is the project-wide pattern:

```rust
// crates/core/src/routing.rs — inline helper functions
fn rule(host: &str, target_browser: &str) -> Rule {
    Rule {
        id: RuleId::default(),
        enabled: true,
        when: MatcherTree::UrlHost { pattern: host.to_string() },
        then: Action::Open { target: BrowserTarget::new(BrowserId::new(target_browser)) },
        source: RuleSource::Gui,
        note: None,
        workspace_id: None,
    }
}

fn ctx(url: &str) -> RoutingContext {
    RoutingContext {
        url: url.to_string(),
        source: Source { kind: SourceKind::System, app_name: None, bundle_id: None, browser: None, profile: None },
        navigation: None,
        environment: None,
    }
}
```

### Integration test setup with temp files

IPC integration tests use `uuid`-suffixed temp paths to avoid collisions:

```rust
// crates/ipc/tests/route_history_roundtrip.rs
fn tmp_socket() -> PathBuf {
    let id = uuid::Uuid::new_v4().simple().to_string();
    std::env::temp_dir().join(format!("lp-h-{}.sock", &id[..8]))
}

fn tmp_config() -> PathBuf {
    let id = uuid::Uuid::new_v4().simple().to_string();
    std::env::temp_dir().join(format!("lp-cfg-{}.json", &id[..8]))
}
```

Tests spin up a `DaemonRuntime` + `linkpilot_ipc::server::serve()`, wait for the socket to appear, then use **plain blocking `std::os::unix::net::UnixStream`** (never async client in tests — dropping a `ServerHandle` from inside another tokio runtime panics).

### CLI integration tests

CLI tests use `CARGO_BIN_EXE_<name>` to locate the test-built binary:

```rust
// crates/cli/tests/daemon_subcommand_smoke.rs
fn lpt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_lpt"))
}

#[test]
fn daemon_top_help_lists_every_action() {
    let out = lpt().args(["daemon", "--help"]).output().expect("run lpt");
    assert!(out.status.success(), "help exit: {}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    for action in ["start", "stop", "restart", "status", "install", "uninstall", "logs"] {
        assert!(stdout.contains(action), "help missing `{action}`:\n{stdout}");
    }
}
```

### Platform gating in tests

Tests that require macOS are gated with `#[cfg(target_os = "macos")]`; the complementary negative path is tested with `#[cfg(not(target_os = "macos"))]`:

```rust
// crates/cli/tests/daemon_subcommand_smoke.rs
#[cfg(target_os = "macos")]
#[test]
fn daemon_status_json_has_expected_keys() { ... }

#[cfg(not(target_os = "macos"))]
#[test]
fn daemon_actions_friendly_error_off_macos() { ... }
```

Unix-only IPC tests use `#![cfg(unix)]` at the crate top:

```rust
// crates/ipc/tests/route_history_roundtrip.rs
#![cfg(unix)]
```

### Graceful skip pattern

Tests that require optional external tooling emit a warning and pass (rather than failing) when the tool is absent:

```rust
// crates/core/tests/dsl_roundtrip.rs — skips when `bun` not on PATH
let Some(bun) = bun_on_path() else {
    eprintln!("skipping dsl_roundtrip: `bun` not on PATH. ...");
    return;
};
```

---

## Test Naming Conventions

Test function names describe the **behavior under test**, not the implementation:

```
falls_back_to_default_target
asks_when_default_target_is_not_configured
matches_host_rule
glob_matches_subdomain
explanation_annotates_each_node
disabled_workspace_skips_its_rules
source_app_matches_by_name_when_context_has_no_bundle_id
source_app_prefers_bundle_id_when_both_present
source_app_bundle_id_mismatch_does_not_fall_through_to_name
first_match_in_list_order_wins
route_history_returns_recent_records_newest_first
serve_succeeds_when_stale_socket_file_already_exists
unknown_verb_returns_error_and_keeps_connection_open
```

Regression tests include the user-facing bug description in a `///` doc comment:

```rust
/// Regression for the v0.2 list-order priority semantics: when two
/// rules both match the same URL+source, the rule that appears
/// EARLIER in `config.rules` wins, period.
#[test]
fn first_match_in_list_order_wins() { ... }
```

---

## Mocking

**No mocking framework.** Fake behavior is provided via:

1. **`StubProvider`** — a no-op implementation of `PlatformProvider` (defined in `crates/core/src/platform.rs`) used as the test double for platform dependencies in integration tests
2. **Inline `impl Trait`** fixtures — e.g. `PingHandler` in IPC tests implements `RequestHandler` with only the variants needed for the test
3. **In-memory `ConfigStore`** with temp files — real store code with a temp path, no mock

```rust
// crates/ipc/tests/stale_socket_cleanup.rs — inline fake handler
struct PingHandler;
impl RequestHandler for PingHandler {
    fn handle(&self, request: Request) -> Response {
        match request {
            Request::StatePing { request_id } => Response::Pong { request_id, daemon_version: "test".into() },
            _ => Response::Error { ... },
        }
    }
}
```

---

## TypeScript / DSL Tests

**Framework:** Bun's built-in test runner (`bun test`)

**Location:** `packages/config-dsl/src/compile.test.ts`

**Run Command:**
```bash
cd packages/config-dsl && bun test
```

**What is tested:** The `compile()` function's wire-format output — verifying that the TypeScript DSL emits snake_case JSON matching the Rust serde expectations:

- Top-level key names are snake_case
- Variant discriminants are kebab-case (`op: "url-host"`, `kind: "open"`)
- Rule `source` is stamped `"ts-compiled"`
- UUIDs are random v4 and non-colliding
- All `MatcherTree` and `Action` variants round-trip
- Settings camelCase → snake_case mapping
- List-order priority (no numeric `priority` field on the wire)

**Framework:** `bun:test` (`describe`, `test`, `expect`)

No frontend (React) tests exist. No Vitest, Jest, or Playwright setup.

---

## CI Test Pipeline

From `.github/workflows/ci.yml` — runs on every PR and `main` push:

### `rust` job (macOS-latest)
```bash
cargo fmt --all -- --check              # Format check (fails on diff)
cargo clippy --workspace --exclude linkpilot-desktop --all-targets -- -D warnings
cargo test --workspace --exclude linkpilot-desktop
cargo check -p linkpilot-desktop       # Type-checks Tauri shell (can't test — macOS runtime)
```

### `frontend` job (ubuntu-latest)
```bash
cd apps/desktop
corepack yarn install --no-lockfile --non-interactive
corepack yarn run build                # tsc --noEmit && vite build
```

There is no `yarn test` command. The frontend is validated by type-check + build, not by test execution.

### `desktop-bundle` job (macOS-latest)
```bash
cd apps/desktop
corepack yarn run tauri build --debug --bundles app
```

Smoke-tests the full Tauri bundling pipeline (icons, Info.plist, capabilities, frontend assets) in debug mode.

---

## Coverage

**No coverage threshold is enforced** in CI or configuration. There is no `cargo-tarpaulin` or `cargo-llvm-cov` setup.

**Honest assessment of coverage depth:**

| Area | Coverage | Notes |
|---|---|---|
| `crates/core/src/routing.rs` | High | 11 inline tests covering all routing paths, edge cases, and regressions |
| `crates/core/src/config/store.rs` | Medium | Inline tests for init, reload, atomic write, migration |
| `crates/core/src/protocol.rs` | Medium | Inline serde round-trip tests |
| `crates/ipc/` | High for server behavior | 3 integration tests covering history, stale socket, unknown-verb |
| `crates/cli/` | Medium | Smoke tests for help text and JSON schema; happy-path skipped (macOS runtime) |
| `crates/core/tests/dsl_roundtrip.rs` | Cross-language gate | Skipped unless `bun` on PATH |
| `packages/config-dsl/` | Good | 12 focused unit tests covering all variant shapes |
| `apps/desktop/src/` (React) | **None** | Zero test files; only tsc + build validation |
| `crates/platform-mac/` | **None** | No tests; macOS runtime required |

---

## Where to Add New Tests

**New routing behavior** (matcher variant, action, workspace logic):
- Add `#[test]` inside `crates/core/src/routing.rs`'s `mod tests`
- Use the existing `rule()` / `ctx()` builder helpers or define new inline ones
- Follow the naming pattern: `<behavior>_<condition>`

**New IPC verb:**
- Add an integration test in `crates/ipc/tests/<verb>_<behavior>.rs`
- Spin up a `DaemonRuntime` or a minimal `RequestHandler` impl
- Use blocking `UnixStream`; never tokio async client in tests

**New CLI subcommand:**
- Add a smoke test in `crates/cli/tests/<subcommand>_smoke.rs`
- Test `--help` output, flag presence, and the error path when the daemon is not running
- Use `env!("CARGO_BIN_EXE_lpt")` for the binary path

**New config field (DSL):**
- Add a test case in `packages/config-dsl/src/compile.test.ts`
- Verify snake_case key name and default value
- The cross-language roundtrip is validated by `crates/core/tests/dsl_roundtrip.rs` (requires `bun` on PATH)

---

*Testing analysis: 2026-06-05*
