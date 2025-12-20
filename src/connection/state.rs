//! Connection state management.

use std::time::Instant;

/// Connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected.
    Disconnected,
    /// Currently attempting to connect.
    Connecting,
    /// Connected and ready.
    Connected,
    /// Connection is being reconnected after failure.
    Reconnecting,
    /// Connection has failed and is not being retried.
    Failed,
}

impl ConnectionState {
    /// Check if the connection is usable.
    pub fn is_connected(&self) -> bool {
        *self == ConnectionState::Connected
    }

    /// Check if a connection attempt is in progress.
    pub fn is_connecting(&self) -> bool {
        matches!(self, ConnectionState::Connecting | ConnectionState::Reconnecting)
    }

    /// Check if the connection has failed.
    pub fn is_failed(&self) -> bool {
        *self == ConnectionState::Failed
    }
}

impl Default for ConnectionState {
    fn default() -> Self {
        ConnectionState::Disconnected
    }
}

/// Connection statistics.
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// Number of successful connections.
    pub connect_count: u64,
    /// Number of connection failures.
    pub failure_count: u64,
    /// Number of reconnection attempts.
    pub reconnect_count: u64,
    /// Number of messages sent.
    pub messages_sent: u64,
    /// Number of messages received.
    pub messages_received: u64,
    /// Total bytes sent.
    pub bytes_sent: u64,
    /// Total bytes received.
    pub bytes_received: u64,
    /// Time of last successful connection.
    pub last_connected: Option<Instant>,
    /// Time of last disconnect.
    pub last_disconnected: Option<Instant>,
    /// Time of last error.
    pub last_error: Option<Instant>,
}

impl Default for ConnectionStats {
    fn default() -> Self {
        Self {
            connect_count: 0,
            failure_count: 0,
            reconnect_count: 0,
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            last_connected: None,
            last_disconnected: None,
            last_error: None,
        }
    }
}

impl ConnectionStats {
    /// Record a successful connection.
    pub fn record_connect(&mut self) {
        self.connect_count += 1;
        self.last_connected = Some(Instant::now());
    }

    /// Record a disconnection.
    pub fn record_disconnect(&mut self) {
        self.last_disconnected = Some(Instant::now());
    }

    /// Record a connection failure.
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_error = Some(Instant::now());
    }

    /// Record a reconnection attempt.
    pub fn record_reconnect(&mut self) {
        self.reconnect_count += 1;
    }

    /// Record a sent message.
    pub fn record_send(&mut self, bytes: usize) {
        self.messages_sent += 1;
        self.bytes_sent += bytes as u64;
    }

    /// Record a received message.
    pub fn record_receive(&mut self, bytes: usize) {
        self.messages_received += 1;
        self.bytes_received += bytes as u64;
    }

    /// Get uptime if connected.
    pub fn uptime(&self) -> Option<std::time::Duration> {
        self.last_connected.map(|t| t.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_state() {
        assert!(ConnectionState::Connected.is_connected());
        assert!(!ConnectionState::Disconnected.is_connected());
        assert!(ConnectionState::Connecting.is_connecting());
        assert!(ConnectionState::Reconnecting.is_connecting());
        assert!(ConnectionState::Failed.is_failed());
    }

    #[test]
    fn test_connection_stats() {
        let mut stats = ConnectionStats::default();

        stats.record_connect();
        assert_eq!(stats.connect_count, 1);
        assert!(stats.last_connected.is_some());

        stats.record_send(100);
        stats.record_receive(200);
        assert_eq!(stats.messages_sent, 1);
        assert_eq!(stats.bytes_sent, 100);
        assert_eq!(stats.messages_received, 1);
        assert_eq!(stats.bytes_received, 200);

        stats.record_failure();
        assert_eq!(stats.failure_count, 1);
        assert!(stats.last_error.is_some());
    }
}
