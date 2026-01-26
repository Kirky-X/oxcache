//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Backend client implementations

pub mod dashmap;
pub mod moka;
#[cfg(feature = "redis")]
pub mod redis;

// Memory backend type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MemoryBackendType {
    /// Moka backend (LRU/TinyLFU with automatic expiration)
    Moka,
    /// DashMap backend (concurrent hashmap with manual TTL)
    DashMap,
}

// Re-export all client backends for convenience
pub use dashmap::DashMapMemoryBackend;
pub use moka::MokaMemoryBackend;
pub use moka::MokaMemoryBackend as MemoryBackend; // 为向后兼容提供别名
#[cfg(feature = "redis")]
pub use redis::{
    DefaultRedisProvider, RedisBackend, RedisBackendBuilder, RedisMode, RedisProvider,
};

// Convenience functions for creating memory backends
pub use dashmap::dashmap_memory;
pub use moka::default_memory_backend;
pub use moka::moka_memory;
