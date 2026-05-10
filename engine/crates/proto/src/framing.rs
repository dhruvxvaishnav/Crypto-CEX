use std::io;

use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Maximum accepted frame payload size in bytes.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Errors returned by the frame codec.
#[derive(Debug, Error)]
pub enum FrameError {
    /// Frame length exceeds [`MAX_FRAME_BYTES`].
    #[error("frame length {length} exceeds max {max}")]
    FrameTooLarge {
        /// Observed frame length.
        length: usize,
        /// Maximum allowed length.
        max: usize,
    },
    /// Underlying IO error.
    #[error("frame io error")]
    Io(#[from] io::Error),
    /// JSON serialisation or deserialisation error.
    #[error("frame json error")]
    Json(#[from] serde_json::Error),
}

/// Reads one length-prefixed JSON frame.
///
/// Returns `Ok(None)` when the peer cleanly closes before a new frame starts.
///
/// # Errors
///
/// Returns [`FrameError`] for IO errors, oversized frames, or invalid JSON.
pub async fn read_json_frame<R, T>(reader: &mut R) -> Result<Option<T>, FrameError>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let mut len_buf = [0_u8; 4];
    let mut first = [0_u8; 1];
    match reader.read_exact(&mut first).await {
        Ok(_) => len_buf[0] = first[0],
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(FrameError::Io(e)),
    }
    reader.read_exact(&mut len_buf[1..]).await?;

    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(FrameError::FrameTooLarge {
            length: len,
            max: MAX_FRAME_BYTES,
        });
    }

    let mut payload = vec![0_u8; len];
    reader.read_exact(&mut payload).await?;
    let frame = serde_json::from_slice(&payload)?;
    Ok(Some(frame))
}

/// Writes one length-prefixed JSON frame.
///
/// # Errors
///
/// Returns [`FrameError`] if serialisation fails, the frame is too large, or the
/// underlying writer fails.
pub async fn write_json_frame<W, T>(writer: &mut W, frame: &T) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let payload = serde_json::to_vec(frame)?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(FrameError::FrameTooLarge {
            length: payload.len(),
            max: MAX_FRAME_BYTES,
        });
    }
    let len = u32::try_from(payload.len()).map_err(|_| FrameError::FrameTooLarge {
        length: payload.len(),
        max: MAX_FRAME_BYTES,
    })?;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(&payload).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tokio::io::duplex;

    use crate::{read_json_frame, write_json_frame, EngineRequest, PingRequest};

    #[tokio::test]
    async fn frame_round_trips_request() {
        let (mut client, mut server) = duplex(1024);
        let request_id = uuid::Uuid::nil();
        let frame = EngineRequest::Ping(PingRequest { request_id });

        write_json_frame(&mut client, &frame)
            .await
            .expect("write succeeds");
        let decoded: EngineRequest = read_json_frame(&mut server)
            .await
            .expect("read succeeds")
            .expect("frame exists");

        assert_eq!(decoded, frame);
    }
}
