//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! oxcache - 高性能多层缓存库
//!
//! 提供L1内存缓存和L2分布式缓存的两级缓存解决方案，
//! 支持缓存降级、故障恢复和优雅关闭等功能。
//!
//! # 初始化（可选）
//!
//! 如果使用 `#[cached]` 宏，需要先初始化全局注册表：
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use oxcache::{new_in_memory, init};
//!
//! #[tokio::main]
//! async fn main() {
//!     // 初始化缓存注册表
//!     let cache = Arc::new(new_in_memory());
//!     oxcache::init(cache);
//!
//!     // 现在可以使用 #[cached] 宏
//!     run_app().await;
//! }
//! ```
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
//!     let cache: Cache<String, User> = Cache::builder().build().await?;
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
//! ## Memory Cache (L1)
//!
//! ```rust,ignore
//! let cache: Cache<String, MyType> = Cache::builder().build().await?;
//! ```
//!
//! ## Redis Cache (L2)
//!
//! ```rust,ignore
//! let cache: Cache<String, MyType> = Cache::redis("redis://localhost:6379").await?;
//! ```
//!
//! ## Tiered Cache (L1 + L2)
//!
//! ```rust,ignore
//! use oxcache::OxCacheBuilder;
//!
//! let cache = OxCacheBuilder::tiered(10000, "redis://localhost:6379")
//!     .await?
//!     .build()?;
//! ```
//!
//! # Advanced Configuration
//!
//! ```rust,ignore
//! use oxcache::{Cache, OxCacheBuilder};
//! use std::time::Duration;
//!
//! let cache: Cache<String, User> = Cache::builder()
//!     .with_backend(
//!         oxcache::backend::MokaMemoryBackend::builder()
//!             .capacity(10000)
//!             .build()
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
//! let cache: Cache<String, User> = Cache::builder().build().await?;
//!
//! // Numeric keys
//! let cache: Cache<u64, User> = Cache::builder().build().await?;
//!
//! // Custom key type
//! impl oxcache::traits::CacheKey for UserId {
//!     fn to_key_string(&self) -> String {
//!         format!("user:{}", self.0)
//!     }
//! }
//!
//! let cache: Cache<UserId, User> = Cache::builder().build().await?;
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
//! # #[cached] Macro
//!
//! Use the `#[cached]` attribute to automatically cache function results:
//!
//! ```rust,ignore
//! use oxcache::Cache;
//!
//! // First, register a cache for macro usage
//! let cache = Cache::<String, Vec<u8>>::builder().build().await?;
//! cache.register_for_macro("my_cache").await;
//!
//! // Then use the macro on functions
//! #[cached(service = "my_cache", ttl = 300)]
//! async fn get_user(id: u64) -> User {
//!     // This result will be cached automatically
//!     database::fetch_user(id).await
//! }
//! ```
//!
//! # Features
//!
//! - `moka`: Enable L1 memory cache (Moka)
//! - `dashmap-backend`: Enable DashMap backend (pure concurrent in-memory)
//! - `redis`: Enable L2 distributed cache (Redis)
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
//!     // Create a memory cache
//!     let cache: Cache<String, String> = Cache::builder().build().await?;
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
#![deny(unsafe_code)]

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

/// 编译时特性依赖检查
///
/// 使用 `compile_error!` 提供清晰的编译时错误信息。
/// 注意：$required 应为特性名称字符串，而非 cfg 表达式。
///
/// # Example
///
/// ```rust,ignore
/// require_feature!("moka", "bloom-filter");
/// ```
///
/// 如果启用了 `bloom-filter` 但没有启用 `moka`，编译时会报错。
#[macro_export]
macro_rules! require_feature {
    ($required:expr, $dependent:expr) => {
        #[cfg(all(feature = $dependent, not(feature = $required)))]
        compile_error!(concat!(
            "Feature '",
            $dependent,
            "' requires the '",
            $required,
            "' feature to be enabled.\n",
            "Add '",
            $required,
            "' to your Cargo.toml features, ",
            "or use the 'full' feature for all capabilities."
        ));
    };
}

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
pub mod client;
pub mod core;
pub mod error;

// Internal module for #[cached] macro support
#[doc(hidden)]
pub(crate) mod internal;

// Registry module for explicit cache initialization
pub mod registry;

// New modernized API modules
pub mod builder;
pub mod cache;
pub mod traits;

// Configuration module (using Confers library)
#[cfg(feature = "confers")]
pub mod config;

// ============================================================================
// Optional Feature-Gated Modules
// ============================================================================

