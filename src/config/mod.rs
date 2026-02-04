//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Configuration management module
//!
//! This module provides configuration structures for the cache library.

#[cfg(feature = "confers")]
pub mod confers_config;

#[cfg(feature = "confers")]
pub use confers_config::{
    BackendConfig, BackendType, CacheType, GlobalConfig, MetricsConfig, PerformanceConfig,
    RecoveryConfig, SecurityConfig, ServiceConfig, UnifiedConfig, UnifiedConfigBuilder,
};
