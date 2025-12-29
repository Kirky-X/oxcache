//! oxcache - 高性能多层缓存库
//!
//! 提供L1内存缓存和L2分布式缓存的两级缓存解决方案，
//! 支持缓存降级、故障恢复和优雅关闭等功能。

#![doc(html_root_url = "https://docs.rs/oxcache/0.1.0")]

pub use serde;
pub use serde::{Deserialize, Serialize};
pub use serde_json;
pub use tokio;

pub mod backend;
pub mod bloom_filter;
pub mod cli;
pub mod client;
pub mod config;
pub mod database;
pub mod debug_test;
pub mod error;
pub mod manager;
pub mod metrics;
pub mod rate_limiting;
pub mod recovery;
pub mod serialization;
pub mod sync;
pub mod utils;

// Re-export commonly used items
pub use client::{CacheExt, CacheOps};
pub use config::Config;
pub use manager::{get_client, CacheManager};
pub use sync::warmup::{WarmupManager, WarmupResult, WarmupStatus};

/// oxcache 版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
