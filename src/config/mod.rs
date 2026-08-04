// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Configuration management module
//!
//! Provides structured configuration types for distributed cache parameters
//! including retry policies, circuit breaker thresholds, and health check intervals.

use std::time::Duration;

/// Distributed cache configuration.
///
/// Encapsulates all tunable parameters for distributed operations:
/// retry policy, circuit breaker, and health check scheduling.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::config::DistributedConfig;
/// use std::time::Duration;
///
/// let config = DistributedConfig::builder()
///     .retry_count(5)
///     .retry_base_delay(Duration::from_millis(200))
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct DistributedConfig {
    /// Maximum number of retry attempts for recoverable operations (default: 3).
    pub retry_count: u32,
    /// Base delay between retries; actual delay doubles each attempt (default: 100ms).
    pub retry_base_delay: Duration,
    /// Consecutive failures before the circuit breaker opens (default: 5).
    pub circuit_breaker_threshold: u32,
    /// Time to wait before transitioning Open → HalfOpen (default: 30s).
    pub circuit_breaker_reset_timeout: Duration,
    /// Interval between periodic health checks (default: 60s).
    pub health_check_interval: Duration,
}

impl Default for DistributedConfig {
    fn default() -> Self {
        Self {
            retry_count: 3,
            retry_base_delay: Duration::from_millis(100),
            circuit_breaker_threshold: 5,
            circuit_breaker_reset_timeout: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(60),
        }
    }
}

impl DistributedConfig {
    /// Create a builder for `DistributedConfig`.
    pub fn builder() -> DistributedConfigBuilder {
        DistributedConfigBuilder::default()
    }
}

/// Builder for `DistributedConfig` with chainable setters.
#[derive(Debug, Default)]
pub struct DistributedConfigBuilder {
    config: DistributedConfig,
}

impl DistributedConfigBuilder {
    /// Set the maximum retry count.
    pub fn retry_count(mut self, count: u32) -> Self {
        self.config.retry_count = count;
        self
    }

    /// Set the base retry delay.
    pub fn retry_base_delay(mut self, delay: Duration) -> Self {
        self.config.retry_base_delay = delay;
        self
    }

    /// Set the circuit breaker failure threshold.
    pub fn circuit_breaker_threshold(mut self, threshold: u32) -> Self {
        self.config.circuit_breaker_threshold = threshold;
        self
    }

    /// Set the circuit breaker reset timeout.
    pub fn circuit_breaker_reset_timeout(mut self, timeout: Duration) -> Self {
        self.config.circuit_breaker_reset_timeout = timeout;
        self
    }

    /// Set the health check interval.
    pub fn health_check_interval(mut self, interval: Duration) -> Self {
        self.config.health_check_interval = interval;
        self
    }

    /// Build the `DistributedConfig`.
    pub fn build(self) -> DistributedConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let config = DistributedConfig::default();
        assert_eq!(config.retry_count, 3);
        assert_eq!(config.retry_base_delay, Duration::from_millis(100));
        assert_eq!(config.circuit_breaker_threshold, 5);
        assert_eq!(config.circuit_breaker_reset_timeout, Duration::from_secs(30));
        assert_eq!(config.health_check_interval, Duration::from_secs(60));
    }

    #[test]
    fn test_builder_chain() {
        let config = DistributedConfig::builder()
            .retry_count(5)
            .retry_base_delay(Duration::from_millis(200))
            .circuit_breaker_threshold(10)
            .circuit_breaker_reset_timeout(Duration::from_secs(60))
            .health_check_interval(Duration::from_secs(120))
            .build();

        assert_eq!(config.retry_count, 5);
        assert_eq!(config.retry_base_delay, Duration::from_millis(200));
        assert_eq!(config.circuit_breaker_threshold, 10);
        assert_eq!(config.circuit_breaker_reset_timeout, Duration::from_secs(60));
        assert_eq!(config.health_check_interval, Duration::from_secs(120));
    }

    #[test]
    fn test_builder_partial_override() {
        let config = DistributedConfig::builder().retry_count(0).build();

        assert_eq!(config.retry_count, 0);
        // Other fields retain defaults
        assert_eq!(config.retry_base_delay, Duration::from_millis(100));
        assert_eq!(config.circuit_breaker_threshold, 5);
    }

    #[test]
    fn test_clone_and_debug() {
        let config = DistributedConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.retry_count, config.retry_count);

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("DistributedConfig"));
        assert!(debug_str.contains("retry_count"));
    }
}
