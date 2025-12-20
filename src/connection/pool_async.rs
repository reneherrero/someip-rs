//! Async connection pooling for TCP clients.

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use tokio::net::ToSocketAddrs;
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::error::Result;
use crate::transport_async::AsyncTcpClient;

use super::config::PoolConfig;

/// Entry in the async connection pool.
struct AsyncPoolEntry {
    /// The client connection.
    client: AsyncTcpClient,
    /// When this connection was created.
    created_at: Instant,
    /// When this connection was last used.
    last_used: Instant,
}

impl AsyncPoolEntry {
    fn new(client: AsyncTcpClient) -> Self {
        let now = Instant::now();
        Self {
            client,
            created_at: now,
            last_used: now,
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

/// A pooled async TCP client that returns to the pool when dropped.
pub struct AsyncPooledTcpClient {
    /// The underlying client.
    client: Option<AsyncTcpClient>,
    /// Pool reference for returning the connection.
    pool: Arc<Mutex<AsyncPoolInner>>,
    /// Address of this connection.
    addr: SocketAddr,
}

impl AsyncPooledTcpClient {
    /// Get a reference to the underlying client.
    pub fn client(&self) -> &AsyncTcpClient {
        self.client.as_ref().unwrap()
    }

    /// Get a mutable reference to the underlying client.
    pub fn client_mut(&mut self) -> &mut AsyncTcpClient {
        self.client.as_mut().unwrap()
    }

    /// Send a request and wait for a response.
    pub async fn call(
        &mut self,
        message: crate::message::SomeIpMessage,
    ) -> Result<crate::message::SomeIpMessage> {
        self.client_mut().call(message).await
    }

    /// Send a fire-and-forget message.
    pub async fn send(&mut self, message: crate::message::SomeIpMessage) -> Result<()> {
        self.client_mut().send(message).await
    }

    /// Receive a message.
    pub async fn receive(&mut self) -> Result<crate::message::SomeIpMessage> {
        self.client_mut().receive().await
    }

    /// Return this connection to the pool without waiting for drop.
    pub async fn release(mut self) {
        if let Some(client) = self.client.take() {
            let mut pool = self.pool.lock().await;
            pool.return_connection(self.addr, client);
        }
    }
}

impl Drop for AsyncPooledTcpClient {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            let pool = self.pool.clone();
            let addr = self.addr;
            // Spawn a task to return the connection since we can't await in drop
            tokio::spawn(async move {
                let mut pool = pool.lock().await;
                pool.return_connection(addr, client);
            });
        }
    }
}

/// Inner pool state.
struct AsyncPoolInner {
    /// Configuration.
    config: PoolConfig,
    /// Connections by address.
    connections: HashMap<SocketAddr, Vec<AsyncPoolEntry>>,
}

impl AsyncPoolInner {
    fn new(config: PoolConfig) -> Self {
        Self {
            config,
            connections: HashMap::new(),
        }
    }

    /// Get an available connection for the given address.
    fn get_connection(&mut self, addr: SocketAddr) -> Option<AsyncTcpClient> {
        let entries = self.connections.entry(addr).or_default();

        // Clean up expired connections first
        entries.retain(|e| !e.is_expired(&self.config));

        // Find and remove an available entry
        if !entries.is_empty() {
            let entry = entries.remove(0);
            return Some(entry.client);
        }

        None
    }

    /// Return a connection to the pool.
    fn return_connection(&mut self, addr: SocketAddr, client: AsyncTcpClient) {
        let entries = self.connections.entry(addr).or_default();

        // Only add back if we're under the limit
        if entries.len() < self.config.max_connections_per_endpoint {
            entries.push(AsyncPoolEntry::new(client));
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

/// An async connection pool for TCP clients.
///
/// The pool manages connections to multiple endpoints and provides:
/// - Connection reuse
/// - Idle timeout
/// - Maximum lifetime
/// - Maximum connections per endpoint
#[derive(Clone)]
pub struct AsyncConnectionPool {
    inner: Arc<Mutex<AsyncPoolInner>>,
}

impl AsyncConnectionPool {
    /// Create a new async connection pool with the given configuration.
    pub fn new(config: PoolConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AsyncPoolInner::new(config))),
        }
    }

    /// Create a new async connection pool with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(PoolConfig::default())
    }

    /// Get a connection to the given address.
    ///
    /// Returns a pooled connection if available, otherwise creates a new one.
    pub async fn get<A: ToSocketAddrs>(&self, addr: A) -> Result<AsyncPooledTcpClient> {
        let addr = tokio::net::lookup_host(addr)
            .await
            .map_err(|e| crate::error::SomeIpError::Io(e))?
            .next()
            .ok_or_else(|| {
                crate::error::SomeIpError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "No address provided",
                ))
            })?;

        let mut pool = self.inner.lock().await;

        // Try to get an existing connection
        if let Some(client) = pool.get_connection(addr) {
            return Ok(AsyncPooledTcpClient {
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

        // Get timeout before releasing lock
        let connect_timeout = pool.config.connection_config.connect_timeout;
        drop(pool);

        // Create new connection
        let client = match timeout(connect_timeout, AsyncTcpClient::connect(addr)).await {
            Ok(Ok(client)) => client,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(crate::error::SomeIpError::Io(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Connection timeout",
                )))
            }
        };

        Ok(AsyncPooledTcpClient {
            client: Some(client),
            pool: self.inner.clone(),
            addr,
        })
    }

    /// Get the number of pooled connections for an address.
    pub async fn connection_count<A: ToSocketAddrs>(&self, addr: A) -> io::Result<usize> {
        let addr = tokio::net::lookup_host(addr).await?.next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "No address provided")
        })?;

        let pool = self.inner.lock().await;
        Ok(pool.connection_count(&addr))
    }

    /// Get total count of all pooled connections.
    pub async fn total_connections(&self) -> usize {
        let pool = self.inner.lock().await;
        pool.total_connections()
    }

    /// Clean up expired connections.
    ///
    /// Returns the number of connections removed.
    pub async fn cleanup(&self) -> usize {
        let mut pool = self.inner.lock().await;
        pool.cleanup()
    }

    /// Clear all pooled connections.
    pub async fn clear(&self) {
        let mut pool = self.inner.lock().await;
        pool.connections.clear();
    }
}

impl std::fmt::Debug for AsyncConnectionPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncConnectionPool").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_async_pool_new() {
        let pool = AsyncConnectionPool::with_defaults();
        assert_eq!(pool.total_connections().await, 0);
    }

    #[tokio::test]
    async fn test_async_pool_cleanup() {
        let config = PoolConfig::default().with_idle_timeout(Duration::from_millis(10));
        let pool = AsyncConnectionPool::new(config);

        // Nothing to cleanup initially
        assert_eq!(pool.cleanup().await, 0);
    }
}
