// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Backend client implementations
//!
//! 各实现模块按其依赖的开启特性门控：dashmap 依赖由 `memory`/`offload`
//! 开启，moka 依赖由 `memory`/`byte-weight` 开启；redis 仅在 `redis` 下编译。

#[cfg(any(feature = "memory", feature = "offload"))]
pub mod dashmap;
#[cfg(test)]
pub mod mock;
#[cfg(any(feature = "memory", feature = "byte-weight"))]
pub mod moka;
#[cfg(feature = "redis")]
pub mod redis;

/// Macro to implement the common `new()` and `builder()` pattern for backends.
///
/// # Example
///
/// ```ignore
/// impl_backend_builder!(MyBackend, MyBackendBuilder);
/// // Expands to:
/// // impl MyBackend {
/// //     pub fn new() -> Self { Self::builder().build() }
/// //     pub fn builder() -> MyBackendBuilder { MyBackendBuilder::default() }
/// // }
/// ```
#[macro_export]
macro_rules! impl_backend_builder {
    ($backend:ty, $builder:ty) => {
        impl $backend {
            pub fn new() -> Self {
                Self::builder().build()
            }

            pub fn builder() -> $builder {
                <$builder>::default()
            }
        }
    };
}

// Memory backend type enumeration
// serde derives 依赖 `memory`/`serialization` 任一开启的 serde
#[cfg(any(feature = "memory", feature = "serialization"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MemoryBackendType {
    /// Moka backend (LRU/TinyLFU with automatic expiration)
    Moka,
    /// DashMap backend (concurrent hashmap with manual TTL)
    DashMap,
}

// Re-export all client backends for convenience
#[cfg(any(feature = "memory", feature = "offload"))]
pub use dashmap::DashMapMemoryBackend;
#[cfg(any(feature = "memory", feature = "byte-weight"))]
pub use moka::MokaMemoryBackend;
#[cfg(feature = "redis")]
pub use redis::{RedisBackend, RedisBackendBuilder, RedisMode};

// Convenience functions for creating memory backends
#[cfg(any(feature = "memory", feature = "offload"))]
pub use dashmap::dashmap_memory;
#[cfg(any(feature = "memory", feature = "byte-weight"))]
pub use moka::{default_memory_backend, moka_memory};

// Re-export builder helpers for external test usage
#[cfg(any(feature = "memory", feature = "offload"))]
pub use dashmap::{
    DashMapBackendBuilder, dashmap_memory_with_capacity, dashmap_memory_with_capacity_and_ttl,
};
#[cfg(any(feature = "memory", feature = "byte-weight"))]
pub use moka::{
    MokaMemoryBackendBuilder, moka_memory_with_capacity, moka_memory_with_capacity_and_ttl,
};

// Re-export MockBackend for crate-internal test usage
#[cfg(test)]
pub use mock::MockBackend;
