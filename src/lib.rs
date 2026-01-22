//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! oxcache - 高性能多层缓存库
//!
//! 提供L1内存缓存和L2分布式缓存的两级缓存解决方案，
//! 支持缓存降级、故障恢复和优雅关闭等功能。
//!
//! # Modern API (Recommended)
//!
//! The new API (v0.2.0+) provides a type-safe, independent cache interface:
//!
//! ```rust,ignore
//! use oxcache::Cache;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize, Debug)]
//! struct User {
//!     id: u64,
//!     name: String,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Simple memory cache
//!     let cache: Cache<String, User> = Cache::new().await?;
//!
//!     // Set a value
//!     let user = User { id: 1, name: "Alice".to_string() };
//!     cache.set(&"user:1".to_string(), &user).await?;
//!
//!     // Get a value
//!     let user: Option<User> = cache.get(&"user:1".to_string()).await?;
//!
//!     // Cache-aside pattern with fallback
//!     let user: User = cache.get_or(&"user:1".to_string(), || async {
//!         fetch_user_from_db(1).await
//!     }).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Cache Types
//!
//! ## Memory Cache
//!
//! ```rust,ignore
//! let cache: Cache<String, MyType> = Cache::new().await?;
//! // or
//! let cache: Cache<String, MyType> = Cache::memory().await?;
//! ```
//!
//! ## Redis Cache
//!
//! ```rust,ignore
//! let cache: Cache<String, MyType> = Cache::redis("redis://localhost:6379").await?;
//! ```
//!
//! ## Tiered Cache (L1 + L2)
//!
//! ```rust,ignore
//! let cache: Cache<String, MyType> = Cache::tiered(10000, "redis://localhost:6379").await?;
//! ```
//!
//! # Advanced Configuration
//!
//! ```rust,ignore
//! use oxcache::{Cache, builder::BackendBuilder};
//! use std::time::Duration;
//!
//! let cache: Cache<String, User> = Cache::builder()
//!     .backend(
//!         BackendBuilder::tiered()
//!             .l1_capacity(10000)
//!             .l2_connection_string("redis://localhost:6379")
//!             .auto_promote(true)
//!     )
//!     .ttl(Duration::from_secs(3600))
//!     .build()
//!     .await?;
//! ```
//!
//! # Key Types
//!
//! The new API supports any type implementing `CacheKey`:
//!
//! ```rust,ignore
//! // String keys (default)
//! let cache: Cache<String, User> = Cache::new().await?;
//!
//! // Numeric keys
//! let cache: Cache<u64, User> = Cache::new().await?;
//!
//! // Custom key type
//! impl oxcache::traits::CacheKey for UserId {
//!     fn to_key_string(&self) -> String {
//!         format!("user:{}", self.0)
//!     }
//! }
//!
//! let cache: Cache<UserId, User> = Cache::new().await?;
//! ```
//!
//! # Batch Operations
//!
//! ```rust,ignore
//! // Batch set
//! cache.set_many(vec![
//!     (&"key1".to_string(), &value1),
//!     (&"key2".to_string(), &value2),
//! ]).await?;
//!
//! // Batch get
//! let results: HashMap<String, User> = cache.get_many(vec![
//!     &"key1".to_string(),
//!     &"key2".to_string(),
//! ]).await?;
//!
//! // Batch delete
//! cache.delete_many(vec![
//!     &"key1".to_string(),
//!     &"key2".to_string(),
//! ]).await?;
//! ```
//!
//! # Migration from Old API
//!
//! If you're using the old API (v0.1.x), see the migration guide:
//! - [Migration Guide](https://docs.rs/oxcache/latest/oxcache/docs/migration/index.html)
//!
//! The old API is deprecated but still functional. To migrate:
//!
//! Old API:
//! ```rust,ignore
//! let config = oxcache_config()
//!     .with_service("default", ServiceConfig::two_level())
//!     .build();
//! oxcache::init(config).await?;
//! let client = oxcache::get_client("default")?;
//! ```
//!
//! New API:
//! ```rust,ignore
//! let cache: Cache<String, User> = Cache::tiered(10000, "redis://localhost:6379").await?;
//! ```
//!
//! # Features
//!
//! - `l1-moka`: Enable L1 memory cache (Moka)
//! - `l2-redis`: Enable L2 distributed cache (Redis)
//! - `serialization`: Enable JSON/Bincode serialization
//! - `metrics`: Enable OpenTelemetry metrics
//! - `wal-recovery`: Enable write-ahead log for recovery
//! - `batch-write`: Enable optimized batch writes
//! - `full`: Enable all features
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::Cache;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a tiered cache
//!     let cache: Cache<String, String> = Cache::tiered(
//!         10000,
//!         "redis://localhost:6379"
//!     ).await?;
//!
//!     // Set and get
//!     cache.set(&"key".to_string(), &"value".to_string()).await?;
//!     let value = cache.get(&"key".to_string()).await?;
//!
//!     println!("Got value: {:?}", value);
//!     Ok(())
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/oxcache/0.2.0")]

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

