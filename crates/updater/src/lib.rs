//! Direct-distribution updates. Store builds never link this crate.
#![cfg(target_os = "macos")]

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const APP_ID: &str = "app.linkpilot.desktop";
const TEAM_REQUIREMENT: &str = "anchor apple generic and certificate leaf[subject.OU] = \"348D74H42G\" and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists";
const PREFIX: &str = ".linkpilot-update-";
pub const BACKGROUND_ARG: &str = "--linkpilot-update-background";
const JOB_ARG: &str = "--linkpilot-update-job";
const TOKEN_ARG: &str = "--linkpilot-update-token";

#[derive(Debug, Serialize, Deserialize)]
struct Job {
    target: PathBuf,
    workspace: PathBuf,
    updates: PathBuf,
    version: String,
    token: String,
    old_pid: u32,
}

impl Job {
    fn candidate(&self) -> PathBuf {
        self.workspace.join("candidate.app")
    }
    fn path(&self) -> PathBuf {
        self.workspace.join("job.json")
    }
    fn health(&self) -> PathBuf {
        self.workspace.join("health.json")
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Health {
    token: String,
    version: String,
    pid: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Outcome {
    pub version: String,
    pub error: Option<String>,
}

pub fn last_failure(updates: &Path, version: &str) -> Option<String> {
    let outcome: Outcome =
        serde_json::from_slice(&fs::read(updates.join("outcome.json")).ok()?).ok()?;
    (outcome.version == version)
        .then_some(outcome.error)
        .flatten()
}

pub fn app_root(executable: &Path) -> Result<PathBuf> {
    let root = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|p| p.extension().is_some_and(|ext| ext == "app"))
        .ok_or_else(|| anyhow!("Silent updates require an installed LinkPilot.app"))?;
    Ok(fs::canonicalize(root)?)
}

fn command_ok(command: &mut Command) -> Result<String> {
    let output = command
        .output()
        .with_context(|| format!("starting {:?}", command.get_program()))?;
    if !output.status.success() {
        bail!(
            "{}: {}",
            command.get_program().to_string_lossy(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn plist(app: &Path, key: &str) -> Result<String> {
    command_ok(
        Command::new("/usr/bin/plutil")
            .args(["-extract", key, "raw", "-o", "-"])
            .arg(app.join("Contents/Info.plist")),
    )
}

/// Checks identity and Developer ID policy, not a renderer-provided hash.
pub fn verify_app(app: &Path, version: &str) -> Result<()> {
    command_ok(
        Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(format!("-R={TEAM_REQUIREMENT} and identifier \"{APP_ID}\""))
            .arg(app),
    )?;
    assess_notarized(app, false)?;
    if plist(app, "CFBundleIdentifier")? != APP_ID
        || plist(app, "CFBundleShortVersionString")? != version
    {
        bail!("The signed application identity or version does not match the update");
    }
    let executable = app.join("Contents/MacOS/linkpilot-desktop");
    use std::io::Read;
    let mut header = [0; 1024];
    let length = File::open(executable)?.read(&mut header)?;
    let cpu = if std::env::consts::ARCH == "aarch64" {
        0x0100_000c
    } else {
        0x0100_0007
    };
    if !macho_has_cpu(&header[..length], cpu) {
        bail!("Update does not support this Mac's architecture");
    }
    Ok(())
}

// Read the signed Mach-O header directly: lipo is a developer-tools shim on
// some Macs and must not trigger a Command Line Tools installation prompt.
fn macho_has_cpu(header: &[u8], cpu: u32) -> bool {
    let Some(magic) = header.get(..4) else {
        return false;
    };
    let word = |offset, little| {
        header.get(offset..offset + 4).map(|bytes| {
            let bytes: [u8; 4] = bytes.try_into().unwrap();
            if little {
                u32::from_le_bytes(bytes)
            } else {
                u32::from_be_bytes(bytes)
            }
        })
    };
    match magic {
        [0xcf, 0xfa, 0xed, 0xfe] => word(4, true) == Some(cpu),
        [0xfe, 0xed, 0xfa, 0xcf] => word(4, false) == Some(cpu),
        [0xca, 0xfe, 0xba, 0xbe]
        | [0xca, 0xfe, 0xba, 0xbf]
        | [0xbe, 0xba, 0xfe, 0xca]
        | [0xbf, 0xba, 0xfe, 0xca] => {
            let little = magic[0] != 0xca;
            let stride = if magic[0] == 0xbf || magic[3] == 0xbf {
                32
            } else {
                20
            };
            word(4, little)
                .filter(|count| *count <= 32)
                .is_some_and(|count| {
                    (0..count as usize).any(|index| word(8 + index * stride, little) == Some(cpu))
                })
        }
        _ => false,
    }
}

fn verify_dmg(dmg: &Path) -> Result<()> {
    command_ok(
        Command::new("/usr/bin/codesign")
            .args(["--verify", "--strict"])
            .arg(format!("-R={TEAM_REQUIREMENT}"))
            .arg(dmg),
    )?;
    assess_notarized(dmg, true)?;
    Ok(())
}

fn assess_notarized(path: &Path, disk_image: bool) -> Result<()> {
    let mut command = Command::new("/usr/sbin/spctl");
    command
        .args(["--assess", "--verbose=2", "--type"])
        .arg(if disk_image { "open" } else { "execute" });
    if disk_image {
        command.args(["--context", "context:primary-signature"]);
    }
    let output = command.arg(path).output()?;
    let assessment = String::from_utf8_lossy(&output.stderr);
    if !output.status.success()
        || !assessment
            .lines()
            .any(|line| line.trim() == "source=Notarized Developer ID")
    {
        bail!(
            "Apple notarization assessment failed: {}",
            assessment.trim()
        );
    }
    Ok(())
}

fn newer(version: &str, current: &str) -> Result<()> {
    let next = semver::Version::parse(version)?;
    if !next.pre.is_empty() || next <= semver::Version::parse(current)? {
        bail!("Only a newer stable release can be installed");
    }
    Ok(())
}

struct Lock(File);
impl Lock {
    fn acquire(updates: &Path) -> Result<Self> {
        fs::create_dir_all(updates)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(updates.join("install.lock"))?;
        use std::os::fd::AsRawFd;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            bail!("Another update is already in progress");
        }
        if let Ok(bytes) = fs::read(updates.join("active.json")) {
            let pid: u32 = serde_json::from_slice(&bytes)?;
            if pid != std::process::id() && alive(pid) {
                bail!("Another update worker is already running");
            }
        }
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new_in(path.parent().context("missing parent")?)?;
    file.write_all(&serde_json::to_vec(value)?)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Stages on the installation volume. The running app is untouched on any
/// preparation error, including a read-only /Applications directory.
pub fn prepare_and_spawn(
    updates: &Path,
    version: &str,
    automatic: bool,
    allowed: impl Fn() -> bool,
) -> Result<bool> {
    let _lock = Lock::acquire(updates)?;
    if automatic {
        if let Some(error) = last_failure(updates, version) {
            bail!("Automatic retry paused for {version}: {error}");
        }
    }
    let result = prepare(updates, version, allowed);
    if let Err(error) = &result {
        write_json(
            &updates.join("outcome.json"),
            &Outcome {
                version: version.into(),
                error: Some(format!("{error:#}")),
            },
        )?;
    }
    result
}

fn prepare(updates: &Path, version: &str, allowed: impl Fn() -> bool) -> Result<bool> {
    // Parse before using the version in a filename.
    semver::Version::parse(version)?;
    let target = app_root(&std::env::current_exe()?)?;
    newer(version, &plist(&target, "CFBundleShortVersionString")?)?;
    if plist(&target, "CFBundleIdentifier")? != APP_ID {
        bail!("External updates are unavailable for this edition");
    }
    let dmg = updates.join(format!(
        "LinkPilot_{version}_{}.dmg",
        std::env::consts::ARCH
    ));
    verify_dmg(&dmg)?;
    let workspace = create_workspace(&target)?;
    let job = Job {
        target,
        workspace: workspace.path().to_owned(),
        updates: fs::canonicalize(updates)?,
        version: version.into(),
        token: uuid::Uuid::new_v4().to_string(),
        old_pid: std::process::id(),
    };
    let mount = job.workspace.join("mount");
    fs::create_dir(&mount)?;
    // Mount failures can leave a volume attached. Always attempt detach; never
    // recursively delete a workspace that might still contain a mounted disk.
    let staged = (|| -> Result<()> {
        command_ok(
            Command::new("/usr/bin/hdiutil")
                .args(["attach", "-readonly", "-nobrowse", "-mountpoint"])
                .arg(&mount)
                .arg(&dmg),
        )?;
        command_ok(
            Command::new("/usr/bin/ditto")
                .arg(mount.join("LinkPilot.app"))
                .arg(job.candidate()),
        )?;
        verify_app(&job.candidate(), version)
    })();
    let detached = command_ok(Command::new("/usr/bin/hdiutil").arg("detach").arg(&mount));
    if detached.is_err() {
        let retained = workspace.keep();
        bail!(
            "Could not detach update volume; staging retained at {}",
            retained.display()
        );
    }
    staged?;
    if !allowed() {
        return Ok(false);
    }
    let helper = job.workspace.join("lpt");
    command_ok(
        Command::new("/usr/bin/ditto")
            .arg(job.target.join("Contents/MacOS/lpt"))
            .arg(&helper),
    )?;
    write_json(&job.path(), &job)?;
    // Keep the current release's helper running outside the bundle that it swaps.
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(job.updates.join("worker.log"))?;
    let mut command = Command::new(helper);
    command
        .arg("__update-worker")
        .arg("--job")
        .arg(job.path())
        .arg("--token")
        .arg(&job.token)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("starting update worker")?;
    // The worker must validate the job before we let the GUI exit. It waits on
    // our process and then obtains the lock after this function returns.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !job.workspace.join("ready").exists() {
        if child.try_wait()?.is_some() || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Update worker did not start; the installed application is unchanged");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if !allowed() {
        let _ = child.kill();
        child.wait()?;
        return Ok(false);
    }
    if let Err(error) = write_json(&job.updates.join("active.json"), &child.id()) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let _ = workspace.keep();
    Ok(true)
}

fn create_workspace(target: &Path) -> Result<tempfile::TempDir> {
    let workspace = tempfile::Builder::new()
        .prefix(PREFIX)
        .tempdir_in(target.parent().context("missing app parent")?)
        .context("App folder is not writable; open the downloaded installer to upgrade manually")?;
    fs::set_permissions(workspace.path(), fs::Permissions::from_mode(0o700))?;
    Ok(workspace)
}

fn load_job(path: &Path, token: &str) -> Result<Job> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.uid() != unsafe { libc::getuid() } || meta.mode() & 0o077 != 0 {
        bail!("Update job must be a private file owned by the current user");
    }
    let job: Job = serde_json::from_slice(&fs::read(path)?)?;
    if uuid::Uuid::parse_str(token).is_err()
        || token != job.token
        || path != job.path()
        || !job
            .workspace
            .file_name()
            .is_some_and(|s| s.to_string_lossy().starts_with(PREFIX))
        || job.workspace.parent() != job.target.parent()
        || fs::canonicalize(&job.workspace)? != job.workspace
        || fs::metadata(&job.workspace)?.mode() & 0o077 != 0
        || !job.target.extension().is_some_and(|ext| ext == "app")
        || fs::canonicalize(&job.target)? != job.target
        || fs::canonicalize(&job.updates)? != job.updates
        || job.old_pid <= 1
    {
        bail!("Invalid update job paths or token");
    }
    Ok(job)
}

fn alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

fn wait_exit(pid: u32, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while alive(pid) {
        if Instant::now() >= deadline {
            bail!("Process {pid} did not exit; update cancelled");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn swap(left: &Path, right: &Path) -> Result<()> {
    let left = CString::new(left.as_os_str().as_bytes())?;
    let right = CString::new(right.as_os_str().as_bytes())?;
    if unsafe { libc::renamex_np(left.as_ptr(), right.as_ptr(), libc::RENAME_SWAP) } != 0 {
        return Err(std::io::Error::last_os_error()).context("atomic application swap failed");
    }
    Ok(())
}

fn start_app(job: &Job, health: bool) -> Result<Child> {
    let mut command = Command::new(job.target.join("Contents/MacOS/linkpilot-desktop"));
    command
        .arg(BACKGROUND_ARG)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if health {
        command
            .arg(JOB_ARG)
            .arg(job.path())
            .arg(TOKEN_ARG)
            .arg(&job.token);
    }
    command.spawn().context("starting LinkPilot")
}

fn await_health(job: &Job, child: &mut Child, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait()?.is_some() {
            bail!("Updated LinkPilot exited before becoming ready");
        }
        if let Ok(bytes) = fs::read(job.health()) {
            if let Ok(health) = serde_json::from_slice::<Health>(&bytes) {
                if health.token == job.token
                    && health.version == job.version
                    && health.pid == child.id()
                {
                    return Ok(());
                }
            }
        }
        if Instant::now() >= deadline {
            bail!("Updated LinkPilot did not confirm startup");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

// Stop/start only a loaded daemon whose executable belongs to this app.
// Do not rewrite the plist or change the disabled state of an unrelated service.
fn owned_daemon(target: &Path) -> Result<bool> {
    let status = linkpilot_platform_mac::launch_agent::daemon_status()?;
    Ok(daemon_belongs_to(target, &status))
}

fn daemon_belongs_to(
    target: &Path,
    status: &linkpilot_platform_mac::launch_agent::LaunchAgentStatus,
) -> bool {
    status.loaded
        && status.exec_path.as_deref() == Some(&target.join("Contents/MacOS/linkpilot-daemon"))
}

fn daemon_load(load: bool) -> Result<()> {
    let previous_pid = if load {
        None
    } else {
        linkpilot_platform_mac::launch_agent::daemon_status()?.pid
    };
    let plist = linkpilot_platform_mac::launch_agent::daemon_plist_path()?;
    command_ok(
        Command::new("/bin/launchctl")
            .arg(if load { "load" } else { "unload" })
            .arg(plist),
    )?;
    if load {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let status = linkpilot_platform_mac::launch_agent::daemon_status()?;
            if status.pid.is_some_and(|p| alive(p as u32)) {
                break;
            }
            if Instant::now() >= deadline {
                bail!("Updated daemon did not start");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    } else if let Some(pid) = previous_pid {
        wait_exit(pid as u32, Duration::from_secs(10))?;
    }
    Ok(())
}

/// Internal CLI entry. Only operates on a private, validated staged job.
pub fn run_worker(path: &Path, token: &str) -> Result<()> {
    let job = load_job(path, token)?;
    if app_root(&std::env::current_exe()?).is_ok()
        || std::env::current_exe()? != job.workspace.join("lpt")
        || unsafe { libc::getppid() as u32 } != job.old_pid
    {
        bail!("Update worker must run from its private workspace");
    }
    fs::write(job.workspace.join("ready"), b"ready")?;
    let mut transaction_started = false;
    let result = (|| -> Result<()> {
        wait_exit(job.old_pid, Duration::from_secs(30))?;
        let _lock = Lock::acquire(&job.updates)?;
        verify_app(&job.candidate(), &job.version)?;
        newer(
            &job.version,
            &plist(&job.target, "CFBundleShortVersionString")?,
        )?;
        let daemon = owned_daemon(&job.target)?;
        if daemon {
            if let Err(error) = daemon_load(false) {
                daemon_load(true).context("restoring daemon after stop failure")?;
                return Err(error);
            }
        }
        transaction_started = true;
        transact(
            &job,
            Duration::from_secs(30),
            || if daemon { daemon_load(true) } else { Ok(()) },
            || if daemon { daemon_load(false) } else { Ok(()) },
        )
    })();
    // Do not launch a duplicate if the GUI never exited. On preparation
    // failure after exit, bring back the unchanged application first.
    let result = if result.is_err() && !transaction_started && !alive(job.old_pid) {
        start_app(&job, false).map(|_| ()).and(result)
    } else {
        result
    };
    let error = result.as_ref().err().map(|e| format!("{e:#}"));
    write_json(
        &job.updates.join("outcome.json"),
        &Outcome {
            version: job.version.clone(),
            error,
        },
    )?;
    if result.is_ok() {
        fs::remove_dir_all(&job.workspace)?;
    } else {
        eprintln!("Update failed; recovery files: {}", job.workspace.display());
    }
    result
}

fn transact(
    job: &Job,
    timeout: Duration,
    start_daemon: impl Fn() -> Result<()>,
    stop_daemon: impl Fn() -> Result<()>,
) -> Result<()> {
    if let Err(error) = swap(&job.target, &job.candidate()) {
        let daemon = start_daemon();
        let gui = start_app(job, false);
        daemon.context("restoring daemon after failed swap")?;
        gui.context("restarting unchanged application")?;
        return Err(error);
    }
    let mut child = None;
    let installed = (|| -> Result<()> {
        start_daemon()?;
        child = Some(start_app(job, true)?);
        await_health(job, child.as_mut().unwrap(), timeout)
    })();
    if let Err(error) = installed {
        if let Some(mut child) = child {
            // Child is unreaped and owned by us; never signal a guessed PID.
            let _ = child.kill();
            child.wait()?;
        }
        let stopped = stop_daemon();
        swap(&job.target, &job.candidate()).with_context(|| {
            format!(
                "{error:#}; rollback failed; previous app retained at {}",
                job.candidate().display()
            )
        })?;
        let daemon = start_daemon();
        let gui = start_app(job, false);
        daemon.context("previous application restored; restarting its daemon failed")?;
        gui.context("restarting previous application")?;
        stopped.context(
            "previous version restored, but stopping the failed daemon reported an error",
        )?;
        bail!("{error:#}; previous version restored");
    }
    Ok(())
}

pub fn background_restart() -> bool {
    std::env::args().any(|arg| arg == BACKGROUND_ARG)
}

/// Called on the new app's event-loop Ready event, after setup and tray creation.
pub fn acknowledge_startup() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let Some(index) = args.iter().position(|s| s == JOB_ARG) else {
        return Ok(());
    };
    let path = args.get(index + 1).context("missing update job")?;
    let token = args
        .iter()
        .position(|s| s == TOKEN_ARG)
        .and_then(|i| args.get(i + 1))
        .context("missing update token")?;
    let job = load_job(Path::new(path), token)?;
    if app_root(&std::env::current_exe()?)? != job.target {
        bail!("Update acknowledgement from wrong installation");
    }
    let version = plist(&job.target, "CFBundleShortVersionString")?;
    if version != job.version {
        bail!("Update acknowledgement version mismatch");
    }
    write_json(
        &job.health(),
        &Health {
            token: token.clone(),
            version,
            pid: std::process::id(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::os::unix::fs::PermissionsExt;

    fn fixture() -> (tempfile::TempDir, Job) {
        let temp = tempfile::tempdir().unwrap();
        let parent = fs::canonicalize(temp.path()).unwrap();
        let workspace = tempfile::Builder::new()
            .prefix(PREFIX)
            .tempdir_in(&parent)
            .unwrap()
            .keep();
        fs::set_permissions(&workspace, fs::Permissions::from_mode(0o700)).unwrap();
        let updates = parent.join("updates");
        fs::create_dir(&updates).unwrap();
        let job = Job {
            target: parent.join("My LinkPilot.app"),
            workspace,
            updates,
            version: "0.5.12".into(),
            token: uuid::Uuid::new_v4().to_string(),
            old_pid: std::process::id(),
        };
        for app in [&job.target, &job.candidate()] {
            fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        }
        fs::write(job.target.join("old-version"), b"old").unwrap();
        fs::write(job.candidate().join("new-version"), b"new").unwrap();
        script(&job.target, "exit 0");
        write_json(&job.path(), &job).unwrap();
        (temp, job)
    }

    fn script(app: &Path, body: &str) {
        let path = app.join("Contents/MacOS/linkpilot-desktop");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn health_script(job: &Job, token: &str, version: &str) -> String {
        // All values are controlled test fixtures. Keep the shell alive long
        // enough to prove readiness belongs to the exact child process.
        format!("printf '{{\"token\":\"{token}\",\"version\":\"{version}\",\"pid\":%s}}' \"$$\" > '{}'\nexec /bin/sleep 5", job.health().display())
    }

    #[test]
    fn atomic_install_keeps_old_bundle_until_exact_child_is_ready() {
        let (_temp, job) = fixture();
        script(
            &job.candidate(),
            &health_script(&job, &job.token, &job.version),
        );
        let data = job.updates.join("user-config.json");
        fs::write(&data, b"keep profile and rules").unwrap();
        transact(&job, Duration::from_secs(2), || Ok(()), || Ok(())).unwrap();
        assert!(job.target.join("new-version").exists());
        assert!(job.candidate().join("old-version").exists());
        assert_eq!(fs::read(data).unwrap(), b"keep profile and rules");
        let health: Health = serde_json::from_slice(&fs::read(job.health()).unwrap()).unwrap();
        unsafe {
            libc::kill(health.pid as i32, libc::SIGTERM);
        }
        let mut status = 0;
        unsafe {
            libc::waitpid(health.pid as i32, &mut status, 0);
        }
    }

    #[test]
    fn exited_candidate_rolls_back_and_restarts_previous_app() {
        let (_temp, job) = fixture();
        script(&job.candidate(), "exit 42");
        let restarted = job.workspace.join("old-restarted");
        script(&job.target, &format!("touch '{}'", restarted.display()));
        let error = transact(&job, Duration::from_secs(1), || Ok(()), || Ok(())).unwrap_err();
        assert!(error.to_string().contains("previous version restored"));
        assert!(job.target.join("old-version").exists());
        let deadline = Instant::now() + Duration::from_secs(2);
        while !restarted.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(restarted.exists());
    }

    #[test]
    fn wrong_health_token_version_or_pid_never_commits() {
        for (token, version, wrong_pid) in [
            ("wrong", "0.5.12", false),
            ("valid", "0.5.11", false),
            ("valid", "0.5.12", true),
        ] {
            let (_temp, job) = fixture();
            let token = if token == "valid" { &job.token } else { token };
            let mut body = health_script(&job, token, version);
            if wrong_pid {
                body = body.replace("\"$$\"", "\"1\"");
            }
            script(&job.candidate(), &body);
            let error =
                transact(&job, Duration::from_millis(200), || Ok(()), || Ok(())).unwrap_err();
            assert!(error.to_string().contains("did not confirm startup"));
            assert!(job.target.join("old-version").exists());
        }
    }

    #[test]
    fn missing_executable_rolls_back() {
        let (_temp, job) = fixture();
        assert!(transact(&job, Duration::from_secs(1), || Ok(()), || Ok(())).is_err());
        assert!(job.target.join("old-version").exists());
    }

    #[test]
    fn failed_swap_leaves_current_bundle_and_restarts_it() {
        let (_temp, job) = fixture();
        fs::remove_dir_all(job.candidate()).unwrap();
        assert!(transact(&job, Duration::from_millis(100), || Ok(()), || Ok(())).is_err());
        assert!(job.target.join("old-version").exists());
    }

    #[test]
    fn read_only_install_folder_keeps_current_application() {
        let (_temp, job) = fixture();
        let parent = job.target.parent().unwrap();
        fs::set_permissions(parent, fs::Permissions::from_mode(0o500)).unwrap();
        let result = create_workspace(&job.target);
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(result.is_err());
        assert!(job.target.join("old-version").exists());
    }

    #[test]
    fn daemon_start_failure_restores_app_and_daemon() {
        let (_temp, job) = fixture();
        let starts = Cell::new(0);
        let stops = Cell::new(0);
        let result = transact(
            &job,
            Duration::from_millis(100),
            || {
                starts.set(starts.get() + 1);
                if starts.get() == 1 {
                    bail!("daemon failed");
                }
                Ok(())
            },
            || {
                stops.set(stops.get() + 1);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(starts.get(), 2);
        assert_eq!(stops.get(), 1);
        assert!(job.target.join("old-version").exists());
    }

    #[test]
    fn independent_and_unloaded_daemons_are_preserved() {
        use linkpilot_platform_mac::launch_agent::LaunchAgentStatus;
        let (_temp, job) = fixture();
        let mut status = LaunchAgentStatus {
            loaded: true,
            exec_path: Some(job.target.join("Contents/MacOS/linkpilot-daemon")),
            ..Default::default()
        };
        assert!(daemon_belongs_to(&job.target, &status));
        status.loaded = false;
        assert!(!daemon_belongs_to(&job.target, &status));
        status.loaded = true;
        status.exec_path = Some("/opt/homebrew/bin/linkpilot-daemon".into());
        assert!(!daemon_belongs_to(&job.target, &status));
    }

    #[test]
    fn rollback_failure_keeps_backup_and_names_recovery_location() {
        let (_temp, job) = fixture();
        script(&job.candidate(), "exit 1");
        let error = transact(
            &job,
            Duration::from_secs(1),
            || Ok(()),
            || {
                fs::rename(&job.target, job.workspace.join("failed.app"))?;
                Ok(())
            },
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("rollback failed"));
        assert!(format!("{error:#}").contains(&job.candidate().display().to_string()));
        assert!(job.candidate().join("old-version").exists());
    }

    #[test]
    fn concurrent_install_is_rejected() {
        let (_temp, job) = fixture();
        let lock = Lock::acquire(&job.updates).unwrap();
        assert!(Lock::acquire(&job.updates).is_err());
        drop(lock);
        assert!(Lock::acquire(&job.updates).is_ok());
        let child = Command::new("/bin/sleep").arg("5").spawn().unwrap();
        write_json(&job.updates.join("active.json"), &child.id()).unwrap();
        assert!(Lock::acquire(&job.updates).is_err());
        let mut child = child;
        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn failed_version_is_not_automatically_retried() {
        let (_temp, job) = fixture();
        write_json(
            &job.updates.join("outcome.json"),
            &Outcome {
                version: job.version.clone(),
                error: Some("rolled back".into()),
            },
        )
        .unwrap();
        let error = prepare_and_spawn(&job.updates, &job.version, true, || true).unwrap_err();
        assert!(error.to_string().contains("retry paused"));
        assert!(last_failure(&job.updates, "0.5.13").is_none());
    }

    #[test]
    fn job_rejects_wrong_token_public_permissions_and_symlinks() {
        let (_temp, job) = fixture();
        load_job(&job.path(), &job.token).unwrap();
        assert!(load_job(&job.path(), &uuid::Uuid::new_v4().to_string()).is_err());
        fs::set_permissions(job.path(), fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load_job(&job.path(), &job.token).is_err());
        fs::remove_file(job.path()).unwrap();
        let elsewhere = job.workspace.join("elsewhere");
        write_json(&elsewhere, &job).unwrap();
        std::os::unix::fs::symlink(elsewhere, job.path()).unwrap();
        assert!(load_job(&job.path(), &job.token).is_err());
    }

    #[test]
    fn downgrade_equal_and_prerelease_versions_are_rejected() {
        for version in ["0.5.10", "0.5.11", "0.5.12-beta.1", "../../evil"] {
            assert!(newer(version, "0.5.11").is_err());
        }
        assert!(newer("0.5.12", "0.5.11").is_ok());
    }

    #[test]
    fn signed_macho_requires_native_cpu_without_developer_tools() {
        let arm = 0x0100_000c;
        let intel = 0x0100_0007;
        let thin = [0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0, 0, 1];
        assert!(macho_has_cpu(&thin, arm));
        assert!(!macho_has_cpu(&thin, intel));
        let mut fat = vec![0; 48];
        fat[..8].copy_from_slice(&[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 2]);
        fat[8..12].copy_from_slice(&intel.to_be_bytes());
        fat[28..32].copy_from_slice(&arm.to_be_bytes());
        assert!(macho_has_cpu(&fat, arm));
        assert!(macho_has_cpu(&fat, intel));
        assert!(!macho_has_cpu(&fat[..8], arm));
        assert!(!macho_has_cpu(b"truncated", arm));
    }

    #[test]
    fn unsigned_payload_is_rejected() {
        let (_temp, job) = fixture();
        assert!(verify_app(&job.candidate(), &job.version).is_err());
        let dmg = job.workspace.join("unsigned.dmg");
        fs::write(&dmg, b"untrusted").unwrap();
        assert!(verify_dmg(&dmg).is_err());
    }

    /// Uses a real, published Developer ID / notarized release. Never launches
    /// the application or changes the installed app, user config or services.
    #[test]
    #[ignore = "set LINKPILOT_TEST_DMG to an official signed release DMG"]
    fn signed_release_authentication_and_tamper_rejection() {
        let dmg = PathBuf::from(std::env::var_os("LINKPILOT_TEST_DMG").unwrap());
        verify_dmg(&dmg).unwrap();
        let temp = tempfile::tempdir().unwrap();
        let mount = temp.path().join("mount");
        fs::create_dir(&mount).unwrap();
        command_ok(
            Command::new("/usr/bin/hdiutil")
                .args(["attach", "-readonly", "-nobrowse", "-mountpoint"])
                .arg(&mount)
                .arg(&dmg),
        )
        .unwrap();
        let app = temp.path().join("Copy.app");
        let copied = command_ok(
            Command::new("/usr/bin/ditto")
                .arg(mount.join("LinkPilot.app"))
                .arg(&app),
        );
        let detached = command_ok(Command::new("/usr/bin/hdiutil").arg("detach").arg(&mount));
        if detached.is_err() {
            let _ = temp.keep();
            panic!("could not detach test volume");
        }
        copied.unwrap();
        let version = plist(&app, "CFBundleShortVersionString").unwrap();
        verify_app(&app, &version).unwrap();
        assert!(verify_app(&app, "99.0.0").is_err());
        command_ok(
            Command::new("/usr/bin/plutil")
                .args([
                    "-replace",
                    "CFBundleShortVersionString",
                    "-string",
                    "99.0.0",
                ])
                .arg(app.join("Contents/Info.plist")),
        )
        .unwrap();
        assert!(verify_app(&app, "99.0.0").is_err());
    }
}
