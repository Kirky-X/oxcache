// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Lightweight circuit breaker for Redis backend.
//!
//! Three-state circuit breaker (Closed / Open / HalfOpen) that prevents
//! cascading failures when Redis is unreachable. Uses only atomic operations
//! — no mutexes, no external dependencies.

use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

/// Circuit breaker states.
const STATE_CLOSED: u8 = 0;
const STATE_OPEN: u8 = 1;
const STATE_HALF_OPEN: u8 = 2;

/// Lightweight circuit breaker using atomic operations.
///
/// # State Transitions
///
/// - **Closed → Open**: consecutive failures reach `threshold`
/// - **Open → HalfOpen**: `reset_timeout` has elapsed since last failure
/// - **HalfOpen → Closed**: one successful operation
/// - **HalfOpen → Open**: another failure
pub(crate) struct CircuitBreaker {
    state: AtomicU8,
    failure_count: AtomicU32,
    threshold: u32,
    reset_timeout_millis: u64,
    last_failure_millis: AtomicU64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    ///
    /// - `threshold`: consecutive failures before opening (must be ≥ 1)
    /// - `reset_timeout`: time to wait before transitioning Open → HalfOpen
    pub(crate) fn new(threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            state: AtomicU8::new(STATE_CLOSED),
            failure_count: AtomicU32::new(0),
            threshold: threshold.max(1),
            reset_timeout_millis: reset_timeout.as_millis() as u64,
            last_failure_millis: AtomicU64::new(0),
        }
    }

    /// Check if the circuit breaker is in Open state (rejecting requests).
    ///
    /// If the reset timeout has elapsed, transitions to HalfOpen and returns `false`.
    pub(crate) fn is_open(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        if state != STATE_OPEN {
            return false;
        }

        // Check if reset_timeout has elapsed → transition to HalfOpen
        let now = now_millis();
        let last_failure = self.last_failure_millis.load(Ordering::Relaxed);
        if now.saturating_sub(last_failure) >= self.reset_timeout_millis {
            // Attempt transition Open → HalfOpen
            if self
                .state
                .compare_exchange(STATE_OPEN, STATE_HALF_OPEN, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return false; // Now HalfOpen, allow request through
            }
            // Another thread already changed state; re-check
            return self.state.load(Ordering::Acquire) == STATE_OPEN;
        }

        true
    }

    /// Record a successful operation.
    ///
    /// - HalfOpen → Closed
    /// - Closed: reset failure count
    pub(crate) fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        let prev_state = self.state.load(Ordering::Acquire);
        if prev_state == STATE_HALF_OPEN {
            self.state.store(STATE_CLOSED, Ordering::Release);
        }
    }

    /// Record a failed operation.
    ///
    /// - Closed: increment failure count; if ≥ threshold → Open
    /// - HalfOpen → Open
    ///
    /// Returns `true` if the circuit breaker just transitioned to Open.
    pub(crate) fn record_failure(&self) -> bool {
        self.last_failure_millis.store(now_millis(), Ordering::Relaxed);

        let current_state = self.state.load(Ordering::Acquire);

        if current_state == STATE_HALF_OPEN {
            // HalfOpen → Open on any failure
            self.state.store(STATE_OPEN, Ordering::Release);
            self.failure_count.store(0, Ordering::Relaxed);
            return true;
        }

        if current_state == STATE_OPEN {
            // Already Open — no-op, do not increment counter
            return false;
        }

        // Closed: increment and check threshold
        let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= self.threshold {
            self.state.store(STATE_OPEN, Ordering::Release);
            return true;
        }

        false
    }

    /// Get the current state for diagnostics.
    #[cfg(test)]
    fn state(&self) -> u8 {
        self.state.load(Ordering::Relaxed)
    }

    /// Get the current failure count for diagnostics.
    #[cfg(test)]
    fn failures(&self) -> u32 {
        self.failure_count.load(Ordering::Relaxed)
    }
}

