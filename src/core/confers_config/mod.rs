// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Configuration module entry point

mod builder;
mod config;
mod types;

pub use builder::UnifiedConfigBuilder;
pub use config::{
    BackendConfig, ConfigProvider, GlobalConfig, MetricsConfig, PerformanceConfig, RecoveryConfig, SecurityConfig,
    ServiceConfig, UnifiedConfig,
};
pub use types::{CacheType, ConfigBackendType, ConfigFormat};
