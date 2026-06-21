// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 该模块定义了缓存系统的后端提供者。

// Backend score system
pub mod score;

// Memory backend implementations
pub mod memory;

// Modernized API backend interface
pub mod interface;

// Client module (backend implementations: dashmap, moka, redis)
pub mod client;

// Storage module (database backends)
#[cfg(any(feature = "database", feature = "full", test))]
pub mod storage;

// Configuration validation utilities
pub mod config_validation;

// Custom tiered backend configuration (always available)
#[cfg(any(
    feature = "moka",
    feature = "redis",
    feature = "minimal",
    feature = "core",
    feature = "full"
))]
pub mod custom_tiered;

// Re-exports for new API
pub use interface::{CacheBackend, CacheConnector, CacheReader, CacheWriter};

// Re-export BackendKind for runtime type identification
pub use interface::BackendKind;

// Score system exports
pub use score::{BackendScore, Scores};

// Memory backend implementations
pub use memory::{
    dashmap_memory,
    default_memory_backend,
    // Convenience functions
    moka_memory,
    DashMapMemoryBackend,
    MemoryBackend,
    // Type definitions
    MemoryBackendType,
    MokaMemoryBackend,
};

#[cfg(feature = "redis")]
pub use memory::{RedisBackend, RedisBackendBuilder, RedisMode};

// Re-exports for custom tiered configuration
#[cfg(any(feature = "moka", feature = "redis", feature = "full", feature = "core"))]
pub use custom_tiered::LayerRestriction;

// 从 core::types 重新导出统一的枚举类型
pub use crate::core::types::{BackendType, CacheLayer};
