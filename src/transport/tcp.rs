//! TCP transport for SOME/IP.

use std::io::{self, BufReader, BufWriter};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use crate::codec::{read_message, write_message};
use crate::error::{Result, SomeIpError};
use crate::header::{ClientId, SessionId};
use crate::message::SomeIpMessage;

/// Default TCP port for SOME/IP.
pub const DEFAULT_PORT: u16 = 30490;

/// A TCP connection wrapper with SOME/IP framing.
#[derive(Debug)]
pub struct TcpConnection {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
    peer_addr: SocketAddr,
}

impl TcpConnection {
    /// Create a new connection from a TcpStream.
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        let peer_addr = stream.peer_addr()?;
        let reader = BufReader::new(stream.try_clone()?);
        let writer = BufWriter::new(stream);
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

    /// Set read timeout.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.writer.get_ref().set_read_timeout(timeout)
    }

    /// Set write timeout.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.writer.get_ref().set_write_timeout(timeout)
    }

    /// Set TCP nodelay option.
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.writer.get_ref().set_nodelay(nodelay)
    }

    /// Read a SOME/IP message from the connection.
    pub fn read_message(&mut self) -> Result<SomeIpMessage> {
        read_message(&mut self.reader)
    }

    /// Write a SOME/IP message to the connection.
    pub fn write_message(&mut self, message: &SomeIpMessage) -> Result<()> {
        write_message(&mut self.writer, message)?;
        self.flush()?;
        Ok(())
    }

    /// Flush the write buffer.
    pub fn flush(&mut self) -> io::Result<()> {
        use std::io::Write;
        self.writer.flush()
    }

    /// Shutdown the connection.
    pub fn shutdown(&self) -> io::Result<()> {
        self.writer.get_ref().shutdown(std::net::Shutdown::Both)
    }
}

/// A SOME/IP TCP client.
///
/// Provides request/response functionality over TCP.
#[derive(Debug)]
pub struct TcpClient {
    connection: TcpConnection,
    client_id: ClientId,
    session_counter: AtomicU16,
}

impl TcpClient {
    /// Connect to a SOME/IP server.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        Self::from_stream(stream)
    }

    /// Connect to a SOME/IP server with a timeout.
    pub fn connect_timeout(addr: &SocketAddr, timeout: Duration) -> Result<Self> {
        let stream = TcpStream::connect_timeout(addr, timeout)?;
        Self::from_stream(stream)
    }

    /// Create a client from an existing TcpStream.
    pub fn from_stream(stream: TcpStream) -> Result<Self> {
        let connection = TcpConnection::new(stream)?;
        Ok(Self {
            connection,
            client_id: ClientId(0x0001), // Default client ID
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
        // Wrap around, skipping 0
        if id == 0 {
            self.session_counter.store(2, Ordering::Relaxed);
            SessionId(1)
        } else {
            SessionId(id)
        }
    }

    /// Set read timeout.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.connection.set_read_timeout(timeout)
    }

    /// Set write timeout.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.connection.set_write_timeout(timeout)
    }

    /// Send a request and wait for a response.
    ///
    /// This method assigns client ID and session ID to the message.
    pub fn call(&mut self, mut message: SomeIpMessage) -> Result<SomeIpMessage> {
        // Assign client and session IDs
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        let request_id = message.header.request_id();

        // Send request
        self.connection.write_message(&message)?;

        // Wait for response
        loop {
            let response = self.connection.read_message()?;

            // Check if this is the response we're waiting for
            if response.header.request_id() == request_id {
                return Ok(response);
            }

            // Store other responses (e.g., notifications) for later
            // In a real implementation, you might want a callback mechanism
        }
    }

    /// Send a fire-and-forget message (no response expected).
    pub fn send(&mut self, mut message: SomeIpMessage) -> Result<()> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();
        self.connection.write_message(&message)
    }

    /// Receive a message (e.g., notification).
    pub fn receive(&mut self) -> Result<SomeIpMessage> {
        self.connection.read_message()
    }

    /// Get a reference to the underlying connection.
    pub fn connection(&self) -> &TcpConnection {
        &self.connection
    }

    /// Get a mutable reference to the underlying connection.
    pub fn connection_mut(&mut self) -> &mut TcpConnection {
        &mut self.connection
    }

    /// Close the connection.
    pub fn close(self) -> io::Result<()> {
        self.connection.shutdown()
    }
}

/// A SOME/IP TCP server.
///
/// Accepts connections and handles incoming messages.
#[derive(Debug)]
pub struct TcpServer {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl TcpServer {
    /// Bind to an address and start listening.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let listener = TcpListener::bind(addr)?;
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
    pub fn accept(&self) -> Result<(TcpConnection, SocketAddr)> {
        let (stream, addr) = self.listener.accept()?;
        let connection = TcpConnection::new(stream)?;
        Ok((connection, addr))
    }

    /// Set non-blocking mode for the listener.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.listener.set_nonblocking(nonblocking)
    }

    /// Get an iterator over incoming connections.
    pub fn incoming(&self) -> impl Iterator<Item = Result<TcpConnection>> + '_ {
        self.listener.incoming().map(|result| {
            result
                .map_err(SomeIpError::from)
                .and_then(|stream| TcpConnection::new(stream).map_err(SomeIpError::from))
        })
    }
}

/// A simple request handler function type.
pub type RequestHandler = Box<dyn Fn(&SomeIpMessage) -> Option<SomeIpMessage> + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{MethodId, ServiceId};
    use std::thread;

    #[test]
    fn test_tcp_client_server() {
        // Start server
        let server = TcpServer::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr();

        // Spawn server thread
        let server_handle = thread::spawn(move || {
            let (mut conn, _) = server.accept().unwrap();

            // Read request
            let request = conn.read_message().unwrap();
            assert_eq!(request.header.service_id, ServiceId(0x1234));

            // Send response
            let response = request.create_response().payload(b"pong".as_slice()).build();
            conn.write_message(&response).unwrap();
        });

        // Connect client
        let mut client = TcpClient::connect(addr).unwrap();

        // Send request
        let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
            .payload(b"ping".as_slice())
            .build();

        let response = client.call(request).unwrap();
        assert_eq!(response.payload.as_ref(), b"pong");

        server_handle.join().unwrap();
    }

    #[test]
    fn test_session_id_increment() {
        let server = TcpServer::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr();

        thread::spawn(move || {
            let (mut conn, _) = server.accept().unwrap();
            for _ in 0..3 {
                let request = conn.read_message().unwrap();
                let response = request.create_response().build();
                conn.write_message(&response).unwrap();
            }
        });

        let mut client = TcpClient::connect(addr).unwrap();

        for expected_session in 1..=3 {
            let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001)).build();
            let response = client.call(request).unwrap();
            assert_eq!(response.header.session_id, SessionId(expected_session));
        }
    }
}
