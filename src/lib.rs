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
#[macro_export]
macro_rules! require_feature {
    ($required:expr, $dependent:expr) => {
        const _: fn() = || {
            if !$required && cfg!(feature = $dependent) {
                panic!(
                    "Feature '{}' requires feature '{}' to be enabled",
                    $dependent,
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
    feature = "full"
))]
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
#[cfg(any(
    feature = "utils",
    feature = "full",
    feature = "minimal",
    feature = "core"
))]
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
pub use config::{
    CacheStrategy, Config, DynamicConfig, GlobalConfig, OxcacheConfig, ServiceConfig,
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
pub use config::{CacheType, L1Config, L2Config, RedisMode, SerializationType, TwoLevelConfig};

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

const _: fn() = || {
    if cfg!(feature = "bloom-filter") && !cfg!(feature = "l1-moka") && !cfg!(feature = "full") {
        panic!("'bloom-filter' feature requires 'l1-moka' feature to be enabled");
    }

    if cfg!(feature = "rate-limiting") && !cfg!(feature = "l1-moka") && !cfg!(feature = "full") {
        panic!("'rate-limiting' feature requires 'l1-moka' feature to be enabled");
    }

    if cfg!(feature = "wal-recovery") && !cfg!(feature = "l2-redis") && !cfg!(feature = "full") {
        panic!("'wal-recovery' feature requires 'l2-redis' feature to be enabled");
    }

    if cfg!(feature = "batch-write") && !cfg!(feature = "l2-redis") && !cfg!(feature = "full") {
        panic!("'batch-write' feature requires 'l2-redis' feature to be enabled");
    }

    if cfg!(feature = "cli") && !cfg!(feature = "confers") && !cfg!(feature = "full") {
        panic!("'cli' feature requires 'confers' feature to be enabled");
    }

    if cfg!(feature = "opentelemetry") && !cfg!(feature = "metrics") && !cfg!(feature = "full") {
        panic!("'opentelemetry' feature requires 'metrics' feature to be enabled");
    }

    if cfg!(feature = "database") && !cfg!(feature = "l2-redis") && !cfg!(feature = "full") {
        panic!("'database' feature requires 'l2-redis' feature to be enabled");
    }
};
