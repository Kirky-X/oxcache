// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 该模块定义了缓存系统的后端提供者。

// Client implementations
pub mod client;

#[cfg(feature = "l2-redis")]
pub mod strategy;

// Modernized API backend modules
pub mod backend;
pub mod tiered;

// Custom tiered backend configuration (always available)
#[cfg(any(
    feature = "l1-moka",
    feature = "l2-redis",
    feature = "full",
    feature = "core"
))]
pub mod custom_tiered;

// Re-exports for new API
pub use backend::CacheBackend;
pub use tiered::TieredBackend;

// Client implementations
pub use client::{
    dashmap_memory,
    default_memory_backend,
    // Convenience functions
    moka_memory,
    DashMapMemoryBackend,
    DefaultRedisProvider,
    MemoryBackend,
    // Type definitions
    MemoryBackendType,
    MokaMemoryBackend,
    RedisBackend as ClientRedisBackend,
    RedisBackendBuilder,
    RedisMode as ClientRedisMode,
    RedisProvider,
};

// Re-exports for custom tiered configuration
#[cfg(any(
    feature = "l1-moka",
    feature = "l2-redis",
    feature = "full",
    feature = "core"
))]
pub use custom_tiered::{
    AutoFixConfig, BackendProvider, BackendType, ConfigFix, ConfigValidationResult,
    CustomTieredConfig, CustomTieredConfigBuilder, DefaultBackendProvider, FixedConfigResult,
    Layer, LayerBackendConfig, LayerRestriction, TieredBackendFactory,
};
