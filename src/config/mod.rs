//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 统一配置模块入口
//!
//! Feature-gated 配置系统：
//! - L1 配置需要 moka feature  
//! - L2 配置需要 redis feature

pub mod layer;
pub mod service;
pub mod unified;

// Re-exports from unified module
pub use unified::{
    BackendConfig, BackendType, MemoryBackendConfig, PerformanceConfig, RedisBackendConfig,
    RedisConnectionConfig, RedisPoolConfig, SerializationConfig, UnifiedConfig,
};

pub use layer::{EvictionPolicy, L1LayerConfig, L2LayerConfig, LayerConfig, TwoLevelLayerConfig};

// Base exports (always available)
pub use service::CacheType;
pub use service::ServiceConfig;

#[cfg(feature = "moka")]
pub use service::L1Config;

#[cfg(feature = "redis")]
pub use service::{L2Config, TwoLevelConfig};
