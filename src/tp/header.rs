//! SOME/IP-TP header definition.

use crate::error::{Result, SomeIpError};

/// Size of the TP header in bytes.
pub const TP_HEADER_SIZE: usize = 4;

/// TP header (4 bytes after SOME/IP header for segmented messages).
///
/// Format:
/// ```text
/// +----------------+----------------+----------------+----------------+
/// |                    Offset (28 bits)              | Res(3) | M(1)  |
/// +----------------+----------------+----------------+----------------+
/// ```
///
/// - Offset: Position in original payload in 16-byte units
/// - Reserved: 3 bits, must be 0
/// - More flag: 1 bit (1 = more segments follow, 0 = last segment)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TpHeader {
    /// Offset in 16-byte units (28 bits).
    pub offset: u32,
    /// More segments flag.
    pub more: bool,
}

impl TpHeader {
    /// Create a new TP header.
    pub fn new(offset: u32, more: bool) -> Self {
        Self { offset, more }
    }

    /// Create a TP header for the first segment.
    pub fn first(more: bool) -> Self {
        Self { offset: 0, more }
    }

    /// Create from byte offset (divides by 16).
    pub fn from_byte_offset(byte_offset: usize, more: bool) -> Self {
        Self {
            offset: (byte_offset / 16) as u32,
            more,
        }
    }

    /// Get the actual byte offset (offset * 16).
    pub fn byte_offset(&self) -> usize {
        (self.offset as usize) * 16
    }

    /// Parse a TP header from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < TP_HEADER_SIZE {
            return Err(SomeIpError::MessageTooShort {
                expected: TP_HEADER_SIZE,
                actual: data.len(),
            });
        }

        // Offset is in bits 0-27 (28 bits), big-endian
        // Reserved is bits 28-30 (3 bits)
        // More flag is bit 31 (1 bit)
        let value = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);

        let offset = value >> 4; // Upper 28 bits
        let more = (value & 0x01) != 0; // Lowest bit

        Ok(Self { offset, more })
    }

    /// Serialize the TP header to bytes.
    pub fn to_bytes(&self) -> [u8; TP_HEADER_SIZE] {
        // Offset in upper 28 bits, reserved 3 bits, more flag in lowest bit
        let value = (self.offset << 4) | (if self.more { 0x01 } else { 0x00 });
        value.to_be_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tp_header_first_segment() {
        let header = TpHeader::first(true);
        assert_eq!(header.offset, 0);
        assert!(header.more);
        assert_eq!(header.byte_offset(), 0);
    }

    #[test]
    fn test_tp_header_from_byte_offset() {
        // 1392 bytes / 16 = 87
        let header = TpHeader::from_byte_offset(1392, true);
        assert_eq!(header.offset, 87);
        assert!(header.more);

        // Byte offset rounds down to 16-byte boundary
        let header = TpHeader::from_byte_offset(1400, false);
        assert_eq!(header.offset, 87); // 1400/16 = 87.5 -> 87
        assert!(!header.more);
    }

    #[test]
    fn test_tp_header_roundtrip() {
        let original = TpHeader::new(12345, true);
        let bytes = original.to_bytes();
        let parsed = TpHeader::from_bytes(&bytes).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_tp_header_roundtrip_last_segment() {
        let original = TpHeader::new(99999, false);
        let bytes = original.to_bytes();
        let parsed = TpHeader::from_bytes(&bytes).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_tp_header_byte_offset() {
        let header = TpHeader::new(100, true);
        assert_eq!(header.byte_offset(), 1600); // 100 * 16
    }

    #[test]
    fn test_tp_header_too_short() {
        let result = TpHeader::from_bytes(&[0, 1, 2]);
        assert!(result.is_err());
    }
}
