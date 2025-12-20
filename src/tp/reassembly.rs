//! SOME/IP-TP message reassembly.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use bytes::{BufMut, BytesMut};

use crate::error::{Result, SomeIpError};
use crate::header::{ClientId, MethodId, ServiceId, SessionId, SomeIpHeader};
use crate::message::SomeIpMessage;

use super::segment::TpSegment;

/// Default timeout for reassembly contexts.
pub const DEFAULT_REASSEMBLY_TIMEOUT: Duration = Duration::from_secs(5);

/// Key for identifying a reassembly context.
///
/// A unique message is identified by its service ID, method ID, client ID, and session ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReassemblyKey {
    /// Service ID.
    pub service_id: ServiceId,
    /// Method ID.
    pub method_id: MethodId,
    /// Client ID.
    pub client_id: ClientId,
    /// Session ID.
    pub session_id: SessionId,
}

impl ReassemblyKey {
    /// Create a new reassembly key from a SOME/IP header.
    pub fn from_header(header: &SomeIpHeader) -> Self {
        Self {
            service_id: header.service_id,
            method_id: header.method_id,
            client_id: header.client_id,
            session_id: header.session_id,
        }
    }
}

/// State for reassembling a single message.
#[derive(Debug)]
struct ReassemblyContext {
    /// Base SOME/IP header (from first segment, will be converted back to non-TP type).
    base_header: SomeIpHeader,
    /// Received segments indexed by offset.
    segments: BTreeMap<u32, bytes::Bytes>,
    /// Total payload length (known when last segment is received).
    total_length: Option<usize>,
    /// When this context was created.
    created_at: Instant,
}

impl ReassemblyContext {
    fn new(header: SomeIpHeader) -> Self {
        Self {
            base_header: header,
            segments: BTreeMap::new(),
            total_length: None,
            created_at: Instant::now(),
        }
    }

    /// Add a segment to this context.
    fn add_segment(&mut self, segment: &TpSegment) {
        let offset = segment.tp_header.offset;
        self.segments.insert(offset, segment.payload.clone());

        // If this is the last segment, calculate total length
        if !segment.tp_header.more {
            let last_offset_bytes = segment.tp_header.byte_offset();
            self.total_length = Some(last_offset_bytes + segment.payload.len());
        }
    }

    /// Check if reassembly is complete.
    fn is_complete(&self) -> bool {
        let total = match self.total_length {
            Some(len) => len,
            None => return false, // Haven't received last segment yet
        };

        // Check that we have contiguous segments from 0 to total
        let mut expected_offset: u32 = 0;
        let mut accumulated_bytes: usize = 0;

        for (&offset, payload) in &self.segments {
            // Check for gap
            if offset != expected_offset {
                return false;
            }

            accumulated_bytes += payload.len();
            expected_offset = (accumulated_bytes / 16) as u32;
        }

        accumulated_bytes >= total
    }

    /// Assemble the complete message.
    fn assemble(&self) -> Result<SomeIpMessage> {
        let total = self.total_length.ok_or_else(|| {
            SomeIpError::invalid_header("Cannot assemble: total length unknown")
        })?;

        let mut payload = BytesMut::with_capacity(total);

        for (_, segment_payload) in &self.segments {
            payload.put_slice(segment_payload);
        }

        // Create header with non-TP message type
        let mut header = self.base_header.clone();
        header.message_type = header.message_type.to_base();
        header.length = 8 + payload.len() as u32;

        Ok(SomeIpMessage::new(header, payload.freeze().to_vec()))
    }

    /// Check if this context has timed out.
    fn is_timed_out(&self, timeout: Duration) -> bool {
        self.created_at.elapsed() > timeout
    }
}

/// TP message reassembler.
///
/// Collects segments and reassembles them into complete messages.
#[derive(Debug)]
pub struct TpReassembler {
    /// Active reassembly contexts.
    contexts: HashMap<ReassemblyKey, ReassemblyContext>,
    /// Timeout for reassembly.
    timeout: Duration,
}

impl TpReassembler {
    /// Create a new reassembler with default timeout.
    pub fn new() -> Self {
        Self::with_timeout(DEFAULT_REASSEMBLY_TIMEOUT)
    }

