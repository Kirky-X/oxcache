// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Backend client implementations

pub mod dashmap;
#[cfg(test)]
pub mod mock;
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
#[cfg(feature = "redis")]
pub use redis::{RedisBackend, RedisBackendBuilder, RedisMode};

// Convenience functions for creating memory backends
pub use dashmap::dashmap_memory;
pub use moka::default_memory_backend;
pub use moka::moka_memory;
