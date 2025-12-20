//! SOME/IP-TP UDP server.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

use crate::error::Result;
use crate::header::HEADER_SIZE;
use crate::message::SomeIpMessage;
use crate::types::ReturnCode;

use super::header::TP_HEADER_SIZE;
use super::reassembly::TpReassembler;
use super::segment::{segment_message, TpSegment, DEFAULT_MAX_SEGMENT_PAYLOAD};

/// Maximum UDP datagram size for TP messages.
const MAX_DATAGRAM_SIZE: usize = 1500;

/// A SOME/IP-TP UDP server.
///
/// Automatically reassembles incoming segments and segments large outgoing messages.
#[derive(Debug)]
pub struct TpUdpServer {
    socket: UdpSocket,
    recv_buffer: Vec<u8>,
    local_addr: SocketAddr,
    max_segment_payload: usize,
    reassembler: TpReassembler,
}

impl TpUdpServer {
    /// Bind to an address.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        let local_addr = socket.local_addr()?;
        Ok(Self {
            socket,
            recv_buffer: vec![0u8; MAX_DATAGRAM_SIZE],
            local_addr,
            max_segment_payload: DEFAULT_MAX_SEGMENT_PAYLOAD,
            reassembler: TpReassembler::new(),
        })
    }

    /// Get the local address.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Set the maximum segment payload size.
    pub fn set_max_segment_payload(&mut self, size: usize) {
        self.max_segment_payload = size;
    }

    /// Set the reassembly timeout.
    pub fn set_reassembly_timeout(&mut self, timeout: Duration) {
        self.reassembler = TpReassembler::with_timeout(timeout);
    }

    /// Set read timeout.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(timeout)
    }

    /// Set non-blocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.socket.set_nonblocking(nonblocking)
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

    /// Send a message to an address, segmenting if necessary.
    pub fn send_to(&self, message: &SomeIpMessage, addr: SocketAddr) -> Result<()> {
        let segments = segment_message(message, self.max_segment_payload);

        if segments.is_empty() {
            // Small message, send directly
            let data = message.to_bytes();
            self.socket.send_to(&data, addr)?;
        } else {
            // Large message, send as segments
            for segment in segments {
                let data = segment.to_bytes();
                self.socket.send_to(&data, addr)?;
            }
        }

        Ok(())
    }

    /// Send a response to a request.
    ///
    /// Creates a response message from the request and sends it.
    /// The response is automatically segmented if necessary.
    pub fn respond(
        &self,
        request: &SomeIpMessage,
        payload: impl Into<bytes::Bytes>,
        addr: SocketAddr,
    ) -> Result<()> {
        let response = request.create_response().payload(payload).build();
        self.send_to(&response, addr)
    }

    /// Send an error response to a request.
    pub fn respond_error(
        &self,
        request: &SomeIpMessage,
        return_code: ReturnCode,
        addr: SocketAddr,
    ) -> Result<()> {
        let response = request.create_error_response(return_code).build();
        self.send_to(&response, addr)
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

    /// Join a multicast group.
    pub fn join_multicast_v4(
        &self,
        multiaddr: &std::net::Ipv4Addr,
        interface: &std::net::Ipv4Addr,
    ) -> io::Result<()> {
        self.socket.join_multicast_v4(multiaddr, interface)
    }

    /// Leave a multicast group.
    pub fn leave_multicast_v4(
        &self,
        multiaddr: &std::net::Ipv4Addr,
        interface: &std::net::Ipv4Addr,
    ) -> io::Result<()> {
        self.socket.leave_multicast_v4(multiaddr, interface)
    }

    /// Get a reference to the underlying socket.
    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{MethodId, ServiceId};
    use std::thread;

    #[test]
    fn test_tp_server_bind() {
        let server = TpUdpServer::bind("127.0.0.1:0").unwrap();
        assert!(server.local_addr().port() > 0);
    }

    #[test]
    fn test_tp_client_server_small_message() {
        use super::super::client::TpUdpClient;

        let mut server = TpUdpServer::bind("127.0.0.1:0").unwrap();
        let server_addr = server.local_addr();

        let server_handle = thread::spawn(move || {
            let (request, client_addr) = server.receive().unwrap();
            assert_eq!(request.header.service_id, ServiceId(0x1234));
            assert_eq!(request.payload.as_ref(), b"ping");

            server
                .respond(&request, b"pong".as_slice(), client_addr)
                .unwrap();
        });

        let mut client = TpUdpClient::new().unwrap();
        client.connect(server_addr).unwrap();

        let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"ping".as_slice())
            .build();

        let response = client.call(request).unwrap();
        assert_eq!(response.payload.as_ref(), b"pong");

        server_handle.join().unwrap();
    }

    #[test]
    fn test_tp_client_server_large_message() {
        use super::super::client::TpUdpClient;
        use bytes::Bytes;

        let mut server = TpUdpServer::bind("127.0.0.1:0").unwrap();
        let server_addr = server.local_addr();

        // Create a large payload that requires segmentation
        let large_payload: Vec<u8> = (0..5000u16).map(|i| (i % 256) as u8).collect();
        let expected_payload = large_payload.clone();

        let server_handle = thread::spawn(move || {
            let (request, client_addr) = server.receive().unwrap();
            assert_eq!(request.header.service_id, ServiceId(0x1234));
            assert_eq!(request.payload.as_ref(), expected_payload.as_slice());

            // Send a large response
            let response_payload: Vec<u8> = (0..4000u16).map(|i| ((i + 1) % 256) as u8).collect();
            server
                .respond(&request, Bytes::from(response_payload), client_addr)
                .unwrap();
        });

        let mut client = TpUdpClient::new().unwrap();
        client.connect(server_addr).unwrap();

        let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload_vec(large_payload)
            .build();

        let response = client.call(request).unwrap();

        // Verify response payload
        let expected_response: Vec<u8> = (0..4000u16).map(|i| ((i + 1) % 256) as u8).collect();
        assert_eq!(response.payload.as_ref(), expected_response.as_slice());

        server_handle.join().unwrap();
    }
}
