use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use linkpilot_core::browser::{BrowserId, BrowserTarget};
use linkpilot_core::config::ConfigDocument;

#[test]
fn quick_open_setting_persists_and_invalid_input_preserves_config() {
    let directory = std::env::temp_dir().join(format!(
        "linkpilot-ask-quick-open-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("config.json");
    let doc = ConfigDocument::with_default(BrowserTarget::new(BrowserId::new("safari")));
    fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();

    for modifier in ["command", "option", "control", "shift", "off"] {
        let output = Command::new(env!("CARGO_BIN_EXE_lpt"))
            .arg("--config")
            .arg(&path)
            .args(["settings", "ask-quick-open", modifier])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["settings"]["ask_quick_open_modifier"], modifier);
        assert_eq!(saved["default_target"]["browser"], "safari");
    }

    let before = fs::read(&path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lpt"))
        .arg("--config")
        .arg(&path)
        .args(["settings", "ask-quick-open", "caps-lock"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown Ask quick-open modifier"));
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::remove_dir_all(directory).unwrap();
}
