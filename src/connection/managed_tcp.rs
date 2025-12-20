//! Managed TCP client with auto-reconnect.

use std::io;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicU16, Ordering};
use std::thread;

use crate::codec::{read_message, write_message};
use crate::error::Result;
use crate::header::{ClientId, SessionId};
use crate::message::SomeIpMessage;

use super::config::ConnectionConfig;
use super::state::{ConnectionState, ConnectionStats};

/// A managed TCP client with auto-reconnect capability.
///
/// This client wraps a TCP connection and provides:
/// - Automatic reconnection on connection loss
/// - Connection state tracking
/// - Statistics collection
pub struct ManagedTcpClient {
    /// Target address.
    addr: SocketAddr,
    /// Connection configuration.
    config: ConnectionConfig,
    /// Current connection state.
    state: ConnectionState,
    /// Active connection.
    stream: Option<TcpStream>,
    /// Client ID for messages.
    client_id: ClientId,
    /// Session counter.
    session_counter: AtomicU16,
    /// Connection statistics.
    stats: ConnectionStats,
    /// Current reconnection attempt count.
    reconnect_attempts: u32,
}

impl ManagedTcpClient {
    /// Create a new managed client for the given address.
    pub fn new<A: ToSocketAddrs>(addr: A, config: ConnectionConfig) -> io::Result<Self> {
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No address provided"))?;

        Ok(Self {
            addr,
            config,
            state: ConnectionState::Disconnected,
            stream: None,
            client_id: ClientId(0x0001),
            session_counter: AtomicU16::new(1),
            stats: ConnectionStats::default(),
            reconnect_attempts: 0,
        })
    }

    /// Create a managed client and immediately connect.
    pub fn connect<A: ToSocketAddrs>(addr: A, config: ConnectionConfig) -> Result<Self> {
        let mut client = Self::new(addr, config)?;
        client.ensure_connected()?;
        Ok(client)
    }

    /// Get the current connection state.
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Get connection statistics.
    pub fn stats(&self) -> &ConnectionStats {
        &self.stats
    }

    /// Set the client ID.
    pub fn set_client_id(&mut self, client_id: ClientId) {
        self.client_id = client_id;
    }

    /// Get the client ID.
    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    /// Get the target address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Check if the client is connected.
    pub fn is_connected(&self) -> bool {
        self.state.is_connected()
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

    /// Ensure the connection is established.
    fn ensure_connected(&mut self) -> Result<()> {
        if self.stream.is_some() && self.state == ConnectionState::Connected {
            return Ok(());
        }

        self.do_connect()
    }

    /// Perform the actual connection.
    fn do_connect(&mut self) -> Result<()> {
        self.state = ConnectionState::Connecting;

        match TcpStream::connect_timeout(&self.addr, self.config.connect_timeout) {
            Ok(stream) => {
                // Apply timeouts
                if let Some(timeout) = self.config.read_timeout {
                    let _ = stream.set_read_timeout(Some(timeout));
                }
                if let Some(timeout) = self.config.write_timeout {
                    let _ = stream.set_write_timeout(Some(timeout));
                }

                self.stream = Some(stream);
                self.state = ConnectionState::Connected;
                self.stats.record_connect();
                self.reconnect_attempts = 0;
                Ok(())
            }
            Err(e) => {
                self.state = ConnectionState::Disconnected;
                self.stats.record_failure();
                Err(e.into())
            }
        }
    }

    /// Attempt to reconnect.
    fn try_reconnect(&mut self) -> Result<()> {
        if !self.config.auto_reconnect {
            self.state = ConnectionState::Failed;
            return Err(crate::error::SomeIpError::Io(io::Error::new(
                io::ErrorKind::NotConnected,
                "Connection lost and auto-reconnect is disabled",
            )));
        }

        while self.config.retry_policy.should_retry(self.reconnect_attempts) {
            self.state = ConnectionState::Reconnecting;
            self.stats.record_reconnect();

            let delay = self.config.retry_policy.delay_for_attempt(self.reconnect_attempts);
            thread::sleep(delay);

            self.reconnect_attempts += 1;

            match self.do_connect() {
                Ok(()) => return Ok(()),
                Err(_) => continue,
            }
        }

        self.state = ConnectionState::Failed;
        Err(crate::error::SomeIpError::Io(io::Error::new(
            io::ErrorKind::NotConnected,
            "Failed to reconnect after maximum attempts",
        )))
    }

    /// Handle a connection error, potentially reconnecting.
    fn handle_error<T>(&mut self, err: crate::error::SomeIpError) -> Result<T> {
        self.stream = None;
        self.state = ConnectionState::Disconnected;
        self.stats.record_disconnect();

        match &err {
            crate::error::SomeIpError::Io(io_err) => {
                let should_retry = match io_err.kind() {
                    io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe => {
                        self.config.retry_policy.retry_on_connection_reset
                    }
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => {
                        self.config.retry_policy.retry_on_timeout
                    }
                    _ => false,
                };

                if should_retry && self.config.auto_reconnect {
                    self.try_reconnect()?;
                    // After reconnection, the caller should retry the operation
                    return Err(err);
                }
            }
            _ => {}
        }

        Err(err)
    }

    /// Send a request and wait for a response.
    pub fn call(&mut self, mut message: SomeIpMessage) -> Result<SomeIpMessage> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        self.ensure_connected()?;

        let request_id = message.header.request_id();

        // Send request
        let bytes = message.to_bytes();
        let stream = self.stream.as_mut().unwrap();

        if let Err(e) = write_message(stream, &message) {
            return self.handle_error(e);
        }

        self.stats.record_send(bytes.len());

        // Receive response
        loop {
            match read_message(stream) {
                Ok(response) => {
                    self.stats.record_receive(response.to_bytes().len());
                    if response.header.request_id() == request_id {
                        return Ok(response);
                    }
                }
                Err(e) => return self.handle_error(e),
            }
        }
    }

