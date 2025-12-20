//! SOME/IP header types and ID newtypes.

use crate::error::{Result, SomeIpError};
use crate::types::{MessageType, ReturnCode, PROTOCOL_VERSION};

/// Size of the SOME/IP header in bytes.
pub const HEADER_SIZE: usize = 16;

/// Service ID - identifies a SOME/IP service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ServiceId(pub u16);

/// Method ID - identifies a method within a service.
/// Bit 15 indicates if this is an event (1) or method (0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MethodId(pub u16);

/// Client ID - identifies the client making a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ClientId(pub u16);

/// Session ID - unique identifier for a request/response pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SessionId(pub u16);

impl MethodId {
    /// Check if this method ID represents an event (bit 15 set).
    pub fn is_event(&self) -> bool {
        self.0 & 0x8000 != 0
    }

    /// Create a method ID for an event.
    pub fn event(id: u16) -> Self {
        Self(id | 0x8000)
    }

    /// Create a method ID for a regular method.
    pub fn method(id: u16) -> Self {
        Self(id & 0x7FFF)
    }
}

impl std::fmt::Display for ServiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:04X}", self.0)
    }
}

impl std::fmt::Display for MethodId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:04X}", self.0)
    }
}

impl std::fmt::Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:04X}", self.0)
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:04X}", self.0)
    }
}

/// SOME/IP message header (16 bytes).
///
/// ```text
/// +----------------+----------------+----------------+----------------+
/// |           Message ID (32 bits)                                   |
/// |   Service ID (16 bits)  |  Method ID (16 bits)                   |
/// +----------------+----------------+----------------+----------------+
/// |           Length (32 bits) - payload length + 8                  |
/// +----------------+----------------+----------------+----------------+
/// |           Request ID (32 bits)                                   |
/// |   Client ID (16 bits)   |  Session ID (16 bits)                  |
/// +----------------+----------------+----------------+----------------+
/// | Protocol Ver | Interface Ver | Message Type | Return Code        |
/// | (8 bits)     | (8 bits)      | (8 bits)     | (8 bits)           |
/// +----------------+----------------+----------------+----------------+
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SomeIpHeader {
    /// Service ID.
    pub service_id: ServiceId,
    /// Method ID.
    pub method_id: MethodId,
    /// Length of payload + 8 bytes (client_id, session_id, protocol_version, interface_version, message_type, return_code).
    pub length: u32,
    /// Client ID.
    pub client_id: ClientId,
    /// Session ID.
    pub session_id: SessionId,
    /// Protocol version (should be 0x01).
    pub protocol_version: u8,
    /// Interface version.
    pub interface_version: u8,
    /// Message type.
    pub message_type: MessageType,
    /// Return code.
    pub return_code: ReturnCode,
}

impl SomeIpHeader {
    /// Create a new header with the given service and method IDs.
    pub fn new(service_id: ServiceId, method_id: MethodId) -> Self {
        Self {
            service_id,
            method_id,
            length: 8, // Minimum length (no payload)
            client_id: ClientId::default(),
            session_id: SessionId::default(),
            protocol_version: PROTOCOL_VERSION,
            interface_version: 1,
            message_type: MessageType::Request,
            return_code: ReturnCode::Ok,
        }
    }

    /// Create a request header.
    pub fn request(service_id: ServiceId, method_id: MethodId) -> Self {
        let mut header = Self::new(service_id, method_id);
        header.message_type = MessageType::Request;
        header
    }

    /// Create a request-no-return header.
    pub fn request_no_return(service_id: ServiceId, method_id: MethodId) -> Self {
        let mut header = Self::new(service_id, method_id);
        header.message_type = MessageType::RequestNoReturn;
        header
    }

    /// Create a notification header.
    pub fn notification(service_id: ServiceId, method_id: MethodId) -> Self {
        let mut header = Self::new(service_id, method_id);
        header.message_type = MessageType::Notification;
        header
    }

    /// Create a response header from a request header.
    pub fn response_from(request: &Self) -> Self {
        Self {
            service_id: request.service_id,
            method_id: request.method_id,
            length: 8,
            client_id: request.client_id,
            session_id: request.session_id,
            protocol_version: PROTOCOL_VERSION,
            interface_version: request.interface_version,
            message_type: MessageType::Response,
            return_code: ReturnCode::Ok,
        }
    }

    /// Create an error response header from a request header.
    pub fn error_from(request: &Self, return_code: ReturnCode) -> Self {
        Self {
            service_id: request.service_id,
            method_id: request.method_id,
            length: 8,
            client_id: request.client_id,
            session_id: request.session_id,
            protocol_version: PROTOCOL_VERSION,
            interface_version: request.interface_version,
            message_type: MessageType::Error,
            return_code,
        }
    }

    /// Get the payload length (length field minus 8).
    pub fn payload_length(&self) -> u32 {
        self.length.saturating_sub(8)
    }

    /// Set the payload length (updates length field to payload_len + 8).
    pub fn set_payload_length(&mut self, payload_len: u32) {
        self.length = payload_len + 8;
    }

