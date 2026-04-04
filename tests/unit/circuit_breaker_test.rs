//! Circuit Breaker 模块单元测试
//!
//! 测试三态熔断器的状态转换、原子操作和并发安全性。

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use oxcache::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};

    // ========================================================================
    // Basic Functionality Tests
    // ========================================================================

    #[test]
    fn test_circuit_breaker_creation() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(1),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_circuit_breaker_default_config() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.recovery_timeout, Duration::from_secs(30));
        assert_eq!(config.half_open_max_calls, 1);
    }

    #[test]
    fn test_circuit_breaker_debug() {
        let config = CircuitBreakerConfig::default();
        let cb = CircuitBreaker::new(config);
        let debug_str = format!("{:?}", cb);
        assert!(debug_str.contains("CircuitBreaker"));
        assert!(debug_str.contains("state"));
        assert!(debug_str.contains("failure_count"));
    }

    #[test]
    fn test_circuit_state_display() {
        assert_eq!(format!("{}", CircuitState::Closed), "Closed");
        assert_eq!(format!("{}", CircuitState::Open), "Open");
        assert_eq!(format!("{}", CircuitState::HalfOpen), "HalfOpen");
    }

    // ========================================================================
    // State Transition Tests: Closed -> Open
    // ========================================================================

    #[test]
    fn test_closed_to_open_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(1),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Initially closed, should allow execution
        assert!(cb.can_execute());
        assert_eq!(cb.state(), CircuitState::Closed);

        // Record failures up to threshold
        cb.record_failure();
        assert_eq!(cb.failure_count(), 1);
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.failure_count(), 3);
        assert_eq!(cb.state(), CircuitState::Open);

        // Now should not allow execution
        assert!(!cb.can_execute());
    }

    #[test]
    fn test_closed_reset_failure_count_on_success() {
        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(1),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Record some failures
        for _ in 0..3 {
            cb.record_failure();
        }
        assert_eq!(cb.failure_count(), 3);

        // Record success should reset failure count
        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    // ========================================================================
    // State Transition Tests: Open -> Half-Open -> Closed
    // ========================================================================

    #[test]
    fn test_open_to_half_open_after_timeout() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(100),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Transition to Open
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.can_execute());

        // Wait for recovery timeout
        thread::sleep(Duration::from_millis(150));

        // Should transition to Half-Open
        let can_exec = cb.can_execute();
        assert!(can_exec);
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_half_open_to_closed_on_success() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(100),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Transition to Open
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for recovery timeout
        thread::sleep(Duration::from_millis(150));

        // Transition to Half-Open
        cb.can_execute();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Record success, should transition back to Closed
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_half_open_to_open_on_failure() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(100),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Transition to Open
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for recovery timeout
        thread::sleep(Duration::from_millis(150));

        // Transition to Half-Open
        cb.can_execute();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Record failure, should transition back to Open
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    // ========================================================================
    // Half-Open Max Calls Tests
    // ========================================================================

    #[test]
    fn test_half_open_limits_calls() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(50),
            half_open_max_calls: 2,
        };
        let cb = CircuitBreaker::new(config);

        // Transition to Open
        cb.record_failure();
        cb.record_failure();

        // Wait for recovery timeout
        thread::sleep(Duration::from_millis(100));

        // First call triggers transition to Half-Open (doesn't increment half_open_call_count)
        let first = cb.can_execute();
        assert!(first, "First call should transition to Half-Open");
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Second call: half_open_call_count = 0 -> 1, 0 < 2 = true
        let second = cb.can_execute();
        assert!(second, "Second call should be allowed");

        // Third call: half_open_call_count = 1 -> 2, 1 < 2 = true
        let third = cb.can_execute();
        assert!(third, "Third call should be allowed");

        // Fourth call: half_open_call_count = 2 -> 3, 2 < 2 = false
        let fourth = cb.can_execute();
        assert!(!fourth, "Fourth call should be denied (exceeded half_open_max_calls)");
    }

    // ========================================================================
    // Reset and Manual Control Tests
    // ========================================================================

    #[test]
    fn test_manual_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_secs(100),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Transition to Open
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Manual reset
        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
        assert!(cb.can_execute());
    }

    #[test]
    fn test_reset_from_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(50),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Transition to Open
        cb.record_failure();
        cb.record_failure();

        // Wait for recovery timeout
        thread::sleep(Duration::from_millis(100));
        cb.can_execute(); // Triggers transition to Half-Open
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Manual reset from Half-Open
        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    // ========================================================================
    // Concurrent Access Tests
    // ========================================================================

    #[test]
    fn test_concurrent_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 10,
            recovery_timeout: Duration::from_secs(1),
            half_open_max_calls: 1,
        };
        let cb = Arc::new(CircuitBreaker::new(config));
        let mut handles = vec![];

        // Spawn 20 threads, each recording one failure
        for _ in 0..20 {
            let cb_clone = cb.clone();
            let handle = thread::spawn(move || {
                cb_clone.record_failure();
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Should be in Open state
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_concurrent_execute_checks() {
        let config = CircuitBreakerConfig {
            failure_threshold: 100,
            recovery_timeout: Duration::from_secs(1),
            half_open_max_calls: 10,
        };
        let cb = Arc::new(CircuitBreaker::new(config));
        let mut handles = vec![];

        // Spawn multiple threads checking can_execute
        for _ in 0..10 {
            let cb_clone = cb.clone();
            let handle = thread::spawn(move || {
                let mut allowed = 0;
                for _ in 0..20 {
                    if cb_clone.can_execute() {
                        allowed += 1;
                    }
                }
                allowed
            });
            handles.push(handle);
        }

        // All threads should succeed
        let total_allowed: u32 = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .sum();

        // All should be allowed since we're in Closed state
        assert_eq!(total_allowed, 200);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    // ========================================================================
    // Edge Cases Tests
    // ========================================================================

    #[test]
    fn test_failure_threshold_one() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout: Duration::from_secs(1),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Single failure should trigger Open
        assert!(cb.can_execute());
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.can_execute());
    }

    #[test]
    fn test_record_success_in_open_state() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_secs(100),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Transition to Open
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Record success in Open state (shouldn't normally happen, but handle it)
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_no_transition_in_open_state() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_secs(1),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Transition to Open
        cb.record_failure();
        cb.record_failure();
        let initial_count = cb.failure_count();

        // Further failures in Open state should not change anything
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();

        assert_eq!(cb.state(), CircuitState::Open);
        assert_eq!(cb.failure_count(), initial_count);
    }

    #[test]
    fn test_recovery_timeout_not_yet_elapsed() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_secs(10),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Transition to Open
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Try to execute immediately (timeout not elapsed)
        let can_exec = cb.can_execute();
        assert!(!can_exec);
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_config_accessor() {
        let config = CircuitBreakerConfig {
            failure_threshold: 42,
            recovery_timeout: Duration::from_secs(99),
            half_open_max_calls: 7,
        };
        let cb = CircuitBreaker::new(config);

        assert_eq!(cb.config().failure_threshold, 42);
        assert_eq!(cb.config().recovery_timeout, Duration::from_secs(99));
        assert_eq!(cb.config().half_open_max_calls, 7);
    }

    // ========================================================================
    // Full Lifecycle Test
    // ========================================================================

    #[test]
    fn test_full_lifecycle() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_millis(100),
            half_open_max_calls: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Phase 1: Closed - normal operation
        assert!(cb.can_execute());
        cb.record_success();
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);

        // Phase 2: Transition to Open after failures
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.can_execute());

        // Phase 3: Wait for recovery, transition to Half-Open
        thread::sleep(Duration::from_millis(150));
        assert!(cb.can_execute());
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Phase 4: Success in Half-Open, transition to Closed
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.can_execute());
    }
}
