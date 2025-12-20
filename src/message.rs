//! SOME/IP message handling.

use bytes::Bytes;

use crate::error::{Result, SomeIpError};
use crate::header::{ClientId, MethodId, ServiceId, SessionId, SomeIpHeader, HEADER_SIZE};
use crate::types::{MessageType, ReturnCode};

/// Maximum payload size (default: 1400 bytes for UDP compatibility).
pub const DEFAULT_MAX_PAYLOAD_SIZE: usize = 1400;

/// A complete SOME/IP message (header + payload).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SomeIpMessage {
    /// Message header.
    pub header: SomeIpHeader,
    /// Message payload.
    pub payload: Bytes,
}

impl SomeIpMessage {
    /// Create a new message with the given header and payload.
    pub fn new(mut header: SomeIpHeader, payload: impl Into<Bytes>) -> Self {
        let payload = payload.into();
        header.set_payload_length(payload.len() as u32);
        Self { header, payload }
    }

    /// Create a new message with an empty payload.
    pub fn with_header(header: SomeIpHeader) -> Self {
        Self::new(header, Bytes::new())
    }

    /// Create a request message builder.
    pub fn request(service_id: ServiceId, method_id: MethodId) -> MessageBuilder {
        MessageBuilder::new(service_id, method_id, MessageType::Request)
    }

    /// Create a request-no-return message builder.
    pub fn request_no_return(service_id: ServiceId, method_id: MethodId) -> MessageBuilder {
        MessageBuilder::new(service_id, method_id, MessageType::RequestNoReturn)
    }

    /// Create a notification message builder.
    pub fn notification(service_id: ServiceId, method_id: MethodId) -> MessageBuilder {
        MessageBuilder::new(service_id, method_id, MessageType::Notification)
    }

    /// Create a response to this message.
    pub fn create_response(&self) -> MessageBuilder {
        let mut builder = MessageBuilder::new(
            self.header.service_id,
            self.header.method_id,
            MessageType::Response,
        );
        builder.client_id = self.header.client_id;
        builder.session_id = self.header.session_id;
        builder.interface_version = self.header.interface_version;
        builder
    }

    /// Create an error response to this message.
    pub fn create_error_response(&self, return_code: ReturnCode) -> MessageBuilder {
        let mut builder = MessageBuilder::new(
            self.header.service_id,
            self.header.method_id,
            MessageType::Error,
        );
        builder.client_id = self.header.client_id;
        builder.session_id = self.header.session_id;
        builder.interface_version = self.header.interface_version;
        builder.return_code = return_code;
        builder
    }

    /// Parse a message from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_SIZE {
            return Err(SomeIpError::MessageTooShort {
                expected: HEADER_SIZE,
                actual: data.len(),
            });
        }

        let header = SomeIpHeader::from_bytes(data)?;
        let expected_total = HEADER_SIZE + header.payload_length() as usize;

        if data.len() < expected_total {
            return Err(SomeIpError::LengthMismatch {
                header_length: header.length,
                actual_length: data.len() - 8,
            });
        }

        let payload = Bytes::copy_from_slice(&data[HEADER_SIZE..expected_total]);

        Ok(Self { header, payload })
    }

    /// Serialize the message to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HEADER_SIZE + self.payload.len());
        buf.extend_from_slice(&self.header.to_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Get the total message size (header + payload).
    pub fn total_size(&self) -> usize {
        HEADER_SIZE + self.payload.len()
    }

    /// Check if this message is a request.
    pub fn is_request(&self) -> bool {
        matches!(
            self.header.message_type,
            MessageType::Request | MessageType::TpRequest
        )
    }

    /// Check if this message is a response.
    pub fn is_response(&self) -> bool {
        self.header.message_type.is_response()
    }

    /// Check if this message expects a response.
    pub fn expects_response(&self) -> bool {
        self.header.message_type.expects_response()
    }

    /// Get the service ID.
    pub fn service_id(&self) -> ServiceId {
        self.header.service_id
    }

    /// Get the method ID.
    pub fn method_id(&self) -> MethodId {
        self.header.method_id
    }

    /// Get the client ID.
    pub fn client_id(&self) -> ClientId {
        self.header.client_id
    }

    /// Get the session ID.
    pub fn session_id(&self) -> SessionId {
        self.header.session_id
    }

    /// Get the return code.
    pub fn return_code(&self) -> ReturnCode {
        self.header.return_code
    }

    /// Check if the return code indicates success.
    pub fn is_ok(&self) -> bool {
        self.header.return_code.is_ok()
    }
}

