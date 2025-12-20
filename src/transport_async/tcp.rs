//! Async TCP transport for SOME/IP.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::time::timeout;

use crate::codec_async::{read_message_async, write_message_async};
use crate::error::{Result, SomeIpError};
use crate::header::{ClientId, SessionId};
use crate::message::SomeIpMessage;

/// Default TCP port for SOME/IP.
pub const DEFAULT_PORT: u16 = 30490;

/// An async TCP connection wrapper with SOME/IP framing.
pub struct AsyncTcpConnection {
    reader: BufReader<OwnedReadHalf>,
    writer: BufWriter<OwnedWriteHalf>,
    peer_addr: SocketAddr,
}

impl AsyncTcpConnection {
    /// Create a new connection from a TcpStream.
    pub fn new(stream: TcpStream) -> std::io::Result<Self> {
        let peer_addr = stream.peer_addr()?;
        let (read_half, write_half) = stream.into_split();
        let reader = BufReader::new(read_half);
        let writer = BufWriter::new(write_half);
        Ok(Self {
            reader,
            writer,
            peer_addr,
        })
    }

    /// Get the peer address.
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    /// Read a SOME/IP message from the connection.
    pub async fn read_message(&mut self) -> Result<SomeIpMessage> {
        read_message_async(&mut self.reader).await
    }

    /// Write a SOME/IP message to the connection.
    pub async fn write_message(&mut self, message: &SomeIpMessage) -> Result<()> {
        write_message_async(&mut self.writer, message).await?;
        self.flush().await?;
        Ok(())
    }

    /// Flush the write buffer.
    pub async fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush().await
    }

    /// Shutdown the connection.
    pub async fn shutdown(&mut self) -> std::io::Result<()> {
        self.writer.shutdown().await
    }
}

/// An async SOME/IP TCP client.
///
/// Provides request/response functionality over TCP.
pub struct AsyncTcpClient {
    connection: AsyncTcpConnection,
    client_id: ClientId,
    session_counter: AtomicU16,
}

impl AsyncTcpClient {
    /// Connect to a SOME/IP server.
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        Self::from_stream(stream)
    }

    /// Connect to a SOME/IP server with a timeout.
    pub async fn connect_timeout<A: ToSocketAddrs>(
        addr: A,
        duration: Duration,
    ) -> Result<Self> {
        let stream = timeout(duration, TcpStream::connect(addr))
            .await
            .map_err(|_| SomeIpError::Timeout)??;
        Self::from_stream(stream)
    }

    /// Create a client from an existing TcpStream.
    pub fn from_stream(stream: TcpStream) -> Result<Self> {
        let connection = AsyncTcpConnection::new(stream)?;
        Ok(Self {
            connection,
            client_id: ClientId(0x0001),
            session_counter: AtomicU16::new(1),
        })
    }

    /// Set the client ID.
    pub fn set_client_id(&mut self, client_id: ClientId) {
        self.client_id = client_id;
    }

    /// Get the client ID.
    pub fn client_id(&self) -> ClientId {
        self.client_id
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

    /// Send a request and wait for a response.
    ///
    /// This method assigns client ID and session ID to the message.
    pub async fn call(&mut self, mut message: SomeIpMessage) -> Result<SomeIpMessage> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        let request_id = message.header.request_id();

        // Send request
        self.connection.write_message(&message).await?;

        // Wait for response
        loop {
            let response = self.connection.read_message().await?;

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

    /// Send a fire-and-forget message (no response expected).
    pub async fn send(&mut self, mut message: SomeIpMessage) -> Result<()> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();
        self.connection.write_message(&message).await
    }

    /// Receive a message (e.g., notification).
    pub async fn receive(&mut self) -> Result<SomeIpMessage> {
        self.connection.read_message().await
    }

    /// Get a reference to the underlying connection.
    pub fn connection(&self) -> &AsyncTcpConnection {
        &self.connection
    }

    /// Get a mutable reference to the underlying connection.
    pub fn connection_mut(&mut self) -> &mut AsyncTcpConnection {
        &mut self.connection
    }

    /// Close the connection.
    pub async fn close(mut self) -> std::io::Result<()> {
        self.connection.shutdown().await
    }
}

/// An async SOME/IP TCP server.
///
/// Accepts connections and handles incoming messages.
pub struct AsyncTcpServer {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl AsyncTcpServer {
    /// Bind to an address and start listening.
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        Ok(Self {
            listener,
            local_addr,
        })
    }

    /// Get the local address the server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Accept a new connection.
    pub async fn accept(&self) -> Result<(AsyncTcpConnection, SocketAddr)> {
        let (stream, addr) = self.listener.accept().await?;
        let connection = AsyncTcpConnection::new(stream)?;
        Ok((connection, addr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{MethodId, ServiceId};

    #[tokio::test]
    async fn test_async_tcp_client_server() {
        // Start server
        let server = AsyncTcpServer::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr();

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let (mut conn, _) = server.accept().await.unwrap();

            // Read request
            let request = conn.read_message().await.unwrap();
            assert_eq!(request.header.service_id, ServiceId(0x1234));

            // Send response
            let response = request.create_response().payload(b"pong".as_slice()).build();
            conn.write_message(&response).await.unwrap();
        });

        // Connect client
        let mut client = AsyncTcpClient::connect(addr).await.unwrap();

        // Send request
        let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"ping".as_slice())
            .build();

        let response = client.call(request).await.unwrap();
        assert_eq!(response.payload.as_ref(), b"pong");

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_async_session_id_increment() {
        let server = AsyncTcpServer::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr();

        tokio::spawn(async move {
            let (mut conn, _) = server.accept().await.unwrap();
            for _ in 0..3 {
                let request = conn.read_message().await.unwrap();
                let response = request.create_response().build();
                conn.write_message(&response).await.unwrap();
            }
        });

        let mut client = AsyncTcpClient::connect(addr).await.unwrap();

        for expected_session in 1..=3u16 {
            let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001)).build();
            let response = client.call(request).await.unwrap();
            assert_eq!(response.header.session_id, SessionId(expected_session));
        }
    }
}
