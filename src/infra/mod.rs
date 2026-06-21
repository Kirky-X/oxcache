//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Infrastructure module
//!
//! Provides infrastructure components: metrics, serialization, telemetry, warmup, db_loader, cli

pub mod metrics;
pub mod serialization;
pub(crate) mod telemetry;

use crate::error::CacheError;

const MAX_CACHE_KEY_LENGTH: usize = 1024;
const VALID_KEY_CHARS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w',
    'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T',
    'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '-', '_', '.', ':', '/', '@',
];

/// Validate cache key format
pub fn validate_cache_key(key: &str) -> Result<(), CacheError> {
    if key.is_empty() {
        return Err(CacheError::InvalidInput("Cache key cannot be empty".to_string()));
    }

    if key.len() > MAX_CACHE_KEY_LENGTH {
        return Err(CacheError::InvalidInput(format!(
            "Cache key exceeds maximum length of {} bytes (got {} bytes)",
            MAX_CACHE_KEY_LENGTH,
            key.len()
        )));
    }

    for c in key.chars() {
        if !VALID_KEY_CHARS.contains(&c) {
            return Err(CacheError::InvalidInput(format!(
                "Cache key contains invalid character '{}'. Valid characters are: alphanumeric and -_.:/@",
                c
            )));
        }
    }

    Ok(())
}

#[cfg(feature = "metrics")]
pub use metrics::{export_json_format, export_prometheus_format, get_enhanced_stats, CacheStats};
