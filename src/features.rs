//! Feature flags and conditional compilation utilities for oxcache.
//!
//! This module provides a centralized way to check which features are enabled
//! at compile time, enabling zero-cost abstractions for feature detection.

/// Unified feature availability check
#[derive(Debug, Clone)]
pub struct FeatureSet {
    /// L1 cache available
    pub l1_available: bool,
    /// L2 cache available
    pub l2_available: bool,
    /// Metrics available
    pub metrics_available: bool,
    /// Bloom filter available
    pub bloom_available: bool,
    /// Rate limiting available
    pub rate_limiting_available: bool,
    /// Batch write available
    pub batch_write_available: bool,
    /// WAL recovery available
    pub wal_recovery_available: bool,
    /// Serialization available
    pub serialization_available: bool,
    /// Compression available
    pub compression_available: bool,
    /// Database available
    pub database_available: bool,
    /// CLI available
    pub cli_available: bool,
    /// OpenTelemetry available
    pub opentelemetry_available: bool,
}

impl FeatureSet {
    /// Create feature set from current features
    pub fn current() -> Self {
        Self {
            l1_available: cfg!(feature = "l1-moka"),
            l2_available: cfg!(feature = "l2-redis"),
            metrics_available: cfg!(feature = "metrics"),
            bloom_available: cfg!(feature = "bloom-filter"),
            rate_limiting_available: cfg!(feature = "rate-limiting"),
            batch_write_available: cfg!(feature = "batch-write"),
            wal_recovery_available: cfg!(feature = "wal-recovery"),
            serialization_available: cfg!(feature = "serialization"),
            compression_available: cfg!(feature = "compression"),
            database_available: cfg!(feature = "database"),
            cli_available: cfg!(feature = "cli"),
            opentelemetry_available: cfg!(feature = "opentelemetry"),
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
}

impl Default for FeatureSet {
    fn default() -> Self {
        Self::current()
    }
}

// ============================================================================
// Individual Feature Availability Functions
// ============================================================================

/// Check if L1 cache is available
#[cfg(feature = "l1-moka")]
pub fn l1_available() -> bool {
    true
}

/// Check if L1 cache is available (stub)
#[cfg(not(feature = "l1-moka"))]
pub fn l1_available() -> bool {
    false
}

/// Check if L2 cache is available
#[cfg(feature = "l2-redis")]
pub fn l2_available() -> bool {
    true
}

/// Check if L2 cache is available (stub)
#[cfg(not(feature = "l2-redis"))]
pub fn l2_available() -> bool {
    false
}

/// Check if metrics are available
#[cfg(feature = "metrics")]
pub fn metrics_available() -> bool {
    true
}

/// Check if metrics are available (stub)
#[cfg(not(feature = "metrics"))]
pub fn metrics_available() -> bool {
    false
}

/// Check if bloom filter is available
#[cfg(feature = "bloom-filter")]
pub fn bloom_available() -> bool {
    true
}

/// Check if bloom filter is available (stub)
#[cfg(not(feature = "bloom-filter"))]
pub fn bloom_available() -> bool {
    false
}

/// Check if rate limiting is available
#[cfg(feature = "rate-limiting")]
pub fn rate_limiting_available() -> bool {
    true
}

/// Check if rate limiting is available (stub)
#[cfg(not(feature = "rate-limiting"))]
pub fn rate_limiting_available() -> bool {
    false
}

/// Check if batch write is available
#[cfg(feature = "batch-write")]
pub fn batch_write_available() -> bool {
    true
}

/// Check if batch write is available (stub)
#[cfg(not(feature = "batch-write"))]
pub fn batch_write_available() -> bool {
    false
}

/// Check if WAL recovery is available
#[cfg(feature = "wal-recovery")]
pub fn wal_recovery_available() -> bool {
    true
}

/// Check if WAL recovery is available (stub)
#[cfg(not(feature = "wal-recovery"))]
pub fn wal_recovery_available() -> bool {
    false
}

/// Check if serialization is available
#[cfg(feature = "serialization")]
pub fn serialization_available() -> bool {
    true
}

/// Check if serialization is available (stub)
#[cfg(not(feature = "serialization"))]
pub fn serialization_available() -> bool {
    false
}

/// Check if compression is available
#[cfg(feature = "compression")]
pub fn compression_available() -> bool {
    true
}

/// Check if compression is available (stub)
#[cfg(not(feature = "compression"))]
pub fn compression_available() -> bool {
    false
}

/// Check if database is available
#[cfg(feature = "database")]
pub fn database_available() -> bool {
    true
}

/// Check if database is available (stub)
#[cfg(not(feature = "database"))]
pub fn database_available() -> bool {
    false
}

/// Check if CLI is available
#[cfg(feature = "cli")]
pub fn cli_available() -> bool {
    true
}

/// Check if CLI is available (stub)
#[cfg(not(feature = "cli"))]
pub fn cli_available() -> bool {
    false
}

/// Check if OpenTelemetry is available
#[cfg(feature = "opentelemetry")]
pub fn opentelemetry_available() -> bool {
    true
}

/// Check if OpenTelemetry is available (stub)
#[cfg(not(feature = "opentelemetry"))]
pub fn opentelemetry_available() -> bool {
    false
}