/// Current time in milliseconds since UNIX epoch.
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_closed() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(10));
        assert_eq!(cb.state(), STATE_CLOSED);
        assert!(!cb.is_open());
    }

    #[test]
    fn test_closed_to_open_after_threshold() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(10));

        assert!(!cb.record_failure()); // count=1
        assert!(!cb.record_failure()); // count=2
        assert!(cb.record_failure()); // count=3 → Open

        assert_eq!(cb.state(), STATE_OPEN);
        assert!(cb.is_open());
    }

    #[test]
    fn test_success_resets_failure_count() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(10));

        cb.record_failure(); // count=1
        cb.record_failure(); // count=2
        cb.record_success(); // reset to 0

        assert_eq!(cb.failures(), 0);
        assert_eq!(cb.state(), STATE_CLOSED);

        // Need 3 more failures to open
        assert!(!cb.record_failure());
        assert!(!cb.record_failure());
        assert!(cb.record_failure());
        assert!(cb.is_open());
    }

    #[test]
    fn test_open_to_half_open_after_timeout() {
        let cb = CircuitBreaker::new(1, Duration::from_millis(50));

        cb.record_failure(); // threshold=1 → Open immediately
        assert!(cb.is_open());

        // Wait for reset timeout
        std::thread::sleep(Duration::from_millis(60));

        // Should transition to HalfOpen
        assert!(!cb.is_open());
        assert_eq!(cb.state(), STATE_HALF_OPEN);
    }

    #[test]
    fn test_half_open_to_closed_on_success() {
        let cb = CircuitBreaker::new(1, Duration::from_millis(50));

        cb.record_failure(); // → Open
        std::thread::sleep(Duration::from_millis(60));
        assert!(!cb.is_open()); // → HalfOpen

        cb.record_success(); // HalfOpen → Closed
        assert_eq!(cb.state(), STATE_CLOSED);
        assert!(!cb.is_open());
    }

    #[test]
    fn test_half_open_to_open_on_failure() {
        let cb = CircuitBreaker::new(1, Duration::from_millis(50));

        cb.record_failure(); // → Open
        std::thread::sleep(Duration::from_millis(60));
        assert!(!cb.is_open()); // → HalfOpen

        assert!(cb.record_failure()); // HalfOpen → Open
        assert_eq!(cb.state(), STATE_OPEN);
        assert!(cb.is_open());
    }

    #[test]
    fn test_threshold_clamped_to_minimum_one() {
        // threshold=0 should be clamped to 1
        let cb = CircuitBreaker::new(0, Duration::from_secs(10));
        assert!(cb.record_failure()); // 1 failure → Open (threshold clamped to 1)
        assert!(cb.is_open());
    }

    #[test]
    fn test_concurrent_safety() {
        use std::sync::Arc;
        use std::thread;

        let cb = Arc::new(CircuitBreaker::new(100, Duration::from_secs(10)));
        let mut handles = Vec::new();

        for _ in 0..10 {
            let cb_clone = cb.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    cb_clone.record_failure();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // 100 failures with threshold=100 → should be Open
        assert!(cb.is_open());
    }

    #[test]
    fn test_record_failure_returns_true_only_on_transition() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(10));
        assert!(!cb.record_failure()); // count=1
        assert!(!cb.record_failure()); // count=2
        assert!(cb.record_failure()); // count=3 → Open, returns true
        // In Open state, record_failure is a no-op
        assert!(!cb.record_failure());
        assert!(!cb.record_failure());
    }

    #[test]
    fn test_half_open_success_then_failure_cycle() {
        // Verify full cycle: Closed → Open → HalfOpen → Closed → Open
        let cb = CircuitBreaker::new(2, Duration::from_millis(30));

        // Closed → Open
        cb.record_failure();
        assert!(cb.record_failure()); // → Open
        assert!(cb.is_open());

        // Wait for HalfOpen
        std::thread::sleep(Duration::from_millis(40));
        assert!(!cb.is_open()); // → HalfOpen

        // HalfOpen → Closed
        cb.record_success();
        assert_eq!(cb.state(), STATE_CLOSED);

        // Closed → Open again
        cb.record_failure();
        assert!(cb.record_failure()); // → Open again
        assert!(cb.is_open());
    }

    #[test]
    fn test_very_large_threshold() {
        let cb = CircuitBreaker::new(u32::MAX, Duration::from_secs(10));
        // Many failures should not open with huge threshold
        for _ in 0..1000 {
            assert!(!cb.record_failure());
        }
        assert!(!cb.is_open());
    }
}
