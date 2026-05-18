//! CLI-surface smoke test for `lp daemon`.
//!
//! v0.2 (M2) introduced a 7-action subcommand group; this test guards
//! the clap structure (help renders, every action parses, status emits
//! the JSON schema the design contract requires). It does NOT exercise
//! daemon-spawn / socket-bind paths — those are macOS-runtime side
//! effects that CI runners can't reproduce hermetically, and they're
//! covered by the platform-mac / core unit tests plus manual
//! verification per docs/linkpilot-design-v0.2.md §14.1.4.
//!
//! Build trick: spawning `target/release/lp` would only work when the
//! release profile happens to be fresh, so we use cargo's built-in
//! `CARGO_BIN_EXE_<name>` which points at the binary the integration
//! test framework just rebuilt for us.

use std::process::Command;

fn lp() -> Command {
    Command::new(env!("CARGO_BIN_EXE_lp"))
}

#[test]
fn daemon_top_help_lists_every_action() {
    let out = lp().args(["daemon", "--help"]).output().expect("run lp");
    assert!(out.status.success(), "help exit: {}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    for action in [
        "start",
        "stop",
        "restart",
        "status",
        "install",
        "uninstall",
        "logs",
    ] {
        assert!(
            stdout.contains(action),
            "help missing `{action}`:\n{stdout}"
        );
    }
}

#[test]
fn daemon_status_help_documents_json_flag() {
    let out = lp()
        .args(["daemon", "status", "--help"])
        .output()
        .expect("run lp");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("--json"));
}

#[test]
fn daemon_logs_help_exposes_follow_and_lines() {
    let out = lp()
        .args(["daemon", "logs", "--help"])
        .output()
        .expect("run lp");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("--follow"), "missing --follow flag");
    assert!(stdout.contains("--lines"), "missing --lines flag");
}

#[cfg(target_os = "macos")]
#[test]
fn daemon_status_json_has_expected_keys() {
    // `status --json` reads platform state (LaunchAgent + socket) but
    // never spawns anything, so it's safe to run in CI / locally even
    // when nothing is installed. We only assert the schema; the
    // values depend on the host environment.
    let out = lp()
        .args(["daemon", "status", "--json"])
        .output()
        .expect("run lp");
    assert!(
        out.status.success(),
        "status --json exited {}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("not JSON: {e}\n{stdout}"));
    for key in [
        "running",
        "version",
        "pid",
        "socket",
        "pid_file",
        "launch_agent",
    ] {
        assert!(
            v.get(key).is_some(),
            "JSON missing `{key}`: {}",
            serde_json::to_string_pretty(&v).unwrap_or_default()
        );
    }
    let la = &v["launch_agent"];
    for key in ["plist_exists", "loaded", "pid", "exec_path", "label"] {
        assert!(
            la.get(key).is_some(),
            "launch_agent JSON missing `{key}`: {}",
            serde_json::to_string_pretty(la).unwrap_or_default()
        );
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn daemon_actions_friendly_error_off_macos() {
    let out = lp().args(["daemon", "status"]).output().expect("run lp");
    assert!(!out.status.success(), "expected non-zero exit off-macos");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("macOS-only"),
        "expected macOS-only message: {stderr}"
    );
}