/// Builder for constructing SOME/IP messages.
#[derive(Debug, Clone)]
pub struct MessageBuilder {
    service_id: ServiceId,
    method_id: MethodId,
    client_id: ClientId,
    session_id: SessionId,
    interface_version: u8,
    message_type: MessageType,
    return_code: ReturnCode,
    payload: Bytes,
}

impl MessageBuilder {
    /// Create a new message builder.
    pub fn new(service_id: ServiceId, method_id: MethodId, message_type: MessageType) -> Self {
        Self {
            service_id,
            method_id,
            client_id: ClientId::default(),
            session_id: SessionId::default(),
            interface_version: 1,
            message_type,
            return_code: ReturnCode::Ok,
            payload: Bytes::new(),
        }
    }

    /// Set the client ID.
    pub fn client_id(mut self, client_id: ClientId) -> Self {
        self.client_id = client_id;
        self
    }

    /// Set the session ID.
    pub fn session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = session_id;
        self
    }

    /// Set the interface version.
    pub fn interface_version(mut self, version: u8) -> Self {
        self.interface_version = version;
        self
    }

    /// Set the return code.
    pub fn return_code(mut self, code: ReturnCode) -> Self {
        self.return_code = code;
        self
    }

    /// Set the payload from bytes.
    pub fn payload(mut self, payload: impl Into<Bytes>) -> Self {
        self.payload = payload.into();
        self
    }

    /// Set the payload from a Vec<u8>.
    pub fn payload_vec(mut self, payload: Vec<u8>) -> Self {
        self.payload = Bytes::from(payload);
        self
    }

    /// Build the message.
    pub fn build(self) -> SomeIpMessage {
        let header = SomeIpHeader {
            service_id: self.service_id,
            method_id: self.method_id,
            length: 8 + self.payload.len() as u32,
            client_id: self.client_id,
            session_id: self.session_id,
            protocol_version: crate::types::PROTOCOL_VERSION,
            interface_version: self.interface_version,
            message_type: self.message_type,
            return_code: self.return_code,
        };

        SomeIpMessage {
            header,
            payload: self.payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_builder() {
        let msg = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .client_id(ClientId(0x0100))
            .session_id(SessionId(0x0001))
            .payload(b"hello".as_slice())
            .build();

        assert_eq!(msg.header.service_id, ServiceId(0x1234));
        assert_eq!(msg.header.method_id, MethodId(0x0001));
        assert_eq!(msg.header.client_id, ClientId(0x0100));
        assert_eq!(msg.header.session_id, SessionId(0x0001));
        assert_eq!(msg.header.message_type, MessageType::Request);
        assert_eq!(msg.payload.as_ref(), b"hello");
        assert_eq!(msg.header.length, 8 + 5); // 8 + payload length
    }

    #[test]
    fn test_message_roundtrip() {
        let original = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x5678))
            .client_id(ClientId(0xABCD))
            .session_id(SessionId(0x0001))
            .payload(vec![1, 2, 3, 4, 5])
            .build();

        let bytes = original.to_bytes();
        let parsed = SomeIpMessage::from_bytes(&bytes).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_create_response() {
        let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .client_id(ClientId(0x0100))
            .session_id(SessionId(0x0042))
            .build();

        let response = request
            .create_response()
            .payload(b"response data".as_slice())
            .build();

        assert_eq!(response.header.service_id, request.header.service_id);
        assert_eq!(response.header.method_id, request.header.method_id);
        assert_eq!(response.header.client_id, request.header.client_id);
        assert_eq!(response.header.session_id, request.header.session_id);
        assert_eq!(response.header.message_type, MessageType::Response);
    }

    #[test]
    fn test_create_error_response() {
        let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .client_id(ClientId(0x0100))
            .session_id(SessionId(0x0042))
            .build();

        let error = request
            .create_error_response(ReturnCode::UnknownMethod)
            .build();

        assert_eq!(error.header.message_type, MessageType::Error);
        assert_eq!(error.header.return_code, ReturnCode::UnknownMethod);
    }

    #[test]
    fn test_total_size() {
        let msg = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(vec![0u8; 100])
            .build();

        assert_eq!(msg.total_size(), HEADER_SIZE + 100);
    }

    #[test]
    fn test_parse_too_short() {
        let data = vec![0u8; 10];
        let result = SomeIpMessage::from_bytes(&data);
        assert!(matches!(result, Err(SomeIpError::MessageTooShort { .. })));
    }
}
