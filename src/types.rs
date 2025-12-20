//! Core SOME/IP types and constants.

/// SOME/IP protocol version (always 0x01).
pub const PROTOCOL_VERSION: u8 = 0x01;

/// SOME/IP message types as defined in the specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageType {
    /// Request expecting a response.
    Request = 0x00,
    /// Request not expecting a response (fire-and-forget).
    RequestNoReturn = 0x01,
    /// Cyclic or event-based notification.
    Notification = 0x02,
    /// Response to a request.
    Response = 0x80,
    /// Error response to a request.
    Error = 0x81,
    /// TP Request (segmented).
    TpRequest = 0x20,
    /// TP Request not expecting a response.
    TpRequestNoReturn = 0x21,
    /// TP Notification.
    TpNotification = 0x22,
    /// TP Response.
    TpResponse = 0xA0,
    /// TP Error.
    TpError = 0xA1,
}

impl MessageType {
    /// Create a MessageType from a raw byte value.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Request),
            0x01 => Some(Self::RequestNoReturn),
            0x02 => Some(Self::Notification),
            0x80 => Some(Self::Response),
            0x81 => Some(Self::Error),
            0x20 => Some(Self::TpRequest),
            0x21 => Some(Self::TpRequestNoReturn),
            0x22 => Some(Self::TpNotification),
            0xA0 => Some(Self::TpResponse),
            0xA1 => Some(Self::TpError),
            _ => None,
        }
    }

    /// Check if this message type expects a response.
    pub fn expects_response(&self) -> bool {
        matches!(self, Self::Request | Self::TpRequest)
    }

    /// Check if this is a response message type.
    pub fn is_response(&self) -> bool {
        matches!(
            self,
            Self::Response | Self::Error | Self::TpResponse | Self::TpError
        )
    }

    /// Check if this is a TP (Transport Protocol) segmented message.
    pub fn is_tp(&self) -> bool {
        matches!(
            self,
            Self::TpRequest
                | Self::TpRequestNoReturn
                | Self::TpNotification
                | Self::TpResponse
                | Self::TpError
        )
    }
}

/// SOME/IP return codes as defined in the specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ReturnCode {
    /// No error occurred.
    Ok = 0x00,
    /// An unspecified error occurred.
    NotOk = 0x01,
    /// The requested Service ID is unknown.
    UnknownService = 0x02,
    /// The requested Method ID is unknown.
    UnknownMethod = 0x03,
    /// Service is not ready.
    NotReady = 0x04,
    /// Service is not reachable.
    NotReachable = 0x05,
    /// Timeout occurred.
    Timeout = 0x06,
    /// Wrong protocol version.
    WrongProtocolVersion = 0x07,
    /// Wrong interface version.
    WrongInterfaceVersion = 0x08,
    /// Malformed message.
    MalformedMessage = 0x09,
    /// Wrong message type.
    WrongMessageType = 0x0A,
    /// E2E repeated.
    E2ERepeated = 0x0B,
    /// E2E wrong sequence.
    E2EWrongSequence = 0x0C,
    /// E2E error (not further specified).
    E2E = 0x0D,
    /// E2E not available.
    E2ENotAvailable = 0x0E,
    /// E2E no new data.
    E2ENoNewData = 0x0F,
}

impl ReturnCode {
    /// Create a ReturnCode from a raw byte value.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Ok),
            0x01 => Some(Self::NotOk),
            0x02 => Some(Self::UnknownService),
            0x03 => Some(Self::UnknownMethod),
            0x04 => Some(Self::NotReady),
            0x05 => Some(Self::NotReachable),
            0x06 => Some(Self::Timeout),
            0x07 => Some(Self::WrongProtocolVersion),
            0x08 => Some(Self::WrongInterfaceVersion),
            0x09 => Some(Self::MalformedMessage),
            0x0A => Some(Self::WrongMessageType),
            0x0B => Some(Self::E2ERepeated),
            0x0C => Some(Self::E2EWrongSequence),
            0x0D => Some(Self::E2E),
            0x0E => Some(Self::E2ENotAvailable),
            0x0F => Some(Self::E2ENoNewData),
            _ => None,
        }
    }

    /// Check if this return code indicates success.
    pub fn is_ok(&self) -> bool {
        *self == Self::Ok
    }

    /// Check if this return code indicates an error.
    pub fn is_error(&self) -> bool {
        *self != Self::Ok
    }
}

impl Default for ReturnCode {
    fn default() -> Self {
        Self::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_from_u8() {
        assert_eq!(MessageType::from_u8(0x00), Some(MessageType::Request));
        assert_eq!(MessageType::from_u8(0x80), Some(MessageType::Response));
        assert_eq!(MessageType::from_u8(0xFF), None);
    }

    #[test]
    fn test_message_type_expects_response() {
        assert!(MessageType::Request.expects_response());
        assert!(!MessageType::RequestNoReturn.expects_response());
        assert!(!MessageType::Notification.expects_response());
        assert!(!MessageType::Response.expects_response());
    }

    #[test]
    fn test_return_code_from_u8() {
        assert_eq!(ReturnCode::from_u8(0x00), Some(ReturnCode::Ok));
        assert_eq!(ReturnCode::from_u8(0x02), Some(ReturnCode::UnknownService));
        assert_eq!(ReturnCode::from_u8(0xFF), None);
    }

    #[test]
    fn test_return_code_is_ok() {
        assert!(ReturnCode::Ok.is_ok());
        assert!(!ReturnCode::NotOk.is_ok());
        assert!(!ReturnCode::Timeout.is_ok());
    }
}
