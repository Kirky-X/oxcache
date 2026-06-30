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
//! # Sync API (0.3.0)
//!
//! Enable `sync_mode(true)` on the builder to get synchronous methods
//! (`get_sync` / `set_sync` / `set_with_ttl_sync` / `delete_sync` /
//! `exists_sync` / `get_or_sync` / `clear_sync`) alongside the async API.
//! Requires `multi_thread` tokio runtime for Moka-backed caches.
//!
//! ```rust,ignore
//! # #[tokio::main(flavor = "multi_thread")]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await?;
//! cache.set_sync(&"k".to_string(), &"v".to_string())?;
//! let v = cache.get_sync(&"k".to_string())?;
//! # Ok(()) }
//! ```
//!
//! # Bloom Filter (0.3.0)
//!
//! Enable the `bloom-filter` feature (not in `full`) for negative-query
//! filtering. [`BloomFilterBackend`] wraps any [`CacheBackend`] and skips
//! the inner backend on BF miss.
//!
//! ```rust,ignore
//! use oxcache::backend::MokaMemoryBackend;
//! use oxcache::features::bloom_filter::BloomFilterBackend;
//! let backend = BloomFilterBackend::new(MokaMemoryBackend::new());
//! ```
//!
//! # Universal per-entry TTL (0.3.0)
//!
//! All backends (Moka / DashMap / Redis / Mock / Chain / Bloom) honor
//! per-entry `set(key, value, Some(ttl))`. Moka uses the `moka::Expiry`
//! trait for real per-entry TTL (overriding the global TTL set on the
//! builder).
//!
//! # Features
//!
//! - `moka`: L1 memory cache (default in minimal/core/full)
//! - `redis`: L2 distributed cache
//! - `serialization`: JSON/Bincode/MessagePack/CBOR
//! - `metrics`: OpenTelemetry metrics
//! - `bloom-filter`: negative-query filtering (not in `full`)
//! - `full`: All features

#![doc(html_root_url = "https://docs.rs/oxcache/0.3.0")]
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
// Must be `pub` (not `pub(crate)`) so the #[cached] macro can access
// __internal_get_cache from external crates. #[doc(hidden)] keeps it out of public docs.
#[doc(hidden)]
pub mod internal;

// ============================================================================
// Primary Modules (Feature-Gated)
// ============================================================================

// Cache module (modern Cache<K,V> API)
pub mod cache;

// Backend module (L1/L2 cache implementation)
#[cfg(any(
    feature = "memory",
    feature = "redis",
    feature = "minimal",
    feature = "core",
    feature = "full"
))]
pub mod backend;

// Features module (optional capabilities)
pub mod features;

// Infrastructure module (metrics, serialization, telemetry, etc.)
#[cfg(any(
    feature = "metrics",
    feature = "memory",
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

// Registry module for #[cached] macro support
pub mod registry;

// Traits module: CacheKey
pub mod traits;

// Config module: confers-based configuration
pub mod config;

// Utils module: key generation utilities
pub mod utils;

// Security module: Redis security validation
pub(crate) mod security;

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

// Re-export internal functions needed by #[cached] macro at crate root
// The macro generates code calling ::oxcache::__internal_get_cache()
#[doc(hidden)]
pub use crate::internal::__internal_get_cache;

// ============================================================================
// New API (Recommended)
// ============================================================================

// New API exports
pub use cache::builder::CacheBuilder;
pub use cache::Cache;

// Re-exports from infra module
#[cfg(any(feature = "metrics", feature = "full"))]
pub use infra::{export_json_format, export_prometheus_format, get_enhanced_stats, CacheStats};

// Re-exports from security module (new brick architecture)
#[cfg(any(feature = "redis", feature = "full"))]
pub use crate::security::{
    clamp_scan_count,
    log::{log_cache_key, sanitize_message},
    redaction::{redact_cache_key, redact_connection_string, redact_field, redact_value, Redacted},
    validate_lua_script, validate_redis_key, validate_scan_pattern,
};

// Public API re-exports (after features re-exports)
pub use cache::chain::{ChainCache, ChainCacheBuilder, ChainLink};
pub use cache::interface::UnifiedCache;
pub use traits::CacheKey;

// Type-safe enum exports
pub use core::types::{BackendType, CacheLayer, RedisModeType, SerializationType};

// Key generator export
pub use crate::utils::KeyGenerator;

// Events module re-export
pub use core::events::{CacheEvent, CacheEventType, EventPublisher};

// Backend exports
pub use backend::{
    dashmap_memory, default_memory_backend, moka_memory, BackendScore, DashMapMemoryBackend, MemoryBackendType,
    MokaMemoryBackend, Scores,
};

#[cfg(feature = "redis")]
pub use backend::{RedisBackend, RedisBackendBuilder, RedisMode};

// ============================================================================
// Factory Functions (Brick Architecture Standard)
// ============================================================================

/// oxcache 版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use crate::VERSION;

    // 测试 check_feature_dependence! 宏
    // 当 full feature 启用时，宏的 cfg 条件为 false，不会触发 compile_error
    #[test]
    fn test_check_feature_dependence_macro_no_error() {
        // 调用宏，使用已启用的 feature，不应触发 compile_error
        check_feature_dependence!("memory", "redis");
    }

    #[test]
    fn test_check_feature_dependence_macro_same_feature() {
        // 使用相同的 feature 名
        check_feature_dependence!("memory", "memory");
    }

    #[test]
    fn test_version_constant() {
        // 测试 VERSION 常量不为空
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_version_format() {
        // 测试 VERSION 格式（应该包含数字）
        assert!(VERSION.chars().any(|c: char| c.is_ascii_digit()));
    }
}
