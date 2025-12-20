//! Async SOME/IP message framing and codec utilities.
//!
//! This module provides async versions of the codec functions for use with tokio.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::Result;
use crate::header::{SomeIpHeader, HEADER_SIZE};
use crate::message::SomeIpMessage;

/// Read a complete SOME/IP message from an async stream.
///
/// This function handles TCP framing by first reading the header,
/// then reading the payload based on the length field.
pub async fn read_message_async<R: AsyncRead + Unpin>(reader: &mut R) -> Result<SomeIpMessage> {
    // Read header
    let mut header_buf = [0u8; HEADER_SIZE];
    reader.read_exact(&mut header_buf).await?;

    let header = SomeIpHeader::from_bytes(&header_buf)?;
    let payload_len = header.payload_length() as usize;

    // Read payload
    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        reader.read_exact(&mut payload).await?;
    }

    Ok(SomeIpMessage::new(header, payload))
}

/// Write a complete SOME/IP message to an async stream.
pub async fn write_message_async<W: AsyncWrite + Unpin>(
    writer: &mut W,
    message: &SomeIpMessage,
) -> Result<()> {
    writer.write_all(&message.header.to_bytes()).await?;
    writer.write_all(&message.payload).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{MethodId, ServiceId};
    use std::io::Cursor;

    #[tokio::test]
    async fn test_async_read_write_message() {
        let original = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"test payload".as_slice())
            .build();

        // Write to buffer
        let mut buffer = Vec::new();
        write_message_async(&mut buffer, &original).await.unwrap();

        // Read back
        let mut cursor = Cursor::new(buffer);
        let parsed = read_message_async(&mut cursor).await.unwrap();

        assert_eq!(original, parsed);
    }

    #[tokio::test]
    async fn test_async_read_empty_payload() {
        let original = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001)).build();

        let mut buffer = Vec::new();
        write_message_async(&mut buffer, &original).await.unwrap();

        let mut cursor = Cursor::new(buffer);
        let parsed = read_message_async(&mut cursor).await.unwrap();

        assert_eq!(original, parsed);
        assert!(parsed.payload.is_empty());
    }
}