// Backend module (L1/L2 cache implementation)
#[cfg(any(
    feature = "moka",
    feature = "redis",
    feature = "minimal",
    feature = "core",
    feature = "full"
))]
pub mod backend;

// Bloom Filter Module
#[cfg(any(feature = "bloom-filter", feature = "full"))]
pub(crate) mod bloom_filter;

// Re-export bloom filter public API
#[cfg(any(feature = "bloom-filter", feature = "full"))]
pub use bloom_filter::{BloomFilter, BloomFilterOptions};

// Metrics Module - also needed for L1-only mode
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
pub mod metrics;

// Rate Limiting Module
#[cfg(any(feature = "rate-limiting", feature = "full"))]
pub(crate) mod rate_limiting;

// Re-export rate limiting public API
#[cfg(any(feature = "rate-limiting", feature = "full"))]
pub use rate_limiting::{ClientRateLimiter, GlobalRateLimiter, RateLimitConfig, RateLimitStatus};

// WAL Recovery Module
#[cfg(any(feature = "wal-recovery", feature = "redis", feature = "full"))]
pub mod recovery;

// Sync writer
#[cfg(any(feature = "batch-write", feature = "redis", feature = "sync", feature = "full"))]
pub mod sync;

// Database Module
#[cfg(any(feature = "database", feature = "full"))]
pub mod database;

// OpenTelemetry Module
#[cfg(any(feature = "opentelemetry", feature = "full"))]
pub mod telemetry;

// Serialization Module
#[cfg(any(feature = "serialization", feature = "full"))]
pub mod serialization;

// Utils Module
#[cfg(any(
    feature = "full",
    feature = "minimal",
    feature = "core",
    feature = "moka",
    feature = "redis"
))]
pub mod utils;

// Smart Strategy Module
#[cfg(any(feature = "smart-strategy", feature = "full"))]
pub(crate) mod smart_strategy;

// Re-export smart strategy public API
#[cfg(any(feature = "smart-strategy", feature = "full"))]
pub use smart_strategy::{
    CompressibilityChecker, CompressionDecider, HitRateCollector, HitRateStats, PrefetchDecider, SmartStrategyConfig,
    SmartStrategyManager,
};

// HTTP Cache Module
#[cfg(any(feature = "http-cache", feature = "full"))]
pub mod http;

// SingleFlight Module
#[cfg(any(feature = "singleflight", feature = "full"))]
pub(crate) mod singleflight;

// Security Module (Only needed for Redis validation)
#[cfg(any(feature = "redis", feature = "full"))]
pub(crate) mod security;

// Re-export security public API for external use
#[cfg(any(feature = "redis", feature = "full"))]
pub use security::{
    clamp_scan_count,
    log::{log_cache_key, sanitize_message},
    redaction::{redact_cache_key, redact_connection_string, redact_field, redact_value, Redacted},
    validate_lua_script, validate_redis_key, validate_scan_pattern,
};

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
pub use builder::{CacheBuilder, OxCacheBuilder};
pub use cache::Cache;

// Configuration module exports (using Confers library)
// Note: confers_config types are exported when confers feature is enabled
#[cfg(feature = "confers")]
pub use config::confers_config;

pub use cache::chain::{ChainCache, ChainCacheBuilder, ChainLink};
pub use cache::interface::UnifiedCache;
pub use traits::{CacheKey, Cacheable};

// Type-safe enum exports
pub use core::types::{BackendType, CacheLayer, RedisModeType, SerializationType};

// Events module export
pub use core::events;

// DashMap backend exports (client)
#[cfg(feature = "dashmap")]
pub use backend::client::DashMapMemoryBackend as DashMapBackend;

// Unified memory backend exports (from client implementations)
pub use backend::{
    dashmap_memory, default_memory_backend, moka_memory, BackendScore, DashMapMemoryBackend, MemoryBackend,
    MemoryBackendType, MokaMemoryBackend, Scores,
};
// Client backend exports
pub use backend::client::{DashMapMemoryBackend as ClientDashMapBackend, MokaMemoryBackend as ClientMokaMemoryBackend};

#[cfg(any(feature = "redis", feature = "core", feature = "full"))]
pub use sync::warmup::{WarmupManager, WarmupStatus};

#[cfg(any(feature = "full", feature = "minimal", feature = "core"))]
pub use utils::key_generator::KeyGenerator;

// Enhanced Stats exports
#[cfg(any(feature = "enhanced-stats", feature = "metrics", feature = "full"))]
pub use metrics::{export_json_format, export_prometheus_format, get_enhanced_stats, CacheStats};