/// 编译时断言：检查特性依赖关系（支持full特性）
///
/// # Example
///
/// ```rust,ignore
/// check_feature_dependence!("bloom-filter", cfg!(feature = "l1-moka"));
/// ```
///
/// 如果启用了 `bloom-filter` 但没有启用 `l1-moka` 或 `full`，编译时会panic。
#[macro_export]
macro_rules! check_feature_dependence {
    ($dependent:expr, $required:expr) => {
        const _: fn() = || {
            if cfg!(feature = $dependent) && !$required && !cfg!(feature = "full") {
                panic!(
                    "ERROR: '{}' feature requires '{}' feature.\n\
                    \n\
                    Solution 1: Enable required feature:\n\
                        oxcache = {{ version = \"0.1\", features = [\"{}\", \"{}\"] }}\n\
                    \n\
                    Solution 2: Enable all features:\n\
                        oxcache = {{ version = \"0.1\", features = [\"full\"] }}",
                    $dependent,
                    stringify!($required)
                        .replace("cfg!(feature = \\\"", "")
                        .replace("\\\")\"", ""),
                    $dependent,
                    stringify!($required)
                        .replace("cfg!(feature = \\\"", "")
                        .replace("\\\")\"", "")
                )
            }
        };
    };
}

/// 运行时检查特性是否启用（用于available_features等场景）
#[macro_export]
macro_rules! add_feature_if_enabled {
    ($features:ident, $name:expr) => {
        if cfg!(feature = $name) {
            $features.push($name);
        }
    };
}

/// 为禁用的特性生成空实现结构体及其基本实现
///
/// # Example
///
/// ```rust,ignore
/// empty_struct!(BatchWriter, Debug, Clone, Default);
/// ```
///
/// 生成：
/// - `#[cfg(not(feature = "batch-write"))]`
/// - `#[derive(Debug, Clone, Default)] pub struct BatchWriter;`
/// - `impl BatchWriter { pub fn new() -> Self { Self } }
#[macro_export]
macro_rules! empty_struct {
    ($name:ident, $($traits:ident),+ $(,)?) => {
        #[cfg(not(feature = "batch-write"))]
        #[derive($($traits),+)]
        pub struct $name;
        #[cfg(not(feature = "batch-write"))]
        impl $name {
            pub fn new() -> Self {
                Self
            }
        }
    };
}

