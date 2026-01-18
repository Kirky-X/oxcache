//! Feature flags and conditional compilation utilities for oxcache.
//!
//! This module provides a centralized way to check which features are enabled
//! at compile time, enabling zero-cost abstractions for feature detection.

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
