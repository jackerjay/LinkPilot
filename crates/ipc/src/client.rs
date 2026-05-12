//! Blocking IPC client used by `lp` and the Native Messaging Host bridge.
//!
//! Builds its own short-lived tokio runtime per call. This keeps callers free
//! of any async-runtime requirement.

use std::time::Duration;

use linkpilot_core::protocol::{Request, Response};
use thiserror::Error;

use crate::path::Endpoint;
use crate::transport::{read_frame, write_frame};

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("transport: {0}")]
    Transport(#[from] crate::transport::TransportError),
    #[error("endpoint not supported on this platform")]
    UnsupportedEndpoint,
    #[error("daemon offline ({0})")]
    Offline(String),
    #[error("timed out after {0:?}")]
    Timeout(Duration),
}

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Send `request` to the daemon at `endpoint` and wait for one response.
pub fn send(endpoint: &Endpoint, request: Request) -> Result<Response, ClientError> {
    let endpoint = endpoint.clone();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        tokio::time::timeout(DEFAULT_TIMEOUT, send_async(&endpoint, request))
            .await
            .map_err(|_| ClientError::Timeout(DEFAULT_TIMEOUT))?
    })
}

#[cfg(unix)]
async fn send_async(endpoint: &Endpoint, request: Request) -> Result<Response, ClientError> {
    use tokio::net::UnixStream;

    let path = match endpoint {
        Endpoint::UnixSocket(p) => p,
        _ => return Err(ClientError::UnsupportedEndpoint),
    };
    if !path.exists() {
        return Err(ClientError::Offline(format!(
            "no socket at {}",
            path.display()
        )));
    }
    let stream = UnixStream::connect(path)
        .await
        .map_err(|e| ClientError::Offline(e.to_string()))?;
    let (read_half, write_half) = stream.into_split();
    let mut reader = read_half;
    let mut writer = write_half;
    write_frame(&mut writer, &request).await?;
    let response: Response = read_frame(&mut reader).await?;
    Ok(response)
}

#[cfg(not(unix))]
async fn send_async(
    _endpoint: &Endpoint,
    _request: Request,
) -> Result<Response, ClientError> {
    Err(ClientError::UnsupportedEndpoint)
}
