//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! oxcache - 高性能多层缓存库
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::Cache;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize, Debug)]
//! struct User { id: u64, name: String }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let cache: Cache<String, User> = Cache::builder().build().await?;
//!     cache.set(&"user:1".to_string(), &User { id: 1, name: "Alice".into() }).await?;
//!     let user = cache.get(&"user:1".to_string()).await?;
//!     Ok(())
//! }
//! ```
//!
//! # Tiered Cache
//!
//! ```rust,ignore
//! use oxcache::cache::{ChainCache, ChainLink};
//! use oxcache::backend::MokaMemoryBackend;
//!
//! let l1 = MokaMemoryBackend::builder().capacity(10000).build();
//! let l2 = oxcache::backend::RedisBackend::new("redis://localhost:6379").await?;
//!
//! let chain = ChainCache::builder()
//!     .link(ChainLink::from_backend(l1))
//!     .link(ChainLink::from_backend(l2))
//!     .enable_backfill()
//!     .build();
//! ```
//!
//! # Features
//!
//! - `moka`: L1 memory cache (default in minimal/core/full)
//! - `redis`: L2 distributed cache
//! - `serialization`: JSON/Bincode/MessagePack/CBOR
//! - `metrics`: OpenTelemetry metrics
//! - `full`: All features

#![doc(html_root_url = "https://docs.rs/oxcache/0.2.0")]
#![deny(unsafe_code)]

// ============================================================================
// Feature Flags and Macros
// ============================================================================

/// 编译时特性依赖检查（支持 full 特性）
///
/// 注意：$required 应为特性名称字符串，而非 cfg 表达式。
///
/// # Example
///
/// ```rust,ignore
/// check_feature_dependence!("moka", "bloom-filter");
/// ```
///
/// 如果启用了 `bloom-filter` 但没有启用 `moka` 或 `full`，编译时会报错。
#[macro_export]
macro_rules! check_feature_dependence {
    ($required:expr, $dependent:expr) => {
        #[cfg(all(feature = $dependent, not(feature = $required), not(feature = "full")))]
        compile_error!(concat!(
            "Feature '",
            $dependent,
            "' requires '",
            $required,
            "' or 'full' feature.\n",
            "\nSolution 1: Enable required feature:\n",
            "    oxcache = { version = \"0.1\", features = [\"",
            $dependent,
            "\", \"",
            $required,
            "\"] }\n",
            "\nSolution 2: Enable all features:\n",
            "    oxcache = { version = \"0.1\", features = [\"full\"] }"
        ));
    };
}

/// Initialize cache configuration from a function.
///
/// This macro generates code that calls the provided function to get configuration,
/// then initializes all caches from that configuration.
///
/// # Arguments
/// * `path` (optional) - Path to a TOML config file. If provided, uses confers_load.
/// * `config` (optional) - A function that returns `OxcacheConfig`.
///
/// Either `path` or `config` must be provided, but not both.
///
/// # Example
///
/// ```rust,ignore
/// #[oxcache::init_config]
/// fn load_config() -> oxcache::OxcacheConfig {
///     oxcache::oxcache_config()
///         .with_service("default", oxcache::ServiceConfig::two_level())
///         .build()
/// }
/// ```
// ============================================================================
// Core Modules (Always Available)
// ============================================================================
pub mod core;
pub mod error;

// Internal module for #[cached] macro support
#[doc(hidden)]
pub(crate) mod internal;

// ============================================================================
// Primary Modules (Feature-Gated)
// ============================================================================

// Cache module (modern Cache<K,V> API)
pub mod cache;

// Backend module (L1/L2 cache implementation)
#[cfg(any(
    feature = "moka",
    feature = "redis",
    feature = "minimal",
    feature = "core",
    feature = "full"
))]
pub mod backend;

// Features module (optional capabilities)
#[cfg(any(
    feature = "bloom-filter",
    feature = "rate-limiting",
    feature = "smart-strategy",
    feature = "http-cache",
    feature = "wal-recovery",
    feature = "redis",
    feature = "full"
))]
pub mod features;