    /// Parse a header from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_SIZE {
            return Err(SomeIpError::MessageTooShort {
                expected: HEADER_SIZE,
                actual: data.len(),
            });
        }

        let service_id = ServiceId(u16::from_be_bytes([data[0], data[1]]));
        let method_id = MethodId(u16::from_be_bytes([data[2], data[3]]));
        let length = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let client_id = ClientId(u16::from_be_bytes([data[8], data[9]]));
        let session_id = SessionId(u16::from_be_bytes([data[10], data[11]]));
        let protocol_version = data[12];
        let interface_version = data[13];

        if protocol_version != PROTOCOL_VERSION {
            return Err(SomeIpError::WrongProtocolVersion(protocol_version));
        }

        let message_type = MessageType::from_u8(data[14])
            .ok_or(SomeIpError::UnknownMessageType(data[14]))?;
        let return_code =
            ReturnCode::from_u8(data[15]).ok_or(SomeIpError::UnknownReturnCode(data[15]))?;

        Ok(Self {
            service_id,
            method_id,
            length,
            client_id,
            session_id,
            protocol_version,
            interface_version,
            message_type,
            return_code,
        })
    }

    /// Serialize the header to bytes.
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];

        buf[0..2].copy_from_slice(&self.service_id.0.to_be_bytes());
        buf[2..4].copy_from_slice(&self.method_id.0.to_be_bytes());
        buf[4..8].copy_from_slice(&self.length.to_be_bytes());
        buf[8..10].copy_from_slice(&self.client_id.0.to_be_bytes());
        buf[10..12].copy_from_slice(&self.session_id.0.to_be_bytes());
        buf[12] = self.protocol_version;
        buf[13] = self.interface_version;
        buf[14] = self.message_type as u8;
        buf[15] = self.return_code as u8;

        buf
    }

    /// Get the message ID (service_id << 16 | method_id).
    pub fn message_id(&self) -> u32 {
        ((self.service_id.0 as u32) << 16) | (self.method_id.0 as u32)
    }

    /// Get the request ID (client_id << 16 | session_id).
    pub fn request_id(&self) -> u32 {
        ((self.client_id.0 as u32) << 16) | (self.session_id.0 as u32)
    }
}

impl Default for SomeIpHeader {
    fn default() -> Self {
        Self::new(ServiceId(0), MethodId(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_roundtrip() {
        let header = SomeIpHeader {
            service_id: ServiceId(0x1234),
            method_id: MethodId(0x5678),
            length: 16,
            client_id: ClientId(0xABCD),
            session_id: SessionId(0xEF01),
            protocol_version: PROTOCOL_VERSION,
            interface_version: 2,
            message_type: MessageType::Request,
            return_code: ReturnCode::Ok,
        };

        let bytes = header.to_bytes();
        let parsed = SomeIpHeader::from_bytes(&bytes).unwrap();

        assert_eq!(header, parsed);
    }

    #[test]
    fn test_header_byte_order() {
        let header = SomeIpHeader {
            service_id: ServiceId(0x1234),
            method_id: MethodId(0x5678),
            length: 8,
            client_id: ClientId(0x0000),
            session_id: SessionId(0x0001),
            protocol_version: PROTOCOL_VERSION,
            interface_version: 1,
            message_type: MessageType::Request,
            return_code: ReturnCode::Ok,
        };

        let bytes = header.to_bytes();

        // Check big-endian order
        assert_eq!(bytes[0], 0x12); // Service ID high byte
        assert_eq!(bytes[1], 0x34); // Service ID low byte
        assert_eq!(bytes[2], 0x56); // Method ID high byte
        assert_eq!(bytes[3], 0x78); // Method ID low byte
    }

    #[test]
    fn test_method_id_event() {
        let event = MethodId::event(0x1234);
        assert!(event.is_event());
        assert_eq!(event.0, 0x9234);

        let method = MethodId::method(0x9234);
        assert!(!method.is_event());
        assert_eq!(method.0, 0x1234);
    }

    #[test]
    fn test_response_from() {
        let request = SomeIpHeader {
            service_id: ServiceId(0x1234),
            method_id: MethodId(0x0001),
            length: 20,
            client_id: ClientId(0x0100),
            session_id: SessionId(0x0001),
            protocol_version: PROTOCOL_VERSION,
            interface_version: 1,
            message_type: MessageType::Request,
            return_code: ReturnCode::Ok,
        };

        let response = SomeIpHeader::response_from(&request);

        assert_eq!(response.service_id, request.service_id);
        assert_eq!(response.method_id, request.method_id);
        assert_eq!(response.client_id, request.client_id);
        assert_eq!(response.session_id, request.session_id);
        assert_eq!(response.message_type, MessageType::Response);
    }

    #[test]
    fn test_parse_too_short() {
        let data = [0u8; 10];
        let result = SomeIpHeader::from_bytes(&data);
        assert!(matches!(result, Err(SomeIpError::MessageTooShort { .. })));
    }

    #[test]
    fn test_parse_wrong_protocol_version() {
        let mut header = SomeIpHeader::default();
        header.protocol_version = 0x02;
        let mut bytes = header.to_bytes();
        bytes[12] = 0x02; // Wrong protocol version

        let result = SomeIpHeader::from_bytes(&bytes);
        assert!(matches!(result, Err(SomeIpError::WrongProtocolVersion(0x02))));
    }
}
