//! Async UDP transport for SOME/IP.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use tokio::net::{ToSocketAddrs, UdpSocket};
use tokio::time::timeout;

use crate::error::{Result, SomeIpError};
use crate::header::{ClientId, SessionId};
use crate::message::SomeIpMessage;
use crate::types::ReturnCode;

/// Default maximum UDP datagram size for SOME/IP.
pub const DEFAULT_MAX_DATAGRAM_SIZE: usize = 1400;

/// Default UDP port for SOME/IP.
pub const DEFAULT_PORT: u16 = 30490;

/// An async SOME/IP UDP client.
///
/// Provides request/response and fire-and-forget functionality over UDP.
pub struct AsyncUdpClient {
    socket: UdpSocket,
    client_id: ClientId,
    session_counter: AtomicU16,
    recv_buffer: Vec<u8>,
    connected_addr: Option<SocketAddr>,
}

impl AsyncUdpClient {
    /// Create a new UDP client bound to any available port.
    pub async fn new() -> Result<Self> {
        Self::bind("0.0.0.0:0").await
    }

    /// Create a new UDP client bound to a specific address.
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        Ok(Self {
            socket,
            client_id: ClientId(0x0001),
            session_counter: AtomicU16::new(1),
            recv_buffer: vec![0u8; DEFAULT_MAX_DATAGRAM_SIZE],
            connected_addr: None,
        })
    }

    /// Connect to a remote address.
    ///
    /// After connecting, `send` and `call` can be used without specifying the address.
    pub async fn connect<A: ToSocketAddrs>(&mut self, addr: A) -> Result<()> {
        self.socket.connect(addr).await?;
        self.connected_addr = self.socket.peer_addr().ok();
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
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Send a request to the connected address and wait for a response.
    pub async fn call(&mut self, mut message: SomeIpMessage) -> Result<SomeIpMessage> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        let request_id = message.header.request_id();
        let data = message.to_bytes();

        self.socket.send(&data).await?;

        // Wait for matching response
        loop {
            let len = self.socket.recv(&mut self.recv_buffer).await?;
            let response = SomeIpMessage::from_bytes(&self.recv_buffer[..len])?;

            if response.header.request_id() == request_id {
                return Ok(response);
            }
        }
    }

    /// Send a request with timeout.
    pub async fn call_timeout(
        &mut self,
        message: SomeIpMessage,
        duration: Duration,
    ) -> Result<SomeIpMessage> {
        timeout(duration, self.call(message))
            .await
            .map_err(|_| SomeIpError::Timeout)?
    }

    /// Send a request to a specific address and wait for a response.
    pub async fn call_to(
        &mut self,
        addr: SocketAddr,
        mut message: SomeIpMessage,
    ) -> Result<SomeIpMessage> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        let request_id = message.header.request_id();
        let data = message.to_bytes();

        self.socket.send_to(&data, addr).await?;

        // Wait for matching response
        loop {
            let (len, _) = self.socket.recv_from(&mut self.recv_buffer).await?;
            let response = SomeIpMessage::from_bytes(&self.recv_buffer[..len])?;

            if response.header.request_id() == request_id {
                return Ok(response);
            }
        }
    }

    /// Send a request to a specific address with timeout.
    pub async fn call_to_timeout(
        &mut self,
        addr: SocketAddr,
        message: SomeIpMessage,
        duration: Duration,
    ) -> Result<SomeIpMessage> {
        timeout(duration, self.call_to(addr, message))
            .await
            .map_err(|_| SomeIpError::Timeout)?
    }

    /// Send a fire-and-forget message to the connected address.
    pub async fn send(&mut self, mut message: SomeIpMessage) -> Result<()> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        let data = message.to_bytes();
        self.socket.send(&data).await?;
        Ok(())
    }

    /// Send a fire-and-forget message to a specific address.
    pub async fn send_to(&mut self, addr: SocketAddr, mut message: SomeIpMessage) -> Result<()> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        let data = message.to_bytes();
        self.socket.send_to(&data, addr).await?;
        Ok(())
    }

    /// Receive a message.
    pub async fn receive(&mut self) -> Result<(SomeIpMessage, SocketAddr)> {
        let (len, addr) = self.socket.recv_from(&mut self.recv_buffer).await?;
        let message = SomeIpMessage::from_bytes(&self.recv_buffer[..len])?;
        Ok((message, addr))
    }

    /// Receive a message with timeout.
    pub async fn receive_timeout(
        &mut self,
        duration: Duration,
    ) -> Result<(SomeIpMessage, SocketAddr)> {
        timeout(duration, self.receive())
            .await
            .map_err(|_| SomeIpError::Timeout)?
    }
}

