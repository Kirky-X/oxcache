// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 该模块定义了缓存系统的后端提供者。

// Backend score system
pub mod score;

// Client implementations
pub mod client;

// Modernized API backend interface
pub mod interface;

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
pub use interface::CacheBackend;

// Score system exports
pub use score::{BackendScore, Scores};

// Client implementations
pub use client::{
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
pub use client::{
    RedisBackend as ClientRedisBackend, RedisBackendBuilder, RedisMode as ClientRedisMode,
};

// Re-exports for custom tiered configuration
#[cfg(any(
    feature = "moka",
    feature = "redis",
    feature = "full",
    feature = "core"
))]
pub use custom_tiered::{
    AutoFixConfig, BackendProvider, BackendType, ConfigFix, ConfigValidationResult,
    CustomTieredConfig, CustomTieredConfigBuilder, DefaultBackendProvider, FixedConfigResult,
    Layer, LayerBackendConfig, LayerRestriction,
};
