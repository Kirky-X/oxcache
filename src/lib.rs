//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! oxcache - 高性能多层缓存库
//!
//! 提供L1内存缓存和L2分布式缓存的两级缓存解决方案，
//! 支持缓存降级、故障恢复和优雅关闭等功能。

#![doc(html_root_url = "https://docs.rs/oxcache/0.1.0")]

// 公共 API 模块
pub mod client;
pub mod config;
pub mod error;
pub mod manager;

// 内部实现模块（不对外暴露）
#[cfg(not(feature = "test"))]
mod backend;
#[cfg(not(feature = "test"))]
mod bloom_filter;
#[cfg(any(feature = "cli", feature = "test"))]
pub mod cli;
#[cfg(not(feature = "test"))]
mod database;
#[cfg(not(feature = "test"))]
mod debug_test;
#[cfg(not(feature = "test"))]
mod metrics;
#[cfg(not(feature = "test"))]
mod rate_limiting;
#[cfg(not(feature = "test"))]
mod recovery;
#[cfg(not(feature = "test"))]
mod serialization;
#[cfg(not(feature = "test"))]
mod sync;
#[cfg(not(feature = "test"))]
mod utils;

// 内部实现模块（仅在测试时公开）
#[cfg(feature = "test")]
pub mod backend;
#[cfg(feature = "test")]
pub mod bloom_filter;
#[cfg(feature = "test")]
pub use cli::*;
#[cfg(feature = "test")]
pub mod database;
#[cfg(feature = "test")]
pub mod debug_test;
#[cfg(feature = "test")]
pub mod metrics;
#[cfg(feature = "test")]
pub mod rate_limiting;
#[cfg(feature = "test")]
pub mod recovery;
#[cfg(feature = "test")]
pub mod serialization;
#[cfg(feature = "test")]
pub mod sync;
#[cfg(feature = "test")]
pub mod utils;

#[cfg(feature = "test")]
pub use config::{CacheType, L1Config, L2Config, RedisMode, SerializationType, TwoLevelConfig};

// 重新导出公共 API
pub use client::{CacheExt, CacheOps};
pub use config::legacy_config::{
    CacheStrategy as LegacyCacheStrategy, DynamicConfig as LegacyDynamicConfig,
};
pub use config::{
    CacheStrategy, Config, ConfigSource, DynamicConfig, EvictionPolicy, GlobalConfig, LayerConfig,
    OxcacheConfig, ServiceConfig,
};
pub use error::{CacheError, Result};
pub use manager::{
    clear_all_strategies, get_client, get_strategy, init, list_strategies, reset_strategy,
    update_eviction_policy, update_l1_capacity, update_strategy, update_ttl, CacheManager,
};
pub use utils::key_generator::KeyGenerator;

// 重新导出预热功能（从内部模块导出）
pub use sync::warmup::{WarmupManager, WarmupResult, WarmupStatus};

// 导出配置构建函数
pub use config::oxcache_config;

// 导出 confers 宏（需要 confers 特性）
#[cfg(feature = "confers")]
#[macro_export]
macro_rules! init_config {
    () => {
        // 从默认配置文件加载
        let config = $crate::config::confers_macro::confers_load("oxcache.toml")
            .map_err(|e| $crate::error::CacheError::ConfigError(e.to_string()))?;
        $crate::init(config).await
    };
    ($path:expr) => {
        // 从指定路径加载配置文件
        let config = $crate::config::confers_macro::confers_load($path)
            .map_err(|e| $crate::error::CacheError::ConfigError(e.to_string()))?;
        $crate::init(config).await
    };
}

/// 便捷函数：使用 confers 从 TOML 文件加载配置并初始化
#[cfg(feature = "confers")]
pub async fn init_from_confers(path: &str) -> Result<()> {
    use crate::config::confers_macro::confers_load;
    let config =
        confers_load(path).map_err(|e| crate::error::CacheError::ConfigError(e.to_string()))?;
    init(config).await
}

/// oxcache 版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
