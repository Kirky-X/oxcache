//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! oxcache - 高性能多层缓存库
//!
//! 提供L1内存缓存和L2分布式缓存的两级缓存解决方案，
//! 支持缓存降级、故障恢复和优雅关闭等功能。

#![doc(html_root_url = "https://docs.rs/oxcache/0.1.0")]

// ============================================================================
// Feature Flags and Macros
// ============================================================================

/// 检查是否启用了指定的功能特性
#[macro_export]
macro_rules! has_feature {
    ($feature:expr) => {
        cfg!(feature = $feature)
    };
}

/// 编译时断言：确保功能依赖满足
///
/// # Example
///
/// ```rust,ignore
/// require_feature!(cfg!(feature = "l1-moka"), "bloom-filter");
/// ```
///
/// 如果启用了 `bloom-filter` 但没有启用 `l1-moka`，编译时会panic。
/// 这是为了在编译时捕获配置错误，而不是在运行时。
#[macro_export]
macro_rules! require_feature {
    ($required:expr, $dependent:expr) => {
        const _: fn() = || {
            if !$required && cfg!(feature = $dependent) {
                panic!(
                    "Feature '{}' requires feature '{}' to be enabled. \
                    Add '{}' to your Cargo.toml features or use the 'full' feature.",
                    $dependent,
                    stringify!($required),
                    stringify!($required)
                )
            }
        };
    };
}

// ============================================================================
// Core Modules (Always Available)
// ============================================================================

pub mod client;
pub mod config;
pub mod error;
pub mod manager;

// ============================================================================
// Optional Feature-Gated Modules
// ============================================================================

// Backend module (L1/L2 cache implementation)
#[cfg(any(
    feature = "l1-moka",
    feature = "l2-redis",
    feature = "minimal",
    feature = "core",
    feature = "full"
))]
pub mod backend;

// Bloom Filter Module
#[cfg(any(feature = "bloom-filter", feature = "full"))]
pub mod bloom_filter;

// Metrics Module - also needed for L1-only mode
#[cfg(any(
    feature = "metrics",
    feature = "l1-moka",
    feature = "l2-redis",
    feature = "minimal",
    feature = "core",
    feature = "full",
    feature = "batch-write",
    feature = "cli"
))]
#[allow(unexpected_cfgs)]
pub mod metrics;

// Rate Limiting Module
#[cfg(any(feature = "rate-limiting", feature = "full"))]
pub mod rate_limiting;

// WAL Recovery Module
#[cfg(any(feature = "wal-recovery", feature = "l2-redis", feature = "full"))]
pub mod recovery;

// Sync writer
#[cfg(any(
    feature = "batch-write",
    feature = "l2-redis",
    feature = "sync",
    feature = "full"
))]
pub mod sync;

// Database Module
#[cfg(any(feature = "database", feature = "full"))]
pub mod database;

// CLI Module
#[cfg(any(feature = "cli", feature = "full"))]
pub mod cli;

// OpenTelemetry Module
#[cfg(any(feature = "opentelemetry", feature = "full"))]
pub mod telemetry;

// Serialization Module
#[cfg(any(feature = "serialization", feature = "full"))]
pub mod serialization;

// Utils Module
#[cfg(any(feature = "full", feature = "minimal", feature = "core"))]
pub mod utils;

// Test-only modules
#[cfg(feature = "test")]
pub mod debug_test;

// ============================================================================
// Public API Re-exports
// ============================================================================

pub use client::{CacheExt, CacheOps};
pub use config::legacy_config::{
    CacheStrategy as LegacyCacheStrategy, DynamicConfig as LegacyDynamicConfig,
};
#[allow(deprecated)]
pub use config::Config;
pub use config::{
    CacheStrategy, CacheType, DynamicConfig, GlobalConfig, OxcacheConfig, OxcacheConfigBuilder,
    RedisMode, SerializationType, ServiceConfig,
};

#[cfg(feature = "confers")]
pub use config::ConfigSource;

#[cfg(feature = "l1-moka")]
pub use config::LayerConfig;
// Use LegacyEvictionPolicy from legacy_config to avoid type conflict
pub use config::LegacyEvictionPolicy as EvictionPolicy;
pub use error::{CacheError, Result};
pub use manager::{
    clear_all_strategies, get_client, get_strategy, init, list_strategies, reset_strategy,
    update_eviction_policy, update_l1_capacity, update_strategy, update_ttl, CacheManager,
};

#[cfg(any(feature = "l2-redis", feature = "core", feature = "full"))]
pub use sync::warmup::{WarmupManager, WarmupResult, WarmupStatus};

pub use config::oxcache_config;

#[cfg(feature = "test")]
pub use config::{L1Config, L2Config, TwoLevelConfig};

#[cfg(any(feature = "full", feature = "minimal", feature = "core"))]
pub use utils::key_generator::KeyGenerator;

// ============================================================================
// Configuration Macros (Feature-Gated)
// ============================================================================