    /// Create a new reassembler with custom timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            contexts: HashMap::new(),
            timeout,
        }
    }

    /// Feed a TP segment to the reassembler.
    ///
    /// Returns `Some(message)` if reassembly is complete, `None` if more segments are needed.
    pub fn feed(&mut self, segment: TpSegment) -> Result<Option<SomeIpMessage>> {
        let key = ReassemblyKey::from_header(&segment.header);

        // Get or create context
        let context = self.contexts.entry(key).or_insert_with(|| {
            ReassemblyContext::new(segment.header.clone())
        });

        // Add segment
        context.add_segment(&segment);

        // Check if complete
        if context.is_complete() {
            let message = context.assemble()?;
            self.contexts.remove(&key);
            return Ok(Some(message));
        }

        Ok(None)
    }

    /// Clean up timed-out reassembly contexts.
    ///
    /// Returns the number of contexts removed.
    pub fn cleanup(&mut self) -> usize {
        let timeout = self.timeout;
        let before = self.contexts.len();
        self.contexts.retain(|_, ctx| !ctx.is_timed_out(timeout));
        before - self.contexts.len()
    }

    /// Get the number of active reassembly contexts.
    pub fn active_contexts(&self) -> usize {
        self.contexts.len()
    }

    /// Clear all reassembly contexts.
    pub fn clear(&mut self) {
        self.contexts.clear();
    }
}

impl Default for TpReassembler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{MethodId, ServiceId};
    use crate::tp::segment::segment_message;

    #[test]
    fn test_reassembly_key() {
        let mut header = SomeIpHeader::default();
        header.service_id = ServiceId(0x1234);
        header.method_id = MethodId(0x0001);
        header.client_id = ClientId(0x0100);
        header.session_id = SessionId(0x0001);

        let key = ReassemblyKey::from_header(&header);

        assert_eq!(key.service_id, ServiceId(0x1234));
        assert_eq!(key.session_id, SessionId(0x0001));
    }

    #[test]
    fn test_reassemble_message() {
        // Create a large message
        let expected_payload: Vec<u8> = (0..3000u16).map(|i| (i % 256) as u8).collect();
        let msg = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload_vec(expected_payload.clone())
            .build();

        // Segment it
        let segments = segment_message(&msg, 1392);
        assert_eq!(segments.len(), 3);

        // Reassemble
        let mut reassembler = TpReassembler::new();

        // Feed first two segments - should return None
        assert!(reassembler.feed(segments[0].clone()).unwrap().is_none());
        assert!(reassembler.feed(segments[1].clone()).unwrap().is_none());
        assert_eq!(reassembler.active_contexts(), 1);

        // Feed last segment - should complete
        let result = reassembler.feed(segments[2].clone()).unwrap();
        assert!(result.is_some());

        let reassembled = result.unwrap();
        assert_eq!(reassembled.payload.as_ref(), expected_payload.as_slice());
        assert!(!reassembled.header.message_type.is_tp());
        assert_eq!(reassembler.active_contexts(), 0);
    }

    #[test]
    fn test_reassemble_out_of_order() {
        let expected_payload: Vec<u8> = (0..3000u16).map(|i| (i % 256) as u8).collect();
        let msg = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload_vec(expected_payload.clone())
            .build();

        let segments = segment_message(&msg, 1392);

        let mut reassembler = TpReassembler::new();

        // Feed in reverse order
        assert!(reassembler.feed(segments[2].clone()).unwrap().is_none());
        assert!(reassembler.feed(segments[0].clone()).unwrap().is_none());

        let result = reassembler.feed(segments[1].clone()).unwrap();
        assert!(result.is_some());

        let reassembled = result.unwrap();
        assert_eq!(reassembled.payload.as_ref(), expected_payload.as_slice());
    }

    #[test]
    fn test_multiple_concurrent_reassemblies() {
        let expected_payload1: Vec<u8> = vec![0xAAu8; 3000];
        let expected_payload2: Vec<u8> = vec![0xBBu8; 3000];

        let msg1 = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .client_id(ClientId(0x0001))
            .session_id(SessionId(0x0001))
            .payload_vec(expected_payload1.clone())
            .build();

        let msg2 = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .client_id(ClientId(0x0001))
            .session_id(SessionId(0x0002))
            .payload_vec(expected_payload2.clone())
            .build();

        let segments1 = segment_message(&msg1, 1392);
        let segments2 = segment_message(&msg2, 1392);

        let mut reassembler = TpReassembler::new();

        // Interleave segments from both messages
        reassembler.feed(segments1[0].clone()).unwrap();
        reassembler.feed(segments2[0].clone()).unwrap();
        assert_eq!(reassembler.active_contexts(), 2);

        reassembler.feed(segments1[1].clone()).unwrap();
        reassembler.feed(segments2[1].clone()).unwrap();

        let result1 = reassembler.feed(segments1[2].clone()).unwrap();
        assert!(result1.is_some());
        assert_eq!(result1.unwrap().payload.as_ref(), expected_payload1.as_slice());

        let result2 = reassembler.feed(segments2[2].clone()).unwrap();
        assert!(result2.is_some());
        assert_eq!(result2.unwrap().payload.as_ref(), expected_payload2.as_slice());

        assert_eq!(reassembler.active_contexts(), 0);
    }
}
