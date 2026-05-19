//! `linkpilot-daemon` — standalone macOS background process.
//!
//! Owns the routing engine, config store, route history, and IPC server.
//! v0.2 ships this alongside the Tauri shell: GUI-less installs run only
//! this binary (managed by a LaunchAgent), while GUI installs may run
//! both, in which case the GUI detects the live socket and becomes an
//! IPC client instead of spawning a second in-process daemon (see §13.3
//! in `docs/linkpilot-design-v0.2.md`).
//!
//! Hard contract: only one daemon per machine. Startup tries to acquire
//! the socket and fails fast with an actionable error if it's already
//! held — no silent fallback, no two-daemons-fighting-over-config.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use linkpilot_core::daemon::{
    cleanup_stale_pid_file, pid_file_path, remove_pid_file, write_pid_file, DaemonRuntime,
};
use linkpilot_core::endpoint::{default_endpoint, Endpoint};
use linkpilot_core::platform::PlatformProvider;
use linkpilot_core::protocol::Request;
use linkpilot_ipc::client;

#[derive(Parser, Debug)]
#[command(
    name = "linkpilot-daemon",
    version,
    about = "LinkPilot background daemon"
)]
struct Args {
    /// Override the config file path (defaults to the platform location).
    #[arg(long)]
    config: Option<PathBuf>,
    /// Run the IPC server in the foreground. Default behaviour — kept
    /// explicit so a LaunchAgent plist's ProgramArguments can pass it
    /// without ambiguity, and so a future `--check` style flag won't
    /// silently change semantics.
    #[arg(long)]
    serve: bool,
}

#[cfg(target_os = "macos")]
fn make_platform() -> Arc<dyn PlatformProvider> {
    Arc::new(linkpilot_platform_mac::MacProvider::default())
}

#[cfg(not(target_os = "macos"))]
fn make_platform() -> Arc<dyn PlatformProvider> {
    Arc::new(linkpilot_core::platform::StubProvider)
}

fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing();

    let endpoint = default_endpoint();

    // Hard fail-fast: if another daemon is alive on this socket, exit
    // immediately. The IPC server's own bind would fail later anyway,
    // but doing the check up front gives users a friendly error rather
    // than a cryptic `EADDRINUSE` deep in a tokio log line.
    if probe_existing_daemon(&endpoint) {
        eprintln!(
            "linkpilot-daemon: another daemon is already listening on {}",
            endpoint.display()
        );
        eprintln!("                 stop it first (`lpt daemon stop`) or kill the process.");
        std::process::exit(1);
    }

    let platform = make_platform();
    let (runtime, created) = DaemonRuntime::bootstrap(
        args.config.clone(),
        Arc::clone(&platform),
        env!("CARGO_PKG_VERSION"),
    )
    .context("bootstrap daemon")?;
    if created {
        tracing::info!(
            path = %runtime.config.path().display(),
            "wrote first-run config"
        );
    }

    // PID file: clean up any leftover from a previously-crashed daemon
    // before stamping our own PID. This is read by `lpt daemon status`
    // and `lpt daemon stop`; without the stale-cleanup, a hard kill of
    // the previous daemon would leave a PID file that the CLI keeps
    // reporting as "running" forever.
    let pid_path = pid_file_path().context("resolve pid file path")?;
    match cleanup_stale_pid_file(&pid_path) {
        Ok(true) => tracing::info!(path = %pid_path.display(), "cleaned up stale pid file"),
        Ok(false) => {}
        Err(err) => {
            tracing::warn!(?err, path = %pid_path.display(), "stale pid cleanup failed; continuing")
        }
    }
    if let Err(err) = write_pid_file(&pid_path) {
        tracing::warn!(?err, path = %pid_path.display(), "writing pid file failed; continuing");
    }

    // Stale-socket cleanup: probe just confirmed no daemon is answering
    // (returned false above), so any leftover socket file at the endpoint
    // path is stale — from a daemon that was SIGKILL'd or crashed before
    // its ServerHandle Drop could unlink. The IPC server's bind path
    // (linkpilot_ipc::server::run) also calls remove_file as a fallback,
    // but doing it here means the cleanup shows up in the daemon's
    // startup log (matching the PID-file pattern just above) so a future
    // debug session can tell at a glance whether a stale socket was
    // around. Errors are non-fatal — bind will retry the unlink anyway.
    if let linkpilot_core::endpoint::Endpoint::UnixSocket(ref path) = endpoint {
        match std::fs::remove_file(path) {
            Ok(()) => {
                tracing::info!(path = %path.display(), "cleaned up stale socket file")
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                tracing::warn!(?err, path = %path.display(), "stale socket cleanup failed; continuing")
            }
        }
    }

    tracing::info!(
        endpoint = %endpoint.display(),
        version = env!("CARGO_PKG_VERSION"),
        config = %runtime.config.path().display(),
        pid_file = %pid_path.display(),
        "linkpilot-daemon starting"
    );

    let handle = linkpilot_ipc::server::serve(endpoint.clone(), Arc::new(runtime))
        .map_err(|e| anyhow!("ipc server bind {}: {e}", endpoint.display()))?;

    // Block the main thread on a SIGTERM/SIGINT-aware tokio runtime so
    // shutdown drops the ServerHandle (which closes the listener and
    // unlinks the socket) instead of the OS yanking everything mid-flight.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("tokio runtime for signal wait")?;
    rt.block_on(async {
        wait_for_shutdown().await;
        tracing::info!("linkpilot-daemon: shutdown signal received");
    });

    // Explicit drop documents the order: the IPC handle's `Drop` impl
    // sends the shutdown signal to the spawned listener and waits for
    // the socket to unlink.
    drop(handle);

    // PID file cleanup happens AFTER the socket is gone so a racing
    // `lpt daemon status` doesn't see "PID file missing but socket
    // present" and infer a half-dead daemon.
    if let Err(err) = remove_pid_file(&pid_path) {
        tracing::warn!(?err, path = %pid_path.display(), "removing pid file on shutdown failed");
    }

    let _ = args.serve; // suppress dead_code until we add other modes
    Ok(())
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,linkpilot=debug"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

/// Returns true if a daemon is already serving on `endpoint`. Done with
/// a `StatePing` rather than a raw socket connect because the latter
/// would succeed for any other process that happens to hold the path —
/// we want to be sure it's a LinkPilot daemon talking back.
fn probe_existing_daemon(endpoint: &Endpoint) -> bool {
    let req = Request::StatePing {
        request_id: "daemon-startup-probe".into(),
    };
    client::send(endpoint, req).is_ok()
}

#[cfg(unix)]
async fn wait_for_shutdown() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(err) => {
            tracing::warn!(?err, "failed to install SIGTERM handler");
            return;
        }
    };
    let mut int_ = match signal(SignalKind::interrupt()) {
        Ok(s) => s,
        Err(err) => {
            tracing::warn!(?err, "failed to install SIGINT handler");
            return;
        }
    };
    tokio::select! {
        _ = term.recv() => tracing::info!("SIGTERM"),
        _ = int_.recv() => tracing::info!("SIGINT"),
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
