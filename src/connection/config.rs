//! Connection management configuration types.

use std::time::Duration;

/// Backoff strategy for reconnection attempts.
#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    /// Fixed delay between attempts.
    Fixed(Duration),
    /// Exponential backoff with configurable parameters.
    Exponential {
        /// Initial delay.
        base: Duration,
        /// Maximum delay.
        max: Duration,
        /// Multiplier for each attempt.
        multiplier: f64,
    },
    /// Linear backoff with configurable parameters.
    Linear {
        /// Initial delay.
        initial: Duration,
        /// Increment per attempt.
        increment: Duration,
        /// Maximum delay.
        max: Duration,
    },
}

impl BackoffStrategy {
    /// Calculate the delay for a given attempt number (0-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        match self {
            BackoffStrategy::Fixed(d) => *d,
            BackoffStrategy::Exponential {
                base,
                max,
                multiplier,
            } => {
                let delay_ms = base.as_millis() as f64 * multiplier.powi(attempt as i32);
                Duration::from_millis(delay_ms.min(max.as_millis() as f64) as u64)
            }
            BackoffStrategy::Linear {
                initial,
                increment,
                max,
            } => {
                let delay = *initial + (*increment * attempt);
                delay.min(*max)
            }
        }
    }
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        BackoffStrategy::Exponential {
            base: Duration::from_millis(100),
            max: Duration::from_secs(30),
            multiplier: 2.0,
        }
    }
}

/// Retry policy for connection attempts.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts. `None` for unlimited.
    pub max_retries: Option<u32>,
    /// Backoff strategy between attempts.
    pub backoff: BackoffStrategy,
    /// Whether to retry on timeout errors.
    pub retry_on_timeout: bool,
    /// Whether to retry on connection reset errors.
    pub retry_on_connection_reset: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: Some(5),
            backoff: BackoffStrategy::default(),
            retry_on_timeout: true,
            retry_on_connection_reset: true,
        }
    }
}

impl RetryPolicy {
    /// Create a policy with no retries.
    pub fn no_retry() -> Self {
        Self {
            max_retries: Some(0),
            backoff: BackoffStrategy::Fixed(Duration::ZERO),
            retry_on_timeout: false,
            retry_on_connection_reset: false,
        }
    }

    /// Create a policy with unlimited retries.
    pub fn unlimited() -> Self {
        Self {
            max_retries: None,
            ..Default::default()
        }
    }

    /// Create a policy with fixed delay retries.
    pub fn fixed(max_retries: u32, delay: Duration) -> Self {
        Self {
            max_retries: Some(max_retries),
            backoff: BackoffStrategy::Fixed(delay),
            ..Default::default()
        }
    }

    /// Check if another retry attempt should be made.
    pub fn should_retry(&self, attempt: u32) -> bool {
        match self.max_retries {
            Some(max) => attempt < max,
            None => true,
        }
    }

    /// Get the delay for the next retry attempt.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        self.backoff.delay_for_attempt(attempt)
    }
}

/// Keep-alive configuration.
#[derive(Debug, Clone)]
pub struct KeepAliveConfig {
    /// Interval between keep-alive probes.
    pub interval: Duration,
    /// Timeout waiting for keep-alive response.
    pub timeout: Duration,
    /// Number of failed probes before considering connection dead.
    pub probes: u32,
}

impl Default for KeepAliveConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            probes: 3,
        }
    }
}

/// Connection configuration.
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Enable automatic reconnection.
    pub auto_reconnect: bool,
    /// Retry policy for reconnection attempts.
    pub retry_policy: RetryPolicy,
    /// Keep-alive configuration.
    pub keep_alive: Option<KeepAliveConfig>,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Read timeout.
    pub read_timeout: Option<Duration>,
    /// Write timeout.
    pub write_timeout: Option<Duration>,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            retry_policy: RetryPolicy::default(),
            keep_alive: Some(KeepAliveConfig::default()),
            connect_timeout: Duration::from_secs(5),
            read_timeout: Some(Duration::from_secs(30)),
            write_timeout: Some(Duration::from_secs(30)),
        }
    }
}

