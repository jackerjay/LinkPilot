//! Wire framing. Length-prefixed JSON: `u32` big-endian length followed by the
//! UTF-8 JSON payload. Real tokio server/client wiring lands in step 4.

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("frame too large: {0} bytes")]
    FrameTooLarge(usize),
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