/// 为禁用的特性生成空实现结构体（带泛型参数）
///
/// # Example
///
/// ```rust,ignore
/// empty_struct_generic!(HealthChecker<T: HealthCheckableBackend>, Debug);
/// ```
#[macro_export]
macro_rules! empty_struct_generic {
    ($name:ident $(<$($generics:tt),+>)?, $($traits:ident),+ $(,)?) => {
        #[cfg(not(feature = "wal-recovery"))]
        #[derive($($traits),+)]
        pub struct $name $(<$($generics),+>)?;
        #[cfg(not(feature = "wal-recovery"))]
        impl $(<$($generics),+>)? $name $(<$($generics),+>)? {
            pub fn new() -> Self {
                Self
            }
        }
    };
}

/// 为禁用的特性生成带有 async 方法的空实现
///
/// # Example
///
/// ```rust,ignore
/// empty_async_methods!(MyStruct, {
///     pub async fn start(&self) {}
///     pub async fn shutdown(&self) {}
/// });
/// ```
#[macro_export]
macro_rules! empty_async_methods {
    ($name:ident, { $(pub async fn $fn_name:ident(&self $(, $param:ident: $param_type: ty)* $(,)? ) -> Result<()> { $($body:stmt)* })+ }) => {
        #[cfg(not(feature = "batch-write"))]
        #[derive(Debug, Clone, Default)]
        pub struct $name;
        #[cfg(not(feature = "batch-write"))]
        impl $name {
            $(
                pub async fn $fn_name(&self $(, $param: $param_type)*) -> Result<()> {
                    $($body)*
                    Ok(())
                }
            )+
        }
    };
}

/// 为禁用的特性生成空 trait 定义
///
/// # Example
///
/// ```rust,ignore
/// empty_trait!(HealthCheckableBackend, Clone + Send + Sync + 'static {
///     async fn ping(&self) -> Result<()>;
///     fn command_timeout_ms(&self) -> u64;
/// });
/// ```
#[macro_export]
macro_rules! empty_trait {
    ($name:ident, $($bounds:tt)*) => {
        #[cfg(not(feature = "wal-recovery"))]
        #[async_trait::async_trait]
        pub trait $name: $($bounds)* {}
    };
}

/// 生成空的 Result 返回方法
///
/// # Example
///
/// ```rust,ignore
/// empty_async_fn!(pub async fn foo(&self) -> Result<()>);
/// empty_async_fn!(pub async fn bar(&self, key: &str) -> Result<()>);
/// ```
#[macro_export]
macro_rules! empty_async_fn {
    (pub async fn $fn_name:ident (&self $(, $param:ident: $param_type: ty)*) -> Result<()>) => {
        #[cfg(not(feature = "batch-write"))]
        pub async fn $fn_name(&self $(, $param: $param_type)*) -> Result<()> {
            Ok(())
        }
    };
}

/// 为禁用功能的模块生成占位符模块声明
///
/// # Example
///
/// ```rust,ignore
/// placeholder_module!(batch_writer, "batch-write");
/// ```
#[macro_export]
macro_rules! placeholder_module {
    ($module:ident, $feature:expr) => {
        #[cfg(not(feature = $feature))]
        pub(crate) mod $module;
    };
}

// ============================================================================
// Core Modules (Always Available)
// ============================================================================

pub mod client;
pub mod config;
pub mod error;
pub mod manager;

// New modernized API modules
pub mod builder;
pub mod cache;
pub mod traits;

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

// Smart Strategy Module
#[cfg(any(feature = "smart-strategy", feature = "full"))]
pub mod smart_strategy;

// HTTP Cache Module
#[cfg(any(feature = "http-cache", feature = "full"))]
pub mod http;

// Security Module (Always available for internal use)
#[cfg(any(feature = "l2-redis", feature = "core", feature = "full"))]
pub mod security;

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

// Note: Legacy manager functions (init, get_client, etc.) are deprecated and no longer re-exported.
// Use Cache::new(), Cache::redis(), or Cache::tiered() instead.

// New API exports
pub use builder::{BackendBuilder, CacheBuilder, TieredCacheBuilder};
pub use cache::Cache;
pub use traits::{CacheKey, Cacheable};

