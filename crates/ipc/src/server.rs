//! Tokio-based IPC server. The Tauri daemon spawns this on startup; CLI
//! clients (and, eventually, the Native Messaging Host) connect to it.

use std::sync::Arc;

use linkpilot_core::protocol::{Request, Response, ERROR_UNKNOWN_VERB};
use thiserror::Error;
use tokio::sync::oneshot;

use crate::path::Endpoint;
use crate::transport::{peek_request_id, read_raw_frame, write_frame, TransportError};

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("transport: {0}")]
    Transport(#[from] crate::transport::TransportError),
    #[error("endpoint not supported on this platform")]
    UnsupportedEndpoint,
}

// Trait moved to linkpilot_core::daemon in v0.2 so DaemonRuntime can impl
// it without a circular crate dependency. Re-exported here so existing
// `linkpilot_ipc::server::RequestHandler` imports still resolve.
pub use linkpilot_core::daemon::RequestHandler;

/// Spawn an IPC listener bound to `endpoint`. Returns a shutdown handle that
/// drops the listener when dropped.
pub fn serve<H: RequestHandler>(
    endpoint: Endpoint,
    handler: Arc<H>,
) -> Result<ServerHandle, ServerError> {
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("linkpilot-ipc")
        .enable_all()
        .build()
        .map_err(ServerError::Io)?;

    let endpoint_for_log = endpoint.clone();
    let jh = rt.spawn(async move {
        if let Err(err) = run(endpoint, handler, &mut shutdown_rx).await {
            tracing::error!(?err, "ipc server exited with error");
        }
    });

    Ok(ServerHandle {
        _rt: rt,
        _join: jh,
        _shutdown: Some(shutdown_tx),
        endpoint: endpoint_for_log,
    })
}

pub struct ServerHandle {
    _rt: tokio::runtime::Runtime,
    _join: tokio::task::JoinHandle<()>,
    _shutdown: Option<oneshot::Sender<()>>,
    pub endpoint: Endpoint,
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        // Notify the listener task to stop. The select! branch on the
        // shutdown receiver runs `remove_file` and returns Ok — clean
        // path.
        if let Some(tx) = self._shutdown.take() {
            let _ = tx.send(());
        }
        // Belt-and-suspenders socket cleanup. Field-order Drop runs
        // _rt next, which tears down the runtime and may cancel the
        // listener task before its shutdown branch unlinks the file.
        // We re-do the unlink here so stale sockets don't accumulate
        // and trip the next daemon start's fail-fast guard.
        if let Endpoint::UnixSocket(path) = &self.endpoint {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(unix)]
async fn run<H: RequestHandler>(
    endpoint: Endpoint,
    handler: Arc<H>,
    shutdown: &mut oneshot::Receiver<()>,
) -> Result<(), ServerError> {
    use tokio::net::UnixListener;

    let path = match endpoint {
        Endpoint::UnixSocket(p) => p,
        _ => return Err(ServerError::UnsupportedEndpoint),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(&path);

    let listener = UnixListener::bind(&path)?;
    tracing::info!(socket = %path.display(), "linkpilot ipc listening");

    loop {
        tokio::select! {
            _ = &mut *shutdown => {
                tracing::info!("ipc server: shutting down");
                let _ = std::fs::remove_file(&path);
                return Ok(());
            }
            accept = listener.accept() => {
                match accept {
                    Ok((stream, _addr)) => {
                        let h = Arc::clone(&handler);
                        tokio::spawn(async move {
                            if let Err(err) = serve_connection_unix(stream, h).await {
                                tracing::debug!(?err, "ipc connection closed");
                            }
                        });
                    }
                    Err(err) => {
                        tracing::warn!(?err, "ipc accept failed");
                    }
                }
            }
        }
    }
}

#[cfg(unix)]
async fn serve_connection_unix<H: RequestHandler>(
    stream: tokio::net::UnixStream,
    handler: Arc<H>,
) -> Result<(), ServerError> {
    let (read_half, write_half) = stream.into_split();
    let mut reader = read_half;
    let mut writer = write_half;
    loop {
        // Read raw bytes first; we still need the typed Request for the
        // happy path, but pulling them apart lets us recover from an
        // unrecognised verb (forward compat with v0.3+ clients) instead
        // of dropping the connection like protocol v1 did.
        let raw = match read_raw_frame(&mut reader).await {
            Ok(buf) => buf,
            Err(TransportError::Closed) => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        let resp = match serde_json::from_slice::<Request>(&raw) {
            Ok(req) => handler.handle(req),
            Err(err) => {
                // Best-effort: echo the original request_id so the
                // client can match this Error to its in-flight call.
                // If the frame is so malformed that no request_id is
                // recoverable, send empty — clients should still be
                // able to surface the message text.
                let request_id = peek_request_id(&raw).unwrap_or_default();
                tracing::debug!(?err, request_id = %request_id, "ipc: unrecognised request");
                Response::Error {
                    request_id,
                    code: ERROR_UNKNOWN_VERB.into(),
                    message: format!("request type not recognised by this daemon: {err}"),
                }
            }
        };
        write_frame(&mut writer, &resp).await?;
    }
}

#[cfg(not(unix))]
async fn run<H: RequestHandler>(
    _endpoint: Endpoint,
    _handler: Arc<H>,
    _shutdown: &mut oneshot::Receiver<()>,
) -> Result<(), ServerError> {
    Err(ServerError::UnsupportedEndpoint)
}
