//! End-to-end check: the JSON `@linkpilot/config`'s `printConfig()`
//! emits must deserialize cleanly into the daemon's `ConfigDocument`,
//! and every key must round-trip without renaming.
//!
//! Strategy: invoke `bun run packages/config-dsl/examples/v0.1-demo.ts`
//! from inside this test, capture stdout, serde_json::from_str it into
//! `ConfigDocument`, and walk a handful of fields to confirm the shape.
//!
//! Skip behaviour: if `bun` isn't on PATH (CI without dev deps), emit a
//! warning and pass — the test gates the DSL ↔ daemon contract, not
//! Bun's presence. The TypeScript unit tests in
//! packages/config-dsl/src/compile.test.ts already validate the wire
//! shape independently of Rust; this test is the cross-language tie.

use std::path::PathBuf;
use std::process::Command;

use linkpilot_core::config::ConfigDocument;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is .../crates/core; the workspace root is two up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("walk up to workspace root")
        .to_path_buf()
}

fn bun_on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join("bun");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

#[test]
fn dsl_demo_round_trips_through_daemon_config_document() {
    let Some(bun) = bun_on_path() else {
        eprintln!(
            "skipping dsl_roundtrip: `bun` not on PATH. \
             Install from https://bun.sh to exercise this gate."
        );
        return;
    };

    let demo = workspace_root().join("packages/config-dsl/examples/v0.1-demo.ts");
    assert!(
        demo.exists(),
        "demo file moved? expected {}",
        demo.display()
    );

    let out = Command::new(&bun)
        .arg("run")
        .arg(&demo)
        .current_dir(workspace_root())
        .output()
        .expect("spawn bun");
    assert!(
        out.status.success(),
        "bun exited {}: stderr=\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8(out.stdout).expect("bun stdout is utf-8");
    let doc: ConfigDocument = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("DSL output failed to parse as ConfigDocument: {e}\n{stdout}"));

    // Walk the values the v0.1-demo.ts authored.
    assert_eq!(doc.version, 1, "schema version");
    assert_eq!(doc.default_target.browser.0, "arc");
    assert_eq!(doc.rules.len(), 6, "demo has 6 rules");
    // Every rule must come back tagged ts-compiled — that's the GUI's
    // signal to render the rules read-only in M4.4.
    for r in &doc.rules {
        assert_eq!(
            r.source,
            linkpilot_core::rules::RuleSource::TsCompiled,
            "rule {} not tagged ts-compiled",
            r.id.0
        );
    }

    // Workspaces + settings round-trip.
    assert_eq!(doc.workspaces.len(), 1);
    assert_eq!(doc.workspaces[0].id, "work");
    assert!(doc.settings.smart_routing_enabled);
}
