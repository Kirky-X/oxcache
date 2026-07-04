//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
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
        pub fn $name() -> bool {
            true
        }

        #[cfg(not(feature = $feature))]
        #[doc = $doc]
        pub fn $name() -> bool {
            false
        }
    };
}

// Generate individual feature availability functions
feature_check!("memory", l1_available, "Check if L1 cache is available");
feature_check!("redis", l2_available, "Check if L2 cache is available");
feature_check!("metrics", metrics_available, "Check if metrics are available");
feature_check!(
    "batch-write",
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
feature_check!("cli", cli_available, "Check if CLI is available");

// ============================================================================
// FeatureSet Structure
// ============================================================================

/// Unified feature availability check
#[derive(Debug, Clone)]
pub struct FeatureSet {
    /// L1 cache available
    pub(crate) l1_available: bool,
    /// L2 cache available
    pub(crate) l2_available: bool,
    /// Metrics available
    pub(crate) metrics_available: bool,
    /// CLI available
    pub(crate) cli_available: bool,
}

impl FeatureSet {
    /// Create feature set from current features
    pub fn current() -> Self {
        Self {
            l1_available: l1_available(),
            l2_available: l2_available(),
            metrics_available: metrics_available(),
            cli_available: cli_available(),
        }
    }

    /// Get tier name
    pub fn tier_name(&self) -> &'static str {
        if self.cli_available && self.metrics_available {
            "full"
        } else if self.l2_available && self.metrics_available {
            "core"
        } else if self.l1_available {
            "minimal"
        } else {
            "core"
        }
    }
}

impl Default for FeatureSet {
    fn default() -> Self {
        Self::current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "full")]
    #[test]
    fn test_feature_set_current() {
        let fs = FeatureSet::current();
        // With "full" feature, all should be available
        assert!(fs.l1_available);
        assert!(fs.l2_available);
        assert!(fs.metrics_available);
        assert!(fs.cli_available);
    }

    #[test]
    fn test_feature_set_default() {
        let fs = FeatureSet::default();
        // Default should match current
        let current = FeatureSet::current();
        assert_eq!(fs.l1_available, current.l1_available);
        assert_eq!(fs.l2_available, current.l2_available);
        assert_eq!(fs.metrics_available, current.metrics_available);
        assert_eq!(fs.cli_available, current.cli_available);
    }

    #[test]
    fn test_feature_set_tier_name_full() {
        let fs = FeatureSet {
            l1_available: true,
            l2_available: true,
            metrics_available: true,
            cli_available: true,
        };
        assert_eq!(fs.tier_name(), "full");
    }

    #[test]
    fn test_feature_set_tier_name_core_with_l2() {
        let fs = FeatureSet {
            l1_available: true,
            l2_available: true,
            metrics_available: true,
            cli_available: false,
        };
        assert_eq!(fs.tier_name(), "core");
    }

    #[test]
    fn test_feature_set_tier_name_minimal() {
        let fs = FeatureSet {
            l1_available: true,
            l2_available: false,
            metrics_available: false,
            cli_available: false,
        };
        assert_eq!(fs.tier_name(), "minimal");
    }

    #[test]
    fn test_feature_set_tier_name_core_fallback() {
        // No l1 available -> falls to "core"
        let fs = FeatureSet {
            l1_available: false,
            l2_available: false,
            metrics_available: false,
            cli_available: false,
        };
        assert_eq!(fs.tier_name(), "core");
    }

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

    #[cfg(feature = "cli")]
    #[test]
    fn test_cli_available() {
        // With "cli" feature, should be available
        assert!(cli_available());
    }

    #[cfg(feature = "batch-write")]
    #[test]
    fn test_batch_write_available() {
        // With "batch-write" feature
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
