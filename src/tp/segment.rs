//! SOME/IP-TP segment handling.

use bytes::Bytes;

use crate::error::{Result, SomeIpError};
use crate::header::{SomeIpHeader, HEADER_SIZE};
use crate::message::SomeIpMessage;
use crate::types::MessageType;

use super::header::{TpHeader, TP_HEADER_SIZE};

/// Default maximum segment payload size.
///
/// This is calculated as: MTU (1500) - IP header (20) - UDP header (8)
/// - SOME/IP header (16) - TP header (4) = 1452, rounded down to 1392
/// for alignment to 16-byte boundaries.
pub const DEFAULT_MAX_SEGMENT_PAYLOAD: usize = 1392;

/// A single TP segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TpSegment {
    /// SOME/IP header (with TP message type).
    pub header: SomeIpHeader,
    /// TP header.
    pub tp_header: TpHeader,
    /// Segment payload (portion of original payload).
    pub payload: Bytes,
}

impl TpSegment {
    /// Create a new TP segment.
    pub fn new(header: SomeIpHeader, tp_header: TpHeader, payload: Bytes) -> Self {
        Self {
            header,
            tp_header,
            payload,
        }
    }

    /// Parse a TP segment from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let min_size = HEADER_SIZE + TP_HEADER_SIZE;
        if data.len() < min_size {
            return Err(SomeIpError::MessageTooShort {
                expected: min_size,
                actual: data.len(),
            });
        }

        let header = SomeIpHeader::from_bytes(&data[..HEADER_SIZE])?;

        if !header.message_type.is_tp() {
            return Err(SomeIpError::invalid_header("Expected TP message type"));
        }

        let tp_header = TpHeader::from_bytes(&data[HEADER_SIZE..HEADER_SIZE + TP_HEADER_SIZE])?;

        // Payload length from SOME/IP header includes TP header + payload
        let payload_start = HEADER_SIZE + TP_HEADER_SIZE;
        let payload = Bytes::copy_from_slice(&data[payload_start..]);

        Ok(Self {
            header,
            tp_header,
            payload,
        })
    }

    /// Serialize the segment to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HEADER_SIZE + TP_HEADER_SIZE + self.payload.len());
        buf.extend_from_slice(&self.header.to_bytes());
        buf.extend_from_slice(&self.tp_header.to_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Get the byte offset of this segment in the original message.
    pub fn byte_offset(&self) -> usize {
        self.tp_header.byte_offset()
    }

    /// Check if this is the last segment.
    pub fn is_last(&self) -> bool {
        !self.tp_header.more
    }
}

/// Segment a large message into TP segments.
///
/// Returns an empty vector if the message doesn't need segmentation.
pub fn segment_message(message: &SomeIpMessage, max_segment_payload: usize) -> Vec<TpSegment> {
    let payload = &message.payload;

    // No segmentation needed for small messages
    if payload.len() <= max_segment_payload {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let mut offset: usize = 0;

    while offset < payload.len() {
        let remaining = payload.len() - offset;
        let segment_size = remaining.min(max_segment_payload);
        let is_last = offset + segment_size >= payload.len();

        // Create TP header
        let tp_header = TpHeader::from_byte_offset(offset, !is_last);

        // Create SOME/IP header with TP message type
        let mut header = message.header.clone();
        header.message_type = message
            .header
            .message_type
            .to_tp()
            .unwrap_or(MessageType::TpRequest);

        // Update length: includes SOME/IP request ID (8 bytes) + TP header + segment payload
        header.length = 8 + TP_HEADER_SIZE as u32 + segment_size as u32;

        // Extract segment payload
        let segment_payload = payload.slice(offset..offset + segment_size);

        segments.push(TpSegment::new(header, tp_header, segment_payload));

        offset += segment_size;
    }

    segments
}

/// Check if a message needs TP segmentation.
pub fn needs_segmentation(message: &SomeIpMessage, max_segment_payload: usize) -> bool {
    message.payload.len() > max_segment_payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{MethodId, ServiceId};

    #[test]
    fn test_segment_small_message() {
        let msg = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"small".as_slice())
            .build();

        let segments = segment_message(&msg, DEFAULT_MAX_SEGMENT_PAYLOAD);
        assert!(segments.is_empty());
    }

    #[test]
    fn test_segment_large_message() {
        // Create a message larger than max segment size
        let msg = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload_vec(vec![0xABu8; 3000])
            .build();

        let segments = segment_message(&msg, 1392);

        // Should create 3 segments: 1392 + 1392 + 216 = 3000
        assert_eq!(segments.len(), 3);

        // First segment
        assert_eq!(segments[0].tp_header.offset, 0);
        assert!(segments[0].tp_header.more);
        assert_eq!(segments[0].payload.len(), 1392);
        assert!(segments[0].header.message_type.is_tp());

        // Second segment
        assert_eq!(segments[1].tp_header.offset, 87); // 1392/16
        assert!(segments[1].tp_header.more);
        assert_eq!(segments[1].payload.len(), 1392);

        // Third (last) segment
        assert_eq!(segments[2].tp_header.offset, 174); // 2784/16
        assert!(!segments[2].tp_header.more);
        assert_eq!(segments[2].payload.len(), 216);
    }

    #[test]
    fn test_segment_roundtrip() {
        let msg = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload_vec(vec![0xCDu8; 2000])
            .build();

        let segments = segment_message(&msg, 1392);

        // Serialize and parse each segment
        for segment in segments {
            let bytes = segment.to_bytes();
            let parsed = TpSegment::from_bytes(&bytes).unwrap();

            assert_eq!(segment.tp_header, parsed.tp_header);
            assert_eq!(segment.payload, parsed.payload);
        }
    }

    #[test]
    fn test_needs_segmentation() {
        let small = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"small".as_slice())
            .build();

        let large = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload_vec(vec![0u8; 2000])
            .build();

        assert!(!needs_segmentation(&small, 1392));
        assert!(needs_segmentation(&large, 1392));
    }
}
