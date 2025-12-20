//! SOME/IP-TP UDP client.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use crate::error::Result;
use crate::header::{ClientId, SessionId, HEADER_SIZE};
use crate::message::SomeIpMessage;

use super::header::TP_HEADER_SIZE;
use super::reassembly::TpReassembler;
use super::segment::{segment_message, TpSegment, DEFAULT_MAX_SEGMENT_PAYLOAD};

/// Maximum UDP datagram size for TP messages.
const MAX_DATAGRAM_SIZE: usize = 1500;

/// A SOME/IP-TP UDP client.
///
/// Automatically segments large messages and reassembles incoming segments.
#[derive(Debug)]
pub struct TpUdpClient {
    socket: UdpSocket,
    client_id: ClientId,
    session_counter: AtomicU16,
    recv_buffer: Vec<u8>,
    max_segment_payload: usize,
    reassembler: TpReassembler,
}

impl TpUdpClient {
    /// Create a new TP UDP client bound to any available port.
    pub fn new() -> Result<Self> {
        Self::bind("0.0.0.0:0")
    }

    /// Create a new TP UDP client bound to a specific address.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        Ok(Self {
            socket,
            client_id: ClientId(0x0001),
            session_counter: AtomicU16::new(1),
            recv_buffer: vec![0u8; MAX_DATAGRAM_SIZE],
            max_segment_payload: DEFAULT_MAX_SEGMENT_PAYLOAD,
            reassembler: TpReassembler::new(),
        })
    }

    /// Connect to a remote address.
    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> Result<()> {
        self.socket.connect(addr)?;
        Ok(())
    }

    /// Set the client ID.
    pub fn set_client_id(&mut self, client_id: ClientId) {
        self.client_id = client_id;
    }

    /// Get the client ID.
    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    /// Set the maximum segment payload size.
    pub fn set_max_segment_payload(&mut self, size: usize) {
        self.max_segment_payload = size;
    }

    /// Set the reassembly timeout.
    pub fn set_reassembly_timeout(&mut self, timeout: Duration) {
        self.reassembler = TpReassembler::with_timeout(timeout);
    }

    /// Get the next session ID.
    fn next_session_id(&self) -> SessionId {
        let id = self.session_counter.fetch_add(1, Ordering::Relaxed);
        if id == 0 {
            self.session_counter.store(2, Ordering::Relaxed);
            SessionId(1)
        } else {
            SessionId(id)
        }
    }

    /// Get the local address.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Set read timeout.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(timeout)
    }

    /// Set write timeout.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.socket.set_write_timeout(timeout)
    }

    /// Set non-blocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.socket.set_nonblocking(nonblocking)
    }

    /// Send a message, segmenting if necessary.
    fn send_message(&self, message: &SomeIpMessage) -> Result<()> {
        let segments = segment_message(message, self.max_segment_payload);

        if segments.is_empty() {
            // Small message, send directly
            let data = message.to_bytes();
            self.socket.send(&data)?;
        } else {
            // Large message, send as segments
            for segment in segments {
                let data = segment.to_bytes();
                self.socket.send(&data)?;
            }
        }

        Ok(())
    }

    /// Send a message to a specific address, segmenting if necessary.
    fn send_message_to<A: ToSocketAddrs>(&self, addr: A, message: &SomeIpMessage) -> Result<()> {
        let segments = segment_message(message, self.max_segment_payload);

        if segments.is_empty() {
            // Small message, send directly
            let data = message.to_bytes();
            self.socket.send_to(&data, &addr)?;
        } else {
            // Large message, send as segments
            for segment in segments {
                let data = segment.to_bytes();
                self.socket.send_to(&data, &addr)?;
            }
        }

        Ok(())
    }

    /// Receive a message, reassembling if necessary.
    ///
    /// Returns the complete message and the sender address.
    pub fn receive(&mut self) -> Result<(SomeIpMessage, SocketAddr)> {
        loop {
            let (len, addr) = self.socket.recv_from(&mut self.recv_buffer)?;
            let data = &self.recv_buffer[..len];

            // Check if this is a TP message
            if len >= HEADER_SIZE + TP_HEADER_SIZE {
                // Parse header to check message type
                let header = crate::header::SomeIpHeader::from_bytes(&data[..HEADER_SIZE])?;

                if header.message_type.is_tp() {
                    // Parse as TP segment
                    let segment = TpSegment::from_bytes(data)?;

                    // Feed to reassembler
                    if let Some(complete_message) = self.reassembler.feed(segment)? {
                        return Ok((complete_message, addr));
                    }
                    // Need more segments, continue receiving
                    continue;
                }
            }

            // Regular message
            let message = SomeIpMessage::from_bytes(data)?;
            return Ok((message, addr));
        }
    }

    /// Send a request to the connected address and wait for a response.
    pub fn call(&mut self, mut message: SomeIpMessage) -> Result<SomeIpMessage> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        let request_id = message.header.request_id();

        self.send_message(&message)?;

        // Wait for matching response
        loop {
            let (response, _) = self.receive()?;

            if response.header.request_id() == request_id {
                return Ok(response);
            }
        }
    }

    /// Send a request to a specific address and wait for a response.
    pub fn call_to<A: ToSocketAddrs>(
        &mut self,
        addr: A,
        mut message: SomeIpMessage,
    ) -> Result<SomeIpMessage> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        let request_id = message.header.request_id();

        self.send_message_to(addr, &message)?;

        // Wait for matching response
        loop {
            let (response, _) = self.receive()?;

            if response.header.request_id() == request_id {
                return Ok(response);
            }
        }
    }

    /// Send a fire-and-forget message to the connected address.
    pub fn send(&mut self, mut message: SomeIpMessage) -> Result<()> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        self.send_message(&message)
    }

    /// Send a fire-and-forget message to a specific address.
    pub fn send_to<A: ToSocketAddrs>(&mut self, addr: A, mut message: SomeIpMessage) -> Result<()> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        self.send_message_to(addr, &message)
    }

    /// Clean up timed-out reassembly contexts.
    ///
    /// Should be called periodically to free resources.
    pub fn cleanup(&mut self) -> usize {
        self.reassembler.cleanup()
    }

    /// Get the number of active reassembly contexts.
    pub fn active_reassemblies(&self) -> usize {
        self.reassembler.active_contexts()
    }

    /// Get a reference to the underlying socket.
    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tp_client_new() {
        let client = TpUdpClient::new().unwrap();
        assert!(client.local_addr().is_ok());
    }

    #[test]
    fn test_tp_client_settings() {
        let mut client = TpUdpClient::new().unwrap();

        client.set_client_id(ClientId(0x1234));
        assert_eq!(client.client_id(), ClientId(0x1234));

        client.set_max_segment_payload(1000);
        client.set_reassembly_timeout(Duration::from_secs(10));
    }
}
