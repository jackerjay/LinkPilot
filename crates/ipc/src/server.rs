//! Tokio-based IPC server. The Tauri daemon spawns this on startup; CLI
//! clients (and, eventually, the Native Messaging Host) connect to it.

use std::sync::Arc;

use linkpilot_core::protocol::{Request, Response};
use thiserror::Error;
use tokio::sync::oneshot;

use crate::path::Endpoint;
use crate::transport::{read_frame, write_frame};

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("transport: {0}")]
    Transport(#[from] crate::transport::TransportError),
    #[error("endpoint not supported on this platform")]
    UnsupportedEndpoint,
}

/// The daemon implements this once and hands it to `serve`.
pub trait RequestHandler: Send + Sync + 'static {
    fn handle(&self, request: Request) -> Response;
}

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
        let req: Request = match read_frame(&mut reader).await {
            Ok(r) => r,
            Err(crate::transport::TransportError::Closed) => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        let resp = handler.handle(req);
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
