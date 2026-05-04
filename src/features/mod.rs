//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Features module
//!
//! Optional feature-gated modules: strategy, security, http, recovery

#[cfg(any(
    feature = "bloom-filter",
    feature = "rate-limiting",
    feature = "smart-strategy",
    feature = "full"
))]
pub mod strategy;

#[cfg(any(feature = "redis", feature = "full"))]
pub mod security;

#[cfg(any(feature = "http-cache", feature = "full"))]
pub mod http;

#[cfg(any(feature = "wal-recovery", feature = "redis", feature = "full"))]
pub mod recovery;

// Re-exports for strategy
#[cfg(any(feature = "bloom-filter", feature = "full"))]
pub use strategy::{BloomFilter, BloomFilterOptions};

#[cfg(any(feature = "rate-limiting", feature = "full"))]
pub use strategy::{ClientRateLimiter, GlobalRateLimiter, RateLimitConfig, RateLimitStatus};

#[cfg(any(feature = "smart-strategy", feature = "full"))]
pub use strategy::{
    CompressibilityChecker, CompressionDecider, HitRateCollector, HitRateStats, PrefetchDecider, SmartStrategyConfig,
    SmartStrategyManager,
};

// Re-exports for security (public API)
#[cfg(any(feature = "redis", feature = "full"))]
pub use security::{
    clamp_scan_count,
    log::{log_cache_key, sanitize_message},
    redaction::{redact_cache_key, redact_connection_string, redact_field, redact_value, Redacted},
    validate_lua_script, validate_redis_key, validate_scan_pattern,
};

// Re-exports for HTTP cache
#[cfg(any(feature = "http-cache", feature = "full"))]
pub use http::{
    CacheMiddlewareConfig, CacheMiddlewareState, HttpCacheAdapter, HttpCacheKeyGenerator, HttpCachePolicy,
    HttpCacheResponse, HttpRequest,
};

// Re-exports for recovery (from wal submodule)
#[cfg(feature = "wal-recovery")]
pub use recovery::wal::{WalEntry, WalManager};
