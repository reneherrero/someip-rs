//! Connection pooling for TCP clients.

use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::error::Result;
use crate::transport::TcpClient;

use super::config::PoolConfig;

/// Entry in the connection pool.
struct PoolEntry {
    /// The client connection.
    client: TcpClient,
    /// When this connection was created.
    created_at: Instant,
    /// When this connection was last used.
    last_used: Instant,
    /// Whether this connection is currently checked out.
    in_use: bool,
}

impl PoolEntry {
    fn new(client: TcpClient) -> Self {
        let now = Instant::now();
        Self {
            client,
            created_at: now,
            last_used: now,
            in_use: false,
        }
    }

    fn is_expired(&self, config: &PoolConfig) -> bool {
        // Check idle timeout
        if self.last_used.elapsed() > config.idle_timeout {
            return true;
        }

        // Check max lifetime
        if let Some(max_lifetime) = config.max_lifetime {
            if self.created_at.elapsed() > max_lifetime {
                return true;
            }
        }

        false
    }
}

/// A pooled TCP client that returns to the pool when dropped.
pub struct PooledTcpClient {
    /// The underlying client.
    client: Option<TcpClient>,
    /// Pool reference for returning the connection.
    pool: Arc<Mutex<PoolInner>>,
    /// Address of this connection.
    addr: SocketAddr,
}

impl PooledTcpClient {
    /// Get a reference to the underlying client.
    pub fn client(&self) -> &TcpClient {
        self.client.as_ref().unwrap()
    }

    /// Get a mutable reference to the underlying client.
    pub fn client_mut(&mut self) -> &mut TcpClient {
        self.client.as_mut().unwrap()
    }

    /// Send a request and wait for a response.
    pub fn call(
        &mut self,
        message: crate::message::SomeIpMessage,
    ) -> Result<crate::message::SomeIpMessage> {
        self.client_mut().call(message)
    }

    /// Send a fire-and-forget message.
    pub fn send(&mut self, message: crate::message::SomeIpMessage) -> Result<()> {
        self.client_mut().send(message)
    }

    /// Receive a message.
    pub fn receive(&mut self) -> Result<crate::message::SomeIpMessage> {
        self.client_mut().receive()
    }
}

impl Drop for PooledTcpClient {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            let mut pool = self.pool.lock().unwrap();
            pool.return_connection(self.addr, client);
        }
    }
}

impl std::ops::Deref for PooledTcpClient {
    type Target = TcpClient;

    fn deref(&self) -> &Self::Target {
        self.client.as_ref().unwrap()
    }
}

impl std::ops::DerefMut for PooledTcpClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.client.as_mut().unwrap()
    }
}

/// Inner pool state.
struct PoolInner {
    /// Configuration.
    config: PoolConfig,
    /// Connections by address.
    connections: HashMap<SocketAddr, Vec<PoolEntry>>,
}

impl PoolInner {
    fn new(config: PoolConfig) -> Self {
        Self {
            config,
            connections: HashMap::new(),
        }
    }

    /// Get an available connection for the given address.
    fn get_connection(&mut self, addr: SocketAddr) -> Option<TcpClient> {
        let entries = self.connections.entry(addr).or_default();

        // Clean up expired connections first
        entries.retain(|e| !e.in_use && !e.is_expired(&self.config));

        // Find an available connection
        for entry in entries.iter_mut() {
            if !entry.in_use {
                entry.in_use = true;
                entry.last_used = Instant::now();
                // We need to take ownership, so we'll swap with a placeholder
                // Actually, we need to remove and return
            }
        }

        // Find and remove an available entry
        if let Some(pos) = entries.iter().position(|e| !e.in_use) {
            let mut entry = entries.remove(pos);
            entry.in_use = true;
            entry.last_used = Instant::now();
            return Some(entry.client);
        }

        None
    }

    /// Return a connection to the pool.
    fn return_connection(&mut self, addr: SocketAddr, client: TcpClient) {
        let entries = self.connections.entry(addr).or_default();

        // Only add back if we're under the limit
        if entries.len() < self.config.max_connections_per_endpoint {
            entries.push(PoolEntry::new(client));
        }
        // Otherwise the connection is just dropped
    }

    /// Get the current count of connections for an address.
    fn connection_count(&self, addr: &SocketAddr) -> usize {
        self.connections.get(addr).map_or(0, |e| e.len())
    }

