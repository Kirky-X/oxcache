// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// 该模块定义了缓存系统的后端提供者。

// Backend score system
pub mod score;

// Memory backend implementations
pub mod memory;

// Modernized API backend interface
pub mod interface;

// Configuration validation utilities
pub mod config_validation;

// Dragonfly backend (Redis-compatible, feature-gated)
#[cfg(feature = "dragonfly")]
pub mod dragonfly;

// Aerospike backend (independent protocol, feature-gated)
#[cfg(feature = "aerospike")]
pub mod aerospike;

// Custom tiered backend configuration (always available)
#[cfg(any(
    feature = "memory",
    feature = "redis",
    feature = "minimal",
    feature = "core",
    feature = "full"
))]
pub mod custom_tiered;

// Re-exports for new API
pub use interface::CacheSetItem;
pub use interface::{CacheBackend, CacheConnector, CacheReader, CacheWriter};
// Re-export atomic operation traits
pub use interface::{AtomicCacheWriter, SyncAtomicCacheWriter};
// Re-exports for synchronous API (任务组 5)
pub use interface::{SyncCacheBackend, SyncCacheConnector, SyncCacheReader, SyncCacheWriter};

// Re-export BackendKind for runtime type identification
pub use interface::BackendKind;

// Re-export LuaExecutor trait for Lua script execution
#[cfg(feature = "lua-script")]
pub use interface::LuaExecutor;

// Re-export ConfigValidation for configuration validation utilities
pub use config_validation::ConfigValidation;

// Dragonfly backend re-exports
#[cfg(feature = "dragonfly")]
pub use dragonfly::{DragonflyBackend, DragonflyRestrictions};

// Aerospike backend re-exports
#[cfg(feature = "aerospike")]
pub use aerospike::{AerospikeBackend, AerospikeConfig};

// Score system exports
pub use score::{BackendScore, Scores};

// Memory backend implementations
pub use memory::{
    DashMapMemoryBackend,
    // Type definitions
    MemoryBackendType,
    MokaMemoryBackend,
    dashmap_memory,
    default_memory_backend,
    // Convenience functions
    moka_memory,
};

// Re-export MockBackend for crate-internal test usage
#[cfg(test)]
pub use memory::MockBackend;

#[cfg(feature = "redis")]
pub use memory::{RedisBackend, RedisBackendBuilder, RedisMode};

// Re-exports for custom tiered configuration
#[cfg(any(
    feature = "memory",
    feature = "redis",
    feature = "minimal",
    feature = "full",
    feature = "core"
))]
pub use custom_tiered::LayerRestriction;

// 从 core::types 重新导出统一的枚举类型
pub use crate::core::{BackendType, CacheLayer};
