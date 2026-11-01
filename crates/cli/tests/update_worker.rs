#![cfg(target_os = "macos")]

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Exercise the actual copied CLI worker, parent exit and recovery launch.
/// Authentication fails before any real LaunchAgent or app can be touched.
#[test]
fn detached_worker_refuses_invalid_candidate_and_restarts_fixture() {
    let temp = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(temp.path()).unwrap();
    let workspace = root.join(".linkpilot-update-integration");
    let updates = root.join("updates");
    let target = root.join("Original App.app");
    fs::create_dir(&workspace).unwrap();
    fs::set_permissions(&workspace, fs::Permissions::from_mode(0o700)).unwrap();
    fs::create_dir(&updates).unwrap();
    fs::create_dir(workspace.join("candidate.app")).unwrap();
    fs::create_dir_all(target.join("Contents/MacOS")).unwrap();
    let executable = target.join("Contents/MacOS/linkpilot-desktop");
    fs::write(
        &executable,
        format!(
            "#!/bin/sh\ntouch '{}'\n",
            workspace.join("restarted").display()
        ),
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let helper = workspace.join("lpt");
    fs::copy(env!("CARGO_BIN_EXE_lpt"), &helper).unwrap();
    let job = workspace.join("job.json");
    let token = uuid::Uuid::new_v4().to_string();

    // The fixture parent creates exactly the process relationship used by
    // the GUI, then exits after the worker signals readiness.
    let mut parent = Command::new("/bin/sh")
        .arg("-c")
        .arg(
            r#"
        read -r signal
        "$1" __update-worker --job "$2" --token "$3" > "$4/log" 2>&1 &
        worker_pid=$!
        echo "$worker_pid" > "$4/pid"
        attempt=0
        while test "$attempt" -lt 50; do
          attempt=$((attempt + 1))
          test -f "$4/ready" && exit 0
          /bin/sleep 0.1
        done
        kill "$worker_pid"
        exit 1
    "#,
        )
        .arg("fixture-parent")
        .arg(&helper)
        .arg(&job)
        .arg(&token)
        .arg(&workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let body = serde_json::json!({
        "target": target, "workspace": workspace, "updates": updates,
        "version": "0.5.12", "token": token, "old_pid": parent.id(),
    });
    fs::write(&job, serde_json::to_vec(&body).unwrap()).unwrap();
    fs::set_permissions(&job, fs::Permissions::from_mode(0o600)).unwrap();
    parent.stdin.take().unwrap().write_all(b"ready\n").unwrap();
    assert!(
        parent.wait().unwrap().success(),
        "{}",
        fs::read_to_string(workspace.join("log")).unwrap_or_default()
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !(updates.join("outcome.json").exists() && workspace.join("restarted").exists())
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    let outcome: serde_json::Value =
        serde_json::from_slice(&fs::read(updates.join("outcome.json")).unwrap()).unwrap();
    assert!(outcome["error"].as_str().unwrap().contains("codesign"));
    assert!(target.join("Contents/MacOS/linkpilot-desktop").exists());
    assert!(workspace.join("restarted").exists());
    assert!(workspace.join("candidate.app").exists());
}