    /// Get total count of all pooled connections.
    fn total_connections(&self) -> usize {
        self.connections.values().map(|e| e.len()).sum()
    }

    /// Clean up expired connections across all endpoints.
    fn cleanup(&mut self) -> usize {
        let mut removed = 0;
        for entries in self.connections.values_mut() {
            let before = entries.len();
            entries.retain(|e| !e.is_expired(&self.config));
            removed += before - entries.len();
        }
        // Remove empty endpoint entries
        self.connections.retain(|_, v| !v.is_empty());
        removed
    }
}

/// A connection pool for TCP clients.
///
/// The pool manages connections to multiple endpoints and provides:
/// - Connection reuse
/// - Idle timeout
/// - Maximum lifetime
/// - Maximum connections per endpoint
#[derive(Clone)]
pub struct ConnectionPool {
    inner: Arc<Mutex<PoolInner>>,
}

impl ConnectionPool {
    /// Create a new connection pool with the given configuration.
    pub fn new(config: PoolConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PoolInner::new(config))),
        }
    }

    /// Create a new connection pool with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(PoolConfig::default())
    }

    /// Get a connection to the given address.
    ///
    /// Returns a pooled connection if available, otherwise creates a new one.
    pub fn get<A: ToSocketAddrs>(&self, addr: A) -> Result<PooledTcpClient> {
        let addr = addr
            .to_socket_addrs()
            .map_err(|e| crate::error::SomeIpError::Io(e))?
            .next()
            .ok_or_else(|| {
                crate::error::SomeIpError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "No address provided",
                ))
            })?;

        let mut pool = self.inner.lock().unwrap();

        // Try to get an existing connection
        if let Some(client) = pool.get_connection(addr) {
            return Ok(PooledTcpClient {
                client: Some(client),
                pool: self.inner.clone(),
                addr,
            });
        }

        // Check if we can create a new connection
        if pool.connection_count(&addr) >= pool.config.max_connections_per_endpoint {
            return Err(crate::error::SomeIpError::Io(io::Error::new(
                io::ErrorKind::Other,
                "Connection pool limit reached for endpoint",
            )));
        }

        // Release lock while connecting
        let connect_timeout = pool.config.connection_config.connect_timeout;
        let read_timeout = pool.config.connection_config.read_timeout;
        let write_timeout = pool.config.connection_config.write_timeout;
        drop(pool);

        // Create new connection
        let client = TcpClient::connect_timeout(&addr, connect_timeout)?;

        if let Some(timeout) = read_timeout {
            let _ = client.set_read_timeout(Some(timeout));
        }
        if let Some(timeout) = write_timeout {
            let _ = client.set_write_timeout(Some(timeout));
        }

        Ok(PooledTcpClient {
            client: Some(client),
            pool: self.inner.clone(),
            addr,
        })
    }

    /// Get the number of pooled connections for an address.
    pub fn connection_count<A: ToSocketAddrs>(&self, addr: A) -> io::Result<usize> {
        let addr = addr.to_socket_addrs()?.next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "No address provided")
        })?;

        let pool = self.inner.lock().unwrap();
        Ok(pool.connection_count(&addr))
    }

    /// Get total count of all pooled connections.
    pub fn total_connections(&self) -> usize {
        let pool = self.inner.lock().unwrap();
        pool.total_connections()
    }

    /// Clean up expired connections.
    ///
    /// Returns the number of connections removed.
    pub fn cleanup(&self) -> usize {
        let mut pool = self.inner.lock().unwrap();
        pool.cleanup()
    }

    /// Clear all pooled connections.
    pub fn clear(&self) {
        let mut pool = self.inner.lock().unwrap();
        pool.connections.clear();
    }
}

impl std::fmt::Debug for ConnectionPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pool = self.inner.lock().unwrap();
        f.debug_struct("ConnectionPool")
            .field("endpoints", &pool.connections.len())
            .field("total_connections", &pool.total_connections())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_pool_config() {
        let config = PoolConfig::default()
            .with_max_connections(5)
            .with_idle_timeout(Duration::from_secs(30));

        assert_eq!(config.max_connections_per_endpoint, 5);
        assert_eq!(config.idle_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_pool_new() {
        let pool = ConnectionPool::with_defaults();
        assert_eq!(pool.total_connections(), 0);
    }
}
