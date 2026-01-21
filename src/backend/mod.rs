//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的后端提供者，包括L1和L2缓存后端。

pub mod l1;

pub mod l2;

pub mod redis_provider;

#[cfg(feature = "l2-redis")]
pub mod strategy;

// New modernized API backend modules
pub mod memory;
pub mod new_backend;
pub mod redis;
pub mod tiered;

// Re-exports for new API
pub use memory::MemoryBackend;
pub use new_backend::CacheBackend;
pub use redis::{RedisBackend, RedisMode};
pub use tiered::TieredBackend;
