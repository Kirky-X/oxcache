//! Feature flags and conditional compilation utilities for oxcache.
//!
//! This module provides a centralized way to check which features are enabled
//! at compile time, enabling zero-cost abstractions for feature detection.

use paste::paste;

// ============================================================================
// Feature Flag Macro
// ============================================================================

/// Macro to generate feature availability functions
/// Reduces code duplication for feature check functions
macro_rules! feature_check {
    ($feature:literal, $name:ident, $doc:expr) => {
        paste! {
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
        }
    };
}

// Generate individual feature availability functions
feature_check!("l1-moka", l1_available, "Check if L1 cache is available");
feature_check!("l2-redis", l2_available, "Check if L2 cache is available");
feature_check!(
    "metrics",
    metrics_available,
    "Check if metrics are available"
);
feature_check!(
    "bloom-filter",
    bloom_available,
    "Check if bloom filter is available"
);
feature_check!(
    "rate-limiting",
    rate_limiting_available,
    "Check if rate limiting is available"
);
feature_check!(
    "batch-write",
    batch_write_available,
    "Check if batch write is available"
);
feature_check!(
    "wal-recovery",
    wal_recovery_available,
    "Check if WAL recovery is available"
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
    "database",
    database_available,
    "Check if database is available"
);
feature_check!("cli", cli_available, "Check if CLI is available");
feature_check!(
    "opentelemetry",
    opentelemetry_available,
    "Check if OpenTelemetry is available"
);

// ============================================================================
// FeatureSet Structure
// ============================================================================

/// Unified feature availability check
#[derive(Debug, Clone)]
pub struct FeatureSet {
    /// L1 cache available
    l1_available: bool,
    /// L2 cache available
    l2_available: bool,
    /// Metrics available
    metrics_available: bool,
    /// Bloom filter available
    bloom_available: bool,
    /// Rate limiting available
    rate_limiting_available: bool,
    /// Batch write available
    batch_write_available: bool,
    /// WAL recovery available
    wal_recovery_available: bool,
    /// Serialization available
    serialization_available: bool,
    /// Compression available
    compression_available: bool,
    /// Database available
    database_available: bool,
    /// CLI available
    cli_available: bool,
    /// OpenTelemetry available
    opentelemetry_available: bool,
}

impl FeatureSet {
    /// Create feature set from current features
    pub fn current() -> Self {
        Self {
            l1_available: l1_available(),
            l2_available: l2_available(),
            metrics_available: metrics_available(),
            bloom_available: bloom_available(),
            rate_limiting_available: rate_limiting_available(),
            batch_write_available: batch_write_available(),
            wal_recovery_available: wal_recovery_available(),
            serialization_available: serialization_available(),
            compression_available: compression_available(),
            database_available: database_available(),
            cli_available: cli_available(),
            opentelemetry_available: opentelemetry_available(),
        }
    }

    /// Get tier name
    pub fn tier_name(&self) -> &'static str {
        if self.opentelemetry_available && self.database_available && self.cli_available {
            "full"
        } else if self.l2_available && self.metrics_available {
            "core"
        } else if self.l1_available {
            "minimal"
        } else {
            "core"
        }
    }

    /// Check if L1 cache is available
    pub fn is_l1_available(&self) -> bool {
        self.l1_available
    }

    /// Check if L2 cache is available
    pub fn is_l2_available(&self) -> bool {
        self.l2_available
    }

    /// Check if metrics are available
    pub fn is_metrics_available(&self) -> bool {
        self.metrics_available
    }

    /// Check if bloom filter is available
    pub fn is_bloom_available(&self) -> bool {
        self.bloom_available
    }

    /// Check if rate limiting is available
    pub fn is_rate_limiting_available(&self) -> bool {
        self.rate_limiting_available
    }

    /// Check if batch write is available
    pub fn is_batch_write_available(&self) -> bool {
        self.batch_write_available
    }

    /// Check if WAL recovery is available
    pub fn is_wal_recovery_available(&self) -> bool {
        self.wal_recovery_available
    }

    /// Check if serialization is available
    pub fn is_serialization_available(&self) -> bool {
        self.serialization_available
    }

    /// Check if compression is available
    pub fn is_compression_available(&self) -> bool {
        self.compression_available
    }

    /// Check if database is available
    pub fn is_database_available(&self) -> bool {
        self.database_available
    }

    /// Check if CLI is available
    pub fn is_cli_available(&self) -> bool {
        self.cli_available
    }

    /// Check if OpenTelemetry is available
    pub fn is_opentelemetry_available(&self) -> bool {
        self.opentelemetry_available
    }
}

impl Default for FeatureSet {
    fn default() -> Self {
        Self::current()
    }
}