    /// Send a fire-and-forget message.
    pub fn send(&mut self, mut message: SomeIpMessage) -> Result<()> {
        message.header.client_id = self.client_id;
        message.header.session_id = self.next_session_id();

        self.ensure_connected()?;

        let bytes = message.to_bytes();
        let stream = self.stream.as_mut().unwrap();

        match write_message(stream, &message) {
            Ok(()) => {
                self.stats.record_send(bytes.len());
                Ok(())
            }
            Err(e) => self.handle_error(e),
        }
    }

    /// Receive a message.
    pub fn receive(&mut self) -> Result<SomeIpMessage> {
        self.ensure_connected()?;

        let stream = self.stream.as_mut().unwrap();

        match read_message(stream) {
            Ok(message) => {
                self.stats.record_receive(message.to_bytes().len());
                Ok(message)
            }
            Err(e) => self.handle_error(e),
        }
    }

    /// Disconnect the client.
    pub fn disconnect(&mut self) {
        if self.stream.is_some() {
            self.stream = None;
            self.state = ConnectionState::Disconnected;
            self.stats.record_disconnect();
        }
    }

    /// Force a reconnection.
    pub fn reconnect(&mut self) -> Result<()> {
        self.disconnect();
        self.reconnect_attempts = 0;
        self.ensure_connected()
    }
}

impl std::fmt::Debug for ManagedTcpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedTcpClient")
            .field("addr", &self.addr)
            .field("state", &self.state)
            .field("client_id", &self.client_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::config::RetryPolicy;

    #[test]
    fn test_managed_client_new() {
        let config = ConnectionConfig::simple();
        let client = ManagedTcpClient::new("127.0.0.1:30490", config).unwrap();

        assert_eq!(client.state(), ConnectionState::Disconnected);
        assert!(!client.is_connected());
    }

    #[test]
    fn test_managed_client_config() {
        let config = ConnectionConfig::default()
            .with_auto_reconnect(true)
            .with_retry_policy(RetryPolicy::fixed(3, std::time::Duration::from_millis(100)));

        let mut client = ManagedTcpClient::new("127.0.0.1:30490", config).unwrap();
        client.set_client_id(ClientId(0x1234));

        assert_eq!(client.client_id(), ClientId(0x1234));
    }
}