// Infrastructure module (metrics, serialization, telemetry, etc.)
#[cfg(any(
    feature = "metrics",
    feature = "moka",
    feature = "redis",
    feature = "minimal",
    feature = "core",
    feature = "full",
    feature = "batch-write",
    feature = "cli"
))]
pub mod infra;

// Mock Module (For testing only)
#[cfg(test)]
mod testing;

// ============================================================================
// Public API Re-exports
// ============================================================================

// Re-export macros when the feature is enabled
#[cfg(feature = "macros")]
pub use oxcache_macros::cached;

#[cfg(feature = "macros")]
pub mod macros {
    pub use oxcache_macros::*;
}

pub use error::{CacheConfigError, CacheError, ConfigResult, Result};

// ============================================================================
// New API (Recommended)
// ============================================================================

// New API exports
pub use cache::builder::CacheBuilder;
pub use cache::Cache;

// Re-exports from features module
#[cfg(any(feature = "bloom-filter", feature = "full"))]
pub use features::{BloomFilter, BloomFilterOptions};

#[cfg(any(feature = "rate-limiting", feature = "full"))]
pub use features::{ClientRateLimiter, GlobalRateLimiter, RateLimitConfig, RateLimitStatus};

#[cfg(any(feature = "smart-strategy", feature = "full"))]
pub use features::{
    CompressibilityChecker, CompressionDecider, HitRateCollector, HitRateStats, PrefetchDecider, SmartStrategyConfig,
    SmartStrategyManager,
};

// Re-exports from infra module
#[cfg(any(feature = "redis", feature = "core", feature = "full"))]
pub use infra::{WarmupManager, WarmupStatus};

#[cfg(any(feature = "full", feature = "minimal", feature = "core"))]
pub use infra::KeyGenerator;

#[cfg(any(feature = "enhanced-stats", feature = "metrics", feature = "full"))]
pub use infra::{export_json_format, export_prometheus_format, get_enhanced_stats, CacheStats};

// Re-exports from features module (security public API)
#[cfg(any(feature = "redis", feature = "full"))]
pub use features::security::{
    clamp_scan_count,
    log::{log_cache_key, sanitize_message},
    redaction::{redact_cache_key, redact_connection_string, redact_field, redact_value, Redacted},
    validate_lua_script, validate_redis_key, validate_scan_pattern,
};

// HTTP Cache exports from features module
#[cfg(any(feature = "http-cache", feature = "full"))]
pub use features::{
    CacheMiddlewareConfig, CacheMiddlewareState, HttpCacheAdapter, HttpCacheKeyGenerator, HttpCachePolicy,
    HttpCacheResponse, HttpRequest,
};

// Public API re-exports (after features re-exports)
pub use cache::chain::{ChainCache, ChainCacheBuilder, ChainLink, OxCache, OxCacheBuilder};
pub use cache::interface::UnifiedCache;
pub use core::traits::{CacheKey, Cacheable};

// Type-safe enum exports
pub use core::types::{BackendType, CacheLayer, RedisModeType, SerializationType};

// Events module export
pub use core::events;

// Backend exports
pub use backend::{
    dashmap_memory, default_memory_backend, moka_memory, BackendScore, DashMapMemoryBackend, MemoryBackend,
    MemoryBackendType, MokaMemoryBackend, Scores,
};

#[cfg(feature = "redis")]
pub use backend::{RedisBackend, RedisBackendBuilder, RedisMode};

// ============================================================================
// Factory Functions (Brick Architecture Standard)
// ============================================================================

/// Create a new in-memory cache backend with zero configuration.
#[cfg(any(feature = "moka", feature = "minimal", feature = "core", feature = "full"))]
pub fn new_in_memory() -> backend::memory::MokaMemoryBackend {
    backend::memory::MokaMemoryBackend::new()
}

/// oxcache 版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// Compile-time feature validation
const _: fn() = || {
    check_feature_dependence!("moka", "bloom-filter");
    check_feature_dependence!("moka", "rate-limiting");
    check_feature_dependence!("redis", "wal-recovery");
    check_feature_dependence!("redis", "batch-write");
    check_feature_dependence!("metrics", "opentelemetry");
    check_feature_dependence!("redis", "database");
};
