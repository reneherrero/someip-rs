//! Error types for SOME/IP operations.

use crate::types::ReturnCode;
use std::io;
use thiserror::Error;

/// Errors that can occur during SOME/IP operations.
#[derive(Error, Debug)]
pub enum SomeIpError {
    /// I/O error during network operations.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Invalid message header.
    #[error("Invalid header: {0}")]
    InvalidHeader(String),

    /// Unknown message type value.
    #[error("Unknown message type: 0x{0:02X}")]
    UnknownMessageType(u8),

    /// Unknown return code value.
    #[error("Unknown return code: 0x{0:02X}")]
    UnknownReturnCode(u8),

    /// Wrong protocol version.
    #[error("Wrong protocol version: expected 0x01, got 0x{0:02X}")]
    WrongProtocolVersion(u8),

    /// Message too short to contain header.
    #[error("Message too short: expected at least {expected} bytes, got {actual}")]
    MessageTooShort { expected: usize, actual: usize },

    /// Message length mismatch.
    #[error("Message length mismatch: header says {header_length} bytes, got {actual_length}")]
    LengthMismatch {
        header_length: u32,
        actual_length: usize,
    },

    /// Payload too large.
    #[error("Payload too large: {size} bytes exceeds maximum of {max} bytes")]
    PayloadTooLarge { size: usize, max: usize },

    /// Protocol error returned by remote.
    #[error("Protocol error: {0:?}")]
    ProtocolError(ReturnCode),

    /// Connection closed unexpectedly.
    #[error("Connection closed")]
    ConnectionClosed,

    /// Operation timed out.
    #[error("Operation timed out")]
    Timeout,

    /// No response received for request.
    #[error("No response received for request (client={client_id:04X}, session={session_id:04X})")]
    NoResponse { client_id: u16, session_id: u16 },
}

/// Result type alias for SOME/IP operations.
pub type Result<T> = std::result::Result<T, SomeIpError>;

impl SomeIpError {
    /// Create a new invalid header error.
    pub fn invalid_header(msg: impl Into<String>) -> Self {
        Self::InvalidHeader(msg.into())
    }

    /// Check if this error is recoverable (transient).
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Io(e) if e.kind() == io::ErrorKind::WouldBlock
                || e.kind() == io::ErrorKind::TimedOut
                || e.kind() == io::ErrorKind::Interrupted
        ) || matches!(self, Self::Timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SomeIpError::UnknownMessageType(0xFF);
        assert_eq!(format!("{err}"), "Unknown message type: 0xFF");

        let err = SomeIpError::MessageTooShort {
            expected: 16,
            actual: 8,
        };
        assert_eq!(
            format!("{err}"),
            "Message too short: expected at least 16 bytes, got 8"
        );
    }

    #[test]
    fn test_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::ConnectionRefused, "test");
        let err: SomeIpError = io_err.into();
        assert!(matches!(err, SomeIpError::Io(_)));
    }
}
