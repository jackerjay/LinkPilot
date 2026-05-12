//! Wire framing. `u32` BE length prefix + UTF-8 JSON payload.

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("frame too large: {0} bytes")]
    FrameTooLarge(usize),
    #[error("connection closed")]
    Closed,
}

pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, TransportError> {
    let json = serde_json::to_vec(msg)?;
    if json.len() > MAX_FRAME_BYTES {
        return Err(TransportError::FrameTooLarge(json.len()));
    }
    let mut out = Vec::with_capacity(4 + json.len());
    out.extend_from_slice(&(json.len() as u32).to_be_bytes());
    out.extend_from_slice(&json);
    Ok(out)
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, TransportError> {
    Ok(serde_json::from_slice(bytes)?)
}

/// Read one length-prefixed JSON message from an async stream.
pub async fn read_frame<R, T>(reader: &mut R) -> Result<T, TransportError>
where
    R: AsyncReadExt + Unpin,
    T: DeserializeOwned,
{
    let mut len_buf = [0u8; 4];
    if let Err(err) = reader.read_exact(&mut len_buf).await {
        if err.kind() == std::io::ErrorKind::UnexpectedEof {
            return Err(TransportError::Closed);
        }
        return Err(TransportError::Io(err));
    }
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(TransportError::FrameTooLarge(len));
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    decode(&buf)
}

/// Write one length-prefixed JSON message to an async stream.
pub async fn write_frame<W, T>(writer: &mut W, msg: &T) -> Result<(), TransportError>
where
    W: AsyncWriteExt + Unpin,
    T: Serialize,
{
    let bytes = encode(msg)?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}
