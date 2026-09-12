// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Feature flags and conditional compilation utilities for oxcache.
//!
//! This module provides a centralized way to check which features are enabled
//! at compile time, enabling zero-cost abstractions for feature detection.

// ============================================================================
// Feature Flag Macro
// ============================================================================

/// Macro to generate feature availability functions
/// Reduces code duplication for feature check functions
macro_rules! feature_check {
    ($feature:literal, $name:ident, $doc:expr) => {
        #[cfg(feature = $feature)]
        #[doc = $doc]
        #[allow(dead_code)]
        pub fn $name() -> bool {
            true
        }

        #[cfg(not(feature = $feature))]
        #[doc = $doc]
        #[allow(dead_code)]
        pub fn $name() -> bool {
            false
        }
    };
}

// Generate individual feature availability functions
feature_check!("memory", l1_available, "Check if L1 cache is available");
feature_check!("redis", l2_available, "Check if L2 cache is available");
feature_check!(
    "metrics",
    metrics_available,
    "Check if metrics are available"
);
feature_check!(
    "batch",
    batch_write_available,
    "Check if batch write is available"
);
feature_check!(
    "serialization",
    serialization_available,
    "Check if serialization is available"
);
feature_check!(
    "compression",
    compression_available,
    "Check if compression is available"
);
feature_check!(
    "lock",
    dist_lock_available,
    "Check if distributed lock is available"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l1_available() {
        // With "memory" feature, l1 should be available
        assert!(l1_available());
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_l2_available() {
        // With "redis" feature, l2 should be available
        assert!(l2_available());
    }

    #[test]
    fn test_metrics_available() {
        // With "metrics" feature, should be available
        assert!(metrics_available());
    }

    #[cfg(feature = "batch")]
    #[test]
    fn test_batch_write_available() {
        // With "batch" feature
        assert!(batch_write_available());
    }

    #[test]
    fn test_serialization_available() {
        // With "serialization" feature
        assert!(serialization_available());
    }

    #[test]
    fn test_compression_available() {
        // compression may or may not be available depending on features
        let _ = compression_available();
    }
}