#[cfg(feature = "confers")]
#[macro_export]
macro_rules! init_config {
    () => {
        let config = $crate::config::confers_macro::confers_load("oxcache.toml")
            .map_err(|e| $crate::error::CacheError::ConfigError(e.to_string()))?;
        $crate::init(config).await
    };
    ($path:expr) => {
        let config = $crate::config::confers_macro::confers_load($path)
            .map_err(|e| $crate::error::CacheError::ConfigError(e.to_string()))?;
        $crate::init(config).await
    };
}

#[cfg(feature = "confers")]
pub async fn init_from_confers(path: &str) -> Result<()> {
    use crate::config::confers_macro::confers_load;
    let config =
        confers_load(path).map_err(|e| crate::error::CacheError::ConfigError(e.to_string()))?;
    init(config).await
}

/// oxcache 版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================================================
// Compile-time Feature Validation
// ============================================================================
// 编译时特性验证
//
// 这些静态断言确保功能依赖在编译时就被正确配置。
// 如果启用了不兼容的特性组合，编译将会失败并显示清晰的错误消息。
//
// # 示例
//
// ```toml,ignore
// [dependencies]
// oxcache = { version = "0.1", features = ["bloom-filter"] }
// ```
//
// 上面的配置会导致编译错误，因为 `bloom-filter` 需要 `l1-moka` 特性。
// 使用 `full` 特性可以启用所有功能：
//
// ```toml,ignore
// [dependencies]
// oxcache = { version = "0.1", features = ["full"] }
// ```

const _: fn() = || {
    // Bloom filter requires L1 cache for storing bloom filter data
    if cfg!(feature = "bloom-filter") && !cfg!(feature = "l1-moka") && !cfg!(feature = "full") {
        panic!(
            "ERROR: 'bloom-filter' feature requires 'l1-moka' feature.\n\
            \n\
            Solution 1: Enable L1 cache support:\n\
                oxcache = {{ version = \"0.1\", features = [\"bloom-filter\", \"l1-moka\"] }}\n\
            \n\
            Solution 2: Enable all features:\n\
                oxcache = {{ version = \"0.1\", features = [\"full\"] }}"
        );
    }

    // Rate limiting requires L1 cache for token bucket storage
    if cfg!(feature = "rate-limiting") && !cfg!(feature = "l1-moka") && !cfg!(feature = "full") {
        panic!(
            "ERROR: 'rate-limiting' feature requires 'l1-moka' feature.\n\
            \n\
            Solution 1: Enable L1 cache support:\n\
                oxcache = {{ version = \"0.1\", features = [\"rate-limiting\", \"l1-moka\"] }}\n\
            \n\
            Solution 2: Enable all features:\n\
                oxcache = {{ version = \"0.1\", features = [\"full\"] }}"
        );
    }

    // WAL recovery requires Redis backend for persistent storage
    if cfg!(feature = "wal-recovery") && !cfg!(feature = "l2-redis") && !cfg!(feature = "full") {
        panic!(
            "ERROR: 'wal-recovery' feature requires 'l2-redis' feature.\n\
            \n\
            Solution 1: Enable Redis backend:\n\
                oxcache = {{ version = \"0.1\", features = [\"wal-recovery\", \"l2-redis\"] }}\n\
            \n\
            Solution 2: Enable all features:\n\
                oxcache = {{ version = \"0.1\", features = [\"full\"] }}"
        );
    }

    // Batch write requires Redis backend for batch operations
    if cfg!(feature = "batch-write") && !cfg!(feature = "l2-redis") && !cfg!(feature = "full") {
        panic!(
            "ERROR: 'batch-write' feature requires 'l2-redis' feature.\n\
            \n\
            Solution 1: Enable Redis backend:\n\
                oxcache = {{ version = \"0.1\", features = [\"batch-write\", \"l2-redis\"] }}\n\
            \n\
            Solution 2: Enable all features:\n\
                oxcache = {{ version = \"0.1\", features = [\"full\"] }}"
        );
    }

    // CLI requires configuration file support
    if cfg!(feature = "cli") && !cfg!(feature = "confers") && !cfg!(feature = "full") {
        panic!(
            "ERROR: 'cli' feature requires 'confers' feature for configuration file support.\n\
            \n\
            Solution 1: Enable configuration support:\n\
                oxcache = {{ version = \"0.1\", features = [\"cli\", \"confers\"] }}\n\
            \n\
            Solution 2: Enable all features:\n\
                oxcache = {{ version = \"0.1\", features = [\"full\"] }}"
        );
    }

    // OpenTelemetry requires metrics support for tracing
    if cfg!(feature = "opentelemetry") && !cfg!(feature = "metrics") && !cfg!(feature = "full") {
        panic!(
            "ERROR: 'opentelemetry' feature requires 'metrics' feature for tracing.\n\
            \n\
            Solution 1: Enable metrics support:\n\
                oxcache = {{ version = \"0.1\", features = [\"opentelemetry\", \"metrics\"] }}\n\
            \n\
            Solution 2: Enable all features:\n\
                oxcache = {{ version = \"0.1\", features = [\"full\"] }}"
        );
    }

    // Database features require Redis backend for fallback
    if cfg!(feature = "database") && !cfg!(feature = "l2-redis") && !cfg!(feature = "full") {
        panic!("'database' feature requires 'l2-redis' feature to be enabled");
    }
};