// HTTP Cache exports
#[cfg(any(feature = "http-cache", feature = "full"))]
pub use http::{
    CacheMiddlewareConfig, CacheMiddlewareState, HttpCacheAdapter, HttpCacheKeyGenerator, HttpCachePolicy,
    HttpCacheResponse, HttpRequest,
};

// Internal module exports (for #[cached] macro)
#[doc(hidden)]
pub use internal::{__internal_get_cache, __internal_register_cache};

// Feature info exports
pub use internal::{get_all_feature_info, get_l1_feature_info, get_l2_feature_info, is_l1_enabled, is_l2_enabled};

// Registry exports (for explicit initialization)
pub use registry::{clear, get, init, init_empty, is_initialized, register, remove};

// ============================================================================
// Factory Functions (Brick Architecture Standard)
// ============================================================================

/// Create a new in-memory cache backend with zero configuration.
///
/// This factory function provides a simple, dependency-free way to create
/// a cache instance for unit tests, feature module `new()` patterns, and
/// rapid prototyping.
///
/// # Returns
///
/// A new `MokaMemoryBackend` instance with default capacity.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::new_in_memory;
///
/// let cache = new_in_memory();
///
/// // Use the cache directly
/// cache.set("key", b"value".to_vec(), None).await?;
/// let value = cache.get("key").await?;
/// ```
///
/// # Feature Requirements
///
/// This function requires the `moka` feature (included in `minimal`, `core`, and `full`).
#[cfg(any(feature = "moka", feature = "minimal", feature = "core", feature = "full"))]
pub fn new_in_memory() -> backend::client::MokaMemoryBackend {
    backend::client::MokaMemoryBackend::new()
}

/// Create a new cache backend from configuration.
///
/// This factory function creates a cache instance based on the provided
/// configuration, supporting Memory, Redis, and Tiered backends.
///
/// # Arguments
///
/// * `config` - Backend configuration specifying type and options
///
/// # Returns
///
/// A new cache backend instance wrapped in `Arc<dyn CacheBackend>`.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::{new_with_config, config::BackendConfig};
///
/// let config = BackendConfig::default();
/// let cache = new_with_config(config).await?;
///
/// // Use the cache
/// cache.set("key", b"value".to_vec(), None).await?;
/// ```
///
/// # Feature Requirements
///
/// - `moka` feature for Memory backend
/// - `redis` feature for Redis backend
#[cfg(feature = "confers")]
pub async fn new_with_config(
    config: config::confers_config::BackendConfig,
) -> error::Result<std::sync::Arc<dyn backend::interface::CacheBackend>> {
    use std::sync::Arc;

    let backend_type = config.backend_type_enum();

    match backend_type {
        config::confers_config::BackendType::Memory => {
            #[cfg(feature = "moka")]
            {
                Ok(Arc::new(backend::client::MokaMemoryBackend::new()))
            }
            #[cfg(not(feature = "moka"))]
            {
                Err(error::CacheError::InvalidInput(
                    "Memory backend requires 'moka' feature".to_string(),
                ))
            }
        }
        config::confers_config::BackendType::Redis => {
            #[cfg(feature = "redis")]
            {
                let _redis_config = config.l2_options();
                // Create Redis backend with config
                let builder = backend::client::RedisBackend::builder();
                // TODO: Parse redis_config and configure builder
                Ok(Arc::new(builder.build().await?))
            }
            #[cfg(not(feature = "redis"))]
            {
                Err(error::CacheError::InvalidInput(
                    "Redis backend requires 'redis' feature".to_string(),
                ))
            }
        }
        config::confers_config::BackendType::Tiered => Err(error::CacheError::InvalidInput(
            "Tiered backend requires manual construction. Use ChainCache or TwoLevelCache builders.".to_string(),
        )),
    }
}

// ============================================================================
// Configuration Macros (Feature-Gated)
// ============================================================================

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
// 上面的配置会导致编译错误，因为 `bloom-filter` 需要 `moka` 特性。
// 使用 `full` 特性可以启用所有功能：
//
// ```toml,ignore
// [dependencies]
// oxcache = { version = "0.1", features = ["full"] }
// ```

const _: fn() = || {
    // 使用统一的宏检查特性依赖
    // 注意：第一个参数是必需的依赖特性名称，第二个是当前启用的特性
    check_feature_dependence!("moka", "bloom-filter");
    check_feature_dependence!("moka", "rate-limiting");
    check_feature_dependence!("redis", "wal-recovery");
    check_feature_dependence!("redis", "batch-write");
    check_feature_dependence!("confers", "cli");
    check_feature_dependence!("metrics", "opentelemetry");
    check_feature_dependence!("redis", "database");
};
