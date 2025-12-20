//! SOME/IP message framing and codec utilities.

use std::io::{Read, Write};

use crate::error::Result;
use crate::header::{SomeIpHeader, HEADER_SIZE};
use crate::message::SomeIpMessage;

/// Read a complete SOME/IP message from a stream.
///
/// This function handles TCP framing by first reading the header,
/// then reading the payload based on the length field.
pub fn read_message<R: Read>(reader: &mut R) -> Result<SomeIpMessage> {
    // Read header
    let mut header_buf = [0u8; HEADER_SIZE];
    reader.read_exact(&mut header_buf)?;

    let header = SomeIpHeader::from_bytes(&header_buf)?;
    let payload_len = header.payload_length() as usize;

    // Read payload
    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        reader.read_exact(&mut payload)?;
    }

    Ok(SomeIpMessage::new(header, payload))
}

/// Write a complete SOME/IP message to a stream.
pub fn write_message<W: Write>(writer: &mut W, message: &SomeIpMessage) -> Result<()> {
    writer.write_all(&message.header.to_bytes())?;
    writer.write_all(&message.payload)?;
    Ok(())
}

/// A buffered reader for SOME/IP messages.
///
/// This handles partial reads and accumulates data until a complete
/// message is available.
#[derive(Debug)]
pub struct MessageReader {
    buffer: Vec<u8>,
    position: usize,
}

impl MessageReader {
    /// Create a new message reader.
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(4096),
            position: 0,
        }
    }

    /// Create a new message reader with a specific buffer capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            position: 0,
        }
    }

    /// Add data to the internal buffer.
    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Try to parse a complete message from the buffer.
    ///
    /// Returns `Some(message)` if a complete message is available,
    /// `None` if more data is needed.
    pub fn try_parse(&mut self) -> Result<Option<SomeIpMessage>> {
        let available = self.buffer.len() - self.position;

        // Need at least header
        if available < HEADER_SIZE {
            return Ok(None);
        }

        // Parse header to get length
        let header_data = &self.buffer[self.position..self.position + HEADER_SIZE];
        let header = SomeIpHeader::from_bytes(header_data)?;
        let total_len = HEADER_SIZE + header.payload_length() as usize;

        // Check if we have the complete message
        if available < total_len {
            return Ok(None);
        }

        // Extract complete message
        let message_data = &self.buffer[self.position..self.position + total_len];
        let message = SomeIpMessage::from_bytes(message_data)?;

        self.position += total_len;

        // Compact buffer if needed
        if self.position > self.buffer.len() / 2 {
            self.compact();
        }

        Ok(Some(message))
    }

    /// Parse all complete messages from the buffer.
    pub fn parse_all(&mut self) -> Result<Vec<SomeIpMessage>> {
        let mut messages = Vec::new();
        while let Some(msg) = self.try_parse()? {
            messages.push(msg);
        }
        Ok(messages)
    }

    /// Compact the buffer by removing consumed data.
    fn compact(&mut self) {
        if self.position > 0 {
            self.buffer.drain(..self.position);
            self.position = 0;
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.position = 0;
    }

    /// Get the number of bytes in the buffer.
    pub fn len(&self) -> usize {
        self.buffer.len() - self.position
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for MessageReader {
    fn default() -> Self {
        Self::new()
    }
}

/// A writer that frames SOME/IP messages.
#[derive(Debug)]
pub struct MessageWriter {
    buffer: Vec<u8>,
}

impl MessageWriter {
    /// Create a new message writer.
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(4096),
        }
    }

    /// Encode a message into the internal buffer.
    pub fn encode(&mut self, message: &SomeIpMessage) {
        self.buffer.extend_from_slice(&message.header.to_bytes());
        self.buffer.extend_from_slice(&message.payload);
    }

    /// Get the encoded data.
    pub fn data(&self) -> &[u8] {
        &self.buffer
    }

    /// Take the encoded data, clearing the internal buffer.
    pub fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buffer)
    }

    /// Clear the internal buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for MessageWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{MethodId, ServiceId};

    #[test]
    fn test_read_write_message() {
        let original = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"test payload".as_slice())
            .build();

        let mut buffer = Vec::new();
        write_message(&mut buffer, &original).unwrap();

        let mut cursor = std::io::Cursor::new(buffer);
        let parsed = read_message(&mut cursor).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_message_reader_complete() {
        let msg = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"hello".as_slice())
            .build();

        let data = msg.to_bytes();

        let mut reader = MessageReader::new();
        reader.feed(&data);

        let parsed = reader.try_parse().unwrap();
        assert!(parsed.is_some());
        assert_eq!(parsed.unwrap(), msg);
    }

    #[test]
    fn test_message_reader_partial() {
        let msg = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"hello".as_slice())
            .build();

        let data = msg.to_bytes();

        let mut reader = MessageReader::new();

        // Feed partial data
        reader.feed(&data[..10]);
        assert!(reader.try_parse().unwrap().is_none());

        // Feed remaining data
        reader.feed(&data[10..]);
        let parsed = reader.try_parse().unwrap();
        assert!(parsed.is_some());
        assert_eq!(parsed.unwrap(), msg);
    }

    #[test]
    fn test_message_reader_multiple() {
        let msg1 = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"first".as_slice())
            .build();
        let msg2 = SomeIpMessage::request(ServiceId(0x5678), MethodId(0x0002))
            .payload(b"second".as_slice())
            .build();

        let mut data = msg1.to_bytes();
        data.extend_from_slice(&msg2.to_bytes());

        let mut reader = MessageReader::new();
        reader.feed(&data);

        let messages = reader.parse_all().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0], msg1);
        assert_eq!(messages[1], msg2);
    }

    #[test]
    fn test_message_writer() {
        let msg = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"test".as_slice())
            .build();

        let mut writer = MessageWriter::new();
        writer.encode(&msg);

        let data = writer.take();
        assert_eq!(data, msg.to_bytes());
    }
}
