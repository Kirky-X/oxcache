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

// Storage module (database backends)
#[cfg(any(feature = "database", feature = "full", test))]
pub mod storage;

// Path validation utilities
pub mod path_validation;

// Configuration validation utilities
pub mod config_validation;

// Validation result types
pub mod validation_result;

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

// Path and config validation utilities
pub use config_validation::ConfigValidation;
pub use path_validation::PathValidationConfig;
pub use validation_result::{ConfigFix, ConfigValidationResult, FixedConfigResult, Layer};

// Re-exports for custom tiered configuration
#[cfg(any(feature = "moka", feature = "redis", feature = "full", feature = "core"))]
pub use custom_tiered::{
    AutoFixConfig, BackendProvider, CustomTieredConfig, CustomTieredConfigBuilder, DefaultBackendProvider,
    LayerBackendConfig, LayerRestriction,
};

// 从 core::types 重新导出统一的枚举类型
pub use crate::core::types::{BackendType, CacheLayer};