// Custom tiered backend configuration exports
#[cfg(any(
    feature = "l1-moka",
    feature = "l2-redis",
    feature = "core",
    feature = "full"
))]
pub use backend::custom_tiered::{
    AutoFixConfig, BackendType, ConfigFix, ConfigValidationResult, CustomTieredConfig,
    CustomTieredConfigBuilder, FixedConfigResult, Layer, LayerBackendConfig, LayerRestriction,
};

#[cfg(any(feature = "l2-redis", feature = "core", feature = "full"))]
pub use sync::warmup::{WarmupManager, WarmupResult, WarmupStatus};

pub use config::oxcache_config;

#[cfg(test)]
pub use config::{L1Config, L2Config, TwoLevelConfig};

#[cfg(any(feature = "full", feature = "minimal", feature = "core"))]
pub use utils::key_generator::KeyGenerator;

// Smart Strategy exports
#[cfg(any(feature = "smart-strategy", feature = "full"))]
pub use smart_strategy::{
    CompressibilityChecker, CompressionDecider, HitRateCollector, HitRateStats, PrefetchDecider,
    SmartStrategyConfig, SmartStrategyManager,
};

// Enhanced Stats exports
#[cfg(any(feature = "enhanced-stats", feature = "metrics", feature = "full"))]
pub use metrics::{export_json_format, export_prometheus_format, get_enhanced_stats, CacheStats};

// HTTP Cache exports
#[cfg(any(feature = "http-cache", feature = "full"))]
pub use http::{
    CacheMiddlewareConfig, CacheMiddlewareState, HttpCacheAdapter, HttpCacheKeyGenerator,
    HttpCachePolicy, HttpCacheResponse, HttpRequest,
};

// ============================================================================
// Configuration Macros (Feature-Gated)
// ============================================================================

#[cfg(feature = "confers")]
#[macro_export]
macro_rules! init_config {
    () => {
        let config = $crate::config::confers_macro::confers_load("oxcache.toml")
            .map_err(|e| $crate::error::CacheError::ConfigError(e.to_string()))?;
        $crate::manager::CacheManager::init(config).await
    };
    ($path:expr) => {
        let config = $crate::config::confers_macro::confers_load($path)
            .map_err(|e| $crate::error::CacheError::ConfigError(e.to_string()))?;
        $crate::manager::CacheManager::init(config).await
    };
}

#[cfg(feature = "confers")]
pub async fn init_from_confers(path: &str) -> Result<()> {
    use crate::config::confers_macro::confers_load;
    use crate::manager::CacheManager;
    let config =
        confers_load(path).map_err(|e| crate::error::CacheError::ConfigError(e.to_string()))?;
    CacheManager::init(config).await
}

/// 从配置文件初始化缓存系统
///
/// # Arguments
/// * `config_path` - 配置文件路径，支持 TOML 格式
///
/// # Example
/// ```rust,ignore
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     oxcache::init_from_file("config.toml").await?;
///     Ok(())
/// }
/// ```
#[cfg(feature = "confers")]
pub async fn init_from_file(config_path: &str) -> Result<()> {
    use crate::config::confers_macro::confers_load;
    use crate::manager::CacheManager;
    let config = confers_load(config_path).map_err(crate::error::CacheError::ConfigError)?;
    CacheManager::init(config).await
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
    // 使用统一的宏检查特性依赖
    check_feature_dependence!("bloom-filter", cfg!(feature = "l1-moka"));
    check_feature_dependence!("rate-limiting", cfg!(feature = "l1-moka"));
    check_feature_dependence!("wal-recovery", cfg!(feature = "l2-redis"));
    check_feature_dependence!("batch-write", cfg!(feature = "l2-redis"));
    check_feature_dependence!("cli", cfg!(feature = "confers"));
    check_feature_dependence!("opentelemetry", cfg!(feature = "metrics"));
    check_feature_dependence!("database", cfg!(feature = "l2-redis"));
};