impl ConnectionConfig {
    /// Create a minimal configuration without auto-reconnect or keep-alive.
    pub fn simple() -> Self {
        Self {
            auto_reconnect: false,
            retry_policy: RetryPolicy::no_retry(),
            keep_alive: None,
            connect_timeout: Duration::from_secs(5),
            read_timeout: None,
            write_timeout: None,
        }
    }

    /// Create a robust configuration with auto-reconnect and keep-alive.
    pub fn robust() -> Self {
        Self::default()
    }

    /// Enable auto-reconnect.
    pub fn with_auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }

    /// Set the retry policy.
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Set the keep-alive configuration.
    pub fn with_keep_alive(mut self, config: KeepAliveConfig) -> Self {
        self.keep_alive = Some(config);
        self
    }

    /// Disable keep-alive.
    pub fn without_keep_alive(mut self) -> Self {
        self.keep_alive = None;
        self
    }

    /// Set the connection timeout.
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set the read timeout.
    pub fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = Some(timeout);
        self
    }

    /// Set the write timeout.
    pub fn with_write_timeout(mut self, timeout: Duration) -> Self {
        self.write_timeout = Some(timeout);
        self
    }
}

/// Connection pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum connections per endpoint.
    pub max_connections_per_endpoint: usize,
    /// Idle timeout before connection is closed.
    pub idle_timeout: Duration,
    /// Maximum lifetime of a connection.
    pub max_lifetime: Option<Duration>,
    /// Connection configuration for new connections.
    pub connection_config: ConnectionConfig,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_endpoint: 10,
            idle_timeout: Duration::from_secs(60),
            max_lifetime: Some(Duration::from_secs(3600)),
            connection_config: ConnectionConfig::simple(),
        }
    }
}

impl PoolConfig {
    /// Set the maximum connections per endpoint.
    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections_per_endpoint = max;
        self
    }

    /// Set the idle timeout.
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Set the maximum lifetime.
    pub fn with_max_lifetime(mut self, lifetime: Duration) -> Self {
        self.max_lifetime = Some(lifetime);
        self
    }

    /// Disable maximum lifetime.
    pub fn without_max_lifetime(mut self) -> Self {
        self.max_lifetime = None;
        self
    }

    /// Set the connection configuration.
    pub fn with_connection_config(mut self, config: ConnectionConfig) -> Self {
        self.connection_config = config;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_fixed() {
        let strategy = BackoffStrategy::Fixed(Duration::from_millis(100));
        assert_eq!(strategy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(strategy.delay_for_attempt(5), Duration::from_millis(100));
    }

    #[test]
    fn test_backoff_exponential() {
        let strategy = BackoffStrategy::Exponential {
            base: Duration::from_millis(100),
            max: Duration::from_secs(10),
            multiplier: 2.0,
        };
        assert_eq!(strategy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(strategy.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(strategy.delay_for_attempt(2), Duration::from_millis(400));
        assert_eq!(strategy.delay_for_attempt(10), Duration::from_secs(10)); // Capped at max
    }

    #[test]
    fn test_backoff_linear() {
        let strategy = BackoffStrategy::Linear {
            initial: Duration::from_millis(100),
            increment: Duration::from_millis(50),
            max: Duration::from_secs(1),
        };
        assert_eq!(strategy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(strategy.delay_for_attempt(1), Duration::from_millis(150));
        assert_eq!(strategy.delay_for_attempt(2), Duration::from_millis(200));
        assert_eq!(strategy.delay_for_attempt(100), Duration::from_secs(1)); // Capped at max
    }

    #[test]
    fn test_retry_policy_should_retry() {
        let policy = RetryPolicy::fixed(3, Duration::from_millis(100));
        assert!(policy.should_retry(0));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));

        let unlimited = RetryPolicy::unlimited();
        assert!(unlimited.should_retry(1000));
    }

    #[test]
    fn test_connection_config_builder() {
        let config = ConnectionConfig::simple()
            .with_auto_reconnect(true)
            .with_connect_timeout(Duration::from_secs(10));

        assert!(config.auto_reconnect);
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
    }
}
