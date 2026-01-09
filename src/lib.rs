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
#[cfg(all(not(feature = "test"), not(feature = "cli")))]
mod cli;
#[cfg(all(not(feature = "test"), feature = "cli"))]
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
pub mod cli;
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

// 重新导出公共 API
pub use client::{CacheExt, CacheOps};
pub use config::{CacheStrategy, Config, DynamicConfig, EvictionPolicy};
pub use error::{CacheError, Result};
pub use manager::{
    get_client, get_strategy, list_strategies, update_eviction_policy, update_l1_capacity,
    update_strategy, update_ttl, CacheManager, clear_all_strategies, reset_strategy,
};
pub use utils::key_generator::KeyGenerator;

// 重新导出预热功能（从内部模块导出）
pub use sync::warmup::{WarmupManager, WarmupResult, WarmupStatus};

/// oxcache 版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
