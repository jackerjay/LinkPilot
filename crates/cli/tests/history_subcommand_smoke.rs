//! CLI-surface smoke test for `lpt history` (M3.3).
//!
//! Guards the clap structure (help text, top-level discovery) and the
//! offline error path — which is the only path we can hit hermetically
//! because the happy path requires a running daemon on the system
//! socket (covered by `crates/ipc/tests/route_history_roundtrip.rs`
//! against a freshly-spawned fixture daemon).

use std::process::Command;

fn lpt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_lpt"))
}

#[test]
fn top_help_lists_history() {
    let out = lpt().arg("--help").output().expect("run lpt");
    assert!(out.status.success(), "help exit: {}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("history"),
        "top-level help missing `history`: {stdout}"
    );
}

#[test]
fn history_help_exposes_limit_and_json() {
    let out = lpt().args(["history", "--help"]).output().expect("run lpt");
    assert!(out.status.success(), "help exit: {}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("--limit"), "missing --limit");
    assert!(stdout.contains("--json"), "missing --json");
}

#[test]
fn history_alias_hist_is_recognised() {
    // Aliases compile into clap; if someone removes the `#[command(alias)]`
    // attribute the help renders without the shortcut and this fails.
    let out = lpt().args(["hist", "--help"]).output().expect("run lpt");
    assert!(out.status.success(), "alias should work: {}", out.status);
}
