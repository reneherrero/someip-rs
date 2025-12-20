//! Connection management for SOME/IP clients.
//!
//! This module provides:
//! - Auto-reconnecting TCP clients
//! - Connection pooling
//! - Configuration for retries, timeouts, and keep-alive
//!
//! # Example
//!
//! ```no_run
//! use someip_rs::connection::{ManagedTcpClient, ConnectionConfig, RetryPolicy};
//! use someip_rs::{SomeIpMessage, ServiceId, MethodId};
//! use std::time::Duration;
//!
//! // Create a managed client with auto-reconnect
//! let config = ConnectionConfig::default()
//!     .with_auto_reconnect(true)
//!     .with_retry_policy(RetryPolicy::fixed(3, Duration::from_secs(1)));
//!
//! let mut client = ManagedTcpClient::connect("127.0.0.1:30490", config).unwrap();
//!
//! // The client will automatically reconnect if the connection is lost
//! let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001)).build();
//! let response = client.call(request).unwrap();
//! ```
//!
//! # Connection Pooling
//!
//! ```no_run
//! use someip_rs::connection::{ConnectionPool, PoolConfig};
//! use someip_rs::{SomeIpMessage, ServiceId, MethodId};
//! use std::time::Duration;
//!
//! // Create a connection pool
//! let config = PoolConfig::default()
//!     .with_max_connections(10)
//!     .with_idle_timeout(Duration::from_secs(60));
//!
//! let pool = ConnectionPool::new(config);
//!
//! // Get a connection from the pool
//! let mut conn = pool.get("127.0.0.1:30490").unwrap();
//!
//! let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001)).build();
//! let response = conn.call(request).unwrap();
//!
//! // Connection is returned to the pool when dropped
//! drop(conn);
//! ```

mod config;
mod managed_tcp;
mod pool;
mod state;

pub use config::{BackoffStrategy, ConnectionConfig, KeepAliveConfig, PoolConfig, RetryPolicy};
pub use managed_tcp::ManagedTcpClient;
pub use pool::{ConnectionPool, PooledTcpClient};
pub use state::{ConnectionState, ConnectionStats};

// Async variants (require tokio feature)
#[cfg(feature = "tokio")]
mod managed_tcp_async;
#[cfg(feature = "tokio")]
mod pool_async;

#[cfg(feature = "tokio")]
pub use managed_tcp_async::AsyncManagedTcpClient;
#[cfg(feature = "tokio")]
pub use pool_async::{AsyncConnectionPool, AsyncPooledTcpClient};
