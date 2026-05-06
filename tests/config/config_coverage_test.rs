// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 补充配置模块测试 - 验证配置系统功能

use oxcache::config::{CacheType, ServiceConfig, UnifiedConfigBuilder};
use oxcache::core::confers_config::ConfigBackendType;

#[test]
fn test_unified_config_creation() {
    let config = UnifiedConfigBuilder::memory_only().build().unwrap();

    assert_eq!(config.backend.backend_type_enum(), ConfigBackendType::Memory);
}

#[test]
fn test_service_config_l1_type() {
    let config = ServiceConfig::l1_only();
    assert_eq!(config.cache_type_enum(), CacheType::L1);
}

#[test]
fn test_service_config_l2_type() {
    let config = ServiceConfig::l2_only();
    assert_eq!(config.cache_type_enum(), CacheType::L2);
}

#[test]
fn test_service_config_two_level_type() {
    let config = ServiceConfig::two_level();
    assert_eq!(config.cache_type_enum(), CacheType::TwoLevel);
}

#[test]
fn test_service_config_with_ttl() {
    let config = ServiceConfig::two_level().with_ttl(600);
    assert_eq!(config.ttl, Some(600));
}

#[test]
fn test_unified_config_memory_backend() {
    let config = UnifiedConfigBuilder::memory_only().build().unwrap();
    assert_eq!(config.backend.backend_type_enum(), ConfigBackendType::Memory);
}

#[test]
fn test_unified_config_redis_backend() {
    let config = UnifiedConfigBuilder::redis_only().build().unwrap();
    assert_eq!(config.backend.backend_type_enum(), ConfigBackendType::Redis);
}

#[cfg(all(feature = "moka", feature = "redis"))]
#[test]
fn test_unified_config_tiered_backend() {
    let config = UnifiedConfigBuilder::tiered().build().unwrap();
    assert_eq!(config.backend.backend_type_enum(), ConfigBackendType::Tiered);
}
