//! A per-user, on-demand Mach service. Never a privileged/root daemon, and
//! never installed by the sandboxed app. Opening the external Helper opts in.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Agent {
    label: String,
    program_arguments: Vec<String>,
    mach_services: BTreeMap<String, bool>,
}

fn domain() -> String {
    format!("gui/{}", unsafe { libc::geteuid() })
}

fn path(label: &str) -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|p| {
            p.join("Library/LaunchAgents")
                .join(format!("{label}.plist"))
        })
        .ok_or_else(|| "Cannot locate the user's LaunchAgents directory".into())
}

pub fn install() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    if executable
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|p| p.to_str())
        != Some("Contents")
    {
        return Err("Install and open the signed LinkPilot Helper.app bundle".into());
    }
    let label = linkpilot_helper_ipc::transport::service_name()?;
    let path = path(&label)?;
    if path.exists() {
        // Only replace a job whose identity/command surface belongs to Helper.
        let existing: Agent = plist::from_file(&path).map_err(|e| e.to_string())?;
        if existing.label != label
            || existing.program_arguments.get(1).map(String::as_str) != Some("serve")
        {
            return Err(
                "Helper LaunchAgent has unexpected contents; refusing to replace it".into(),
            );
        }
    }
    stop_loaded(&label)?;
    let agent = Agent {
        label: label.clone(),
        program_arguments: vec![executable.to_string_lossy().into_owned(), "serve".into()],
        mach_services: BTreeMap::from([(label, true)]),
    };
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    // The file contains paths only; no tokens, URLs, profile data, or credentials.
    plist::to_file_xml(&path, &agent).map_err(|e| e.to_string())?;
    let result = Command::new("/bin/launchctl")
        .args(["bootstrap", &domain()])
        .arg(&path)
        .output()
        .map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(format!(
            "Cannot register Helper: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(())
}

fn stop_loaded(label: &str) -> Result<(), String> {
    let target = format!("{}/{label}", domain());
    let loaded = Command::new("/bin/launchctl")
        .args(["print", &target])
        .output()
        .map_err(|e| e.to_string())?;
    if loaded.status.success() {
        let result = Command::new("/bin/launchctl")
            .args(["bootout", &target])
            .output()
            .map_err(|e| e.to_string())?;
        if !result.status.success() {
            return Err(format!(
                "Cannot stop Helper: {}",
                String::from_utf8_lossy(&result.stderr)
            ));
        }
    }
    Ok(())
}

pub fn uninstall() -> Result<(), String> {
    let label = linkpilot_helper_ipc::transport::service_name()?;
    let path = path(&label)?;
    if path.exists() {
        let existing: Agent = plist::from_file(&path).map_err(|e| e.to_string())?;
        if existing.label != label {
            return Err(
                "Helper LaunchAgent belongs to a different service; refusing to remove it".into(),
            );
        }
    }
    stop_loaded(&label)?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
