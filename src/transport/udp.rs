//! UDP transport for SOME/IP.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use crate::error::Result;
use crate::header::{ClientId, SessionId};
use crate::message::SomeIpMessage;

/// Default maximum UDP datagram size for SOME/IP.
pub const DEFAULT_MAX_DATAGRAM_SIZE: usize = 1400;

/// Default UDP port for SOME/IP.
pub const DEFAULT_PORT: u16 = 30490;

/// A SOME/IP UDP client.
///
/// Provides request/response and fire-and-forget functionality over UDP.
#[derive(Debug)]
pub struct UdpClient {
    socket: UdpSocket,
    client_id: ClientId,
    session_counter: AtomicU16,
    recv_buffer: Vec<u8>,
    max_datagram_size: usize,
}

impl UdpClient {
    /// Create a new UDP client bound to any available port.
    pub fn new() -> Result<Self> {
        Self::bind("0.0.0.0:0")
    }

    /// Create a new UDP client bound to a specific address.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        Ok(Self {
            socket,
            client_id: ClientId(0x0001),
            session_counter: AtomicU16::new(1),
            recv_buffer: vec![0u8; DEFAULT_MAX_DATAGRAM_SIZE],
            max_datagram_size: DEFAULT_MAX_DATAGRAM_SIZE,
        })
    }

    /// Connect to a remote address.
    ///
    /// After connecting, `send` and `receive` can be used without specifying the address.
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

    /// Set the maximum datagram size.
    pub fn set_max_datagram_size(&mut self, size: usize) {
        self.max_datagram_size = size;
        self.recv_buffer.resize(size, 0);
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

    /// Send a request to the connected address and wait for a response.
    pub fn call(&mut self, mut message: SomeIpMessage) -> Result<SomeIpMessage> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        let request_id = message.header.request_id();
        let data = message.to_bytes();

        self.socket.send(&data)?;

        // Wait for matching response
        loop {
            let (len, _) = self.socket.recv_from(&mut self.recv_buffer)?;
            let response = SomeIpMessage::from_bytes(&self.recv_buffer[..len])?;

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
        let data = message.to_bytes();

        self.socket.send_to(&data, addr)?;

        // Wait for matching response
        loop {
            let (len, _) = self.socket.recv_from(&mut self.recv_buffer)?;
            let response = SomeIpMessage::from_bytes(&self.recv_buffer[..len])?;

            if response.header.request_id() == request_id {
                return Ok(response);
            }
        }
    }

    /// Send a fire-and-forget message to the connected address.
    pub fn send(&mut self, mut message: SomeIpMessage) -> Result<()> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        let data = message.to_bytes();
        self.socket.send(&data)?;
        Ok(())
    }

    /// Send a fire-and-forget message to a specific address.
    pub fn send_to<A: ToSocketAddrs>(&mut self, addr: A, mut message: SomeIpMessage) -> Result<()> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        let data = message.to_bytes();
        self.socket.send_to(&data, addr)?;
        Ok(())
    }

    /// Receive a message.
    pub fn receive(&mut self) -> Result<(SomeIpMessage, SocketAddr)> {
        let (len, addr) = self.socket.recv_from(&mut self.recv_buffer)?;
        let message = SomeIpMessage::from_bytes(&self.recv_buffer[..len])?;
        Ok((message, addr))
    }

    /// Get a reference to the underlying socket.
    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }
}

/// A SOME/IP UDP server.
///
/// Binds to an address and handles incoming messages.
#[derive(Debug)]
pub struct UdpServer {
    socket: UdpSocket,
    recv_buffer: Vec<u8>,
    local_addr: SocketAddr,
}

impl UdpServer {
    /// Bind to an address.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        let local_addr = socket.local_addr()?;
        Ok(Self {
            socket,
            recv_buffer: vec![0u8; DEFAULT_MAX_DATAGRAM_SIZE],
            local_addr,
        })
    }

    /// Get the local address.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Set read timeout.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(timeout)
    }

    /// Set non-blocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.socket.set_nonblocking(nonblocking)
    }

    /// Receive a message.
    pub fn receive(&mut self) -> Result<(SomeIpMessage, SocketAddr)> {
        let (len, addr) = self.socket.recv_from(&mut self.recv_buffer)?;
        let message = SomeIpMessage::from_bytes(&self.recv_buffer[..len])?;
        Ok((message, addr))
    }

    /// Send a message to an address.
    pub fn send_to(&self, message: &SomeIpMessage, addr: SocketAddr) -> Result<()> {
        let data = message.to_bytes();
        self.socket.send_to(&data, addr)?;
        Ok(())
    }

    /// Send a response to a request.
    ///
    /// Creates a response message from the request and sends it.
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
        return_code: crate::types::ReturnCode,
        addr: SocketAddr,
    ) -> Result<()> {
        let response = request.create_error_response(return_code).build();
        self.send_to(&response, addr)
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
    fn test_udp_client_server() {
        // Start server
        let mut server = UdpServer::bind("127.0.0.1:0").unwrap();
        let server_addr = server.local_addr();

        // Spawn server thread
        let server_handle = thread::spawn(move || {
            let (request, client_addr) = server.receive().unwrap();
            assert_eq!(request.header.service_id, ServiceId(0x1234));

            server
                .respond(&request, b"pong".as_slice(), client_addr)
                .unwrap();
        });

        // Create client and connect
        let mut client = UdpClient::new().unwrap();
        client.connect(server_addr).unwrap();

        // Send request
        let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"ping".as_slice())
            .build();

        let response = client.call(request).unwrap();
        assert_eq!(response.payload.as_ref(), b"pong");

        server_handle.join().unwrap();
    }

    #[test]
    fn test_udp_fire_and_forget() {
        let mut server = UdpServer::bind("127.0.0.1:0").unwrap();
        let server_addr = server.local_addr();

        let server_handle = thread::spawn(move || {
            let (request, _) = server.receive().unwrap();
            assert_eq!(request.header.service_id, ServiceId(0x5678));
            assert_eq!(request.payload.as_ref(), b"notification");
        });

        let mut client = UdpClient::new().unwrap();

        let msg = SomeIpMessage::notification(ServiceId(0x5678), MethodId(0x8001))
            .payload(b"notification".as_slice())
            .build();

        client.send_to(server_addr, msg).unwrap();

        server_handle.join().unwrap();
    }

    #[test]
    fn test_udp_call_to() {
        let mut server = UdpServer::bind("127.0.0.1:0").unwrap();
        let server_addr = server.local_addr();

        let server_handle = thread::spawn(move || {
            let (request, client_addr) = server.receive().unwrap();
            server
                .respond(&request, b"response".as_slice(), client_addr)
                .unwrap();
        });

        let mut client = UdpClient::new().unwrap();
        let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001)).build();

        let response = client.call_to(server_addr, request).unwrap();
        assert_eq!(response.payload.as_ref(), b"response");

        server_handle.join().unwrap();
    }
}