/// An async SOME/IP UDP server.
///
/// Binds to an address and handles incoming messages.
pub struct AsyncUdpServer {
    socket: UdpSocket,
    recv_buffer: Vec<u8>,
    local_addr: SocketAddr,
}

impl AsyncUdpServer {
    /// Bind to an address.
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
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

    /// Receive a message.
    pub async fn receive(&mut self) -> Result<(SomeIpMessage, SocketAddr)> {
        let (len, addr) = self.socket.recv_from(&mut self.recv_buffer).await?;
        let message = SomeIpMessage::from_bytes(&self.recv_buffer[..len])?;
        Ok((message, addr))
    }

    /// Receive a message with timeout.
    pub async fn receive_timeout(
        &mut self,
        duration: Duration,
    ) -> Result<(SomeIpMessage, SocketAddr)> {
        timeout(duration, self.receive())
            .await
            .map_err(|_| SomeIpError::Timeout)?
    }

    /// Send a message to an address.
    pub async fn send_to(&self, message: &SomeIpMessage, addr: SocketAddr) -> Result<()> {
        let data = message.to_bytes();
        self.socket.send_to(&data, addr).await?;
        Ok(())
    }

    /// Send a response to a request.
    pub async fn respond(
        &self,
        request: &SomeIpMessage,
        payload: impl Into<bytes::Bytes>,
        addr: SocketAddr,
    ) -> Result<()> {
        let response = request.create_response().payload(payload).build();
        self.send_to(&response, addr).await
    }

    /// Send an error response to a request.
    pub async fn respond_error(
        &self,
        request: &SomeIpMessage,
        return_code: ReturnCode,
        addr: SocketAddr,
    ) -> Result<()> {
        let response = request.create_error_response(return_code).build();
        self.send_to(&response, addr).await
    }

    /// Join a multicast group.
    pub fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> std::io::Result<()> {
        self.socket.join_multicast_v4(*multiaddr, *interface)
    }

    /// Leave a multicast group.
    pub fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> std::io::Result<()> {
        self.socket.leave_multicast_v4(*multiaddr, *interface)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{MethodId, ServiceId};

    #[tokio::test]
    async fn test_async_udp_client_server() {
        // Start server
        let mut server = AsyncUdpServer::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server.local_addr();

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let (request, client_addr) = server.receive().await.unwrap();
            assert_eq!(request.header.service_id, ServiceId(0x1234));

            server
                .respond(&request, b"pong".as_slice(), client_addr)
                .await
                .unwrap();
        });

        // Create client and connect
        let mut client = AsyncUdpClient::new().await.unwrap();
        client.connect(server_addr).await.unwrap();

        // Send request
        let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"ping".as_slice())
            .build();

        let response = client.call(request).await.unwrap();
        assert_eq!(response.payload.as_ref(), b"pong");

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_async_udp_fire_and_forget() {
        let mut server = AsyncUdpServer::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server.local_addr();

        let server_handle = tokio::spawn(async move {
            let (request, _) = server.receive().await.unwrap();
            assert_eq!(request.header.service_id, ServiceId(0x5678));
            assert_eq!(request.payload.as_ref(), b"notification");
        });

        let mut client = AsyncUdpClient::new().await.unwrap();

        let msg = SomeIpMessage::notification(ServiceId(0x5678), MethodId(0x8001))
            .payload(b"notification".as_slice())
            .build();

        client.send_to(server_addr, msg).await.unwrap();

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_async_udp_call_to() {
        let mut server = AsyncUdpServer::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server.local_addr();

        let server_handle = tokio::spawn(async move {
            let (request, client_addr) = server.receive().await.unwrap();
            server
                .respond(&request, b"response".as_slice(), client_addr)
                .await
                .unwrap();
        });

        let mut client = AsyncUdpClient::new().await.unwrap();
        let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001)).build();

        let response = client.call_to(server_addr, request).await.unwrap();
        assert_eq!(response.payload.as_ref(), b"response");

        server_handle.await.unwrap();
    }
}
