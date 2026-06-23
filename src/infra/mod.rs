//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Infrastructure module
//!
//! Provides infrastructure components: metrics, serialization, telemetry, warmup, db_loader, cli

pub mod metrics;
pub mod serialization;

use crate::error::CacheError;

/// Validate cache key format
pub fn validate_cache_key(key: &str) -> Result<(), CacheError> {
    crate::utils::key_generator::KeyGenerator::new().validate_key(key)
}

#[cfg(feature = "metrics")]
pub use metrics::{export_json_format, export_prometheus_format, get_enhanced_stats, CacheStats};
