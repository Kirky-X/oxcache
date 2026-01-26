// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 补充配置模块测试 - 验证配置系统功能

use oxcache::config::{UnifiedConfig, ServiceConfig, CacheType};

#[test]
fn test_unified_config_creation() {
    let config = UnifiedConfig::memory_only().build();
    
    assert_eq!(config.backend.backend_type, oxcache::config::BackendType::Memory);
}

#[test]
fn test_service_config_l1_type() {
    let config = ServiceConfig::l1_only();
    assert_eq!(config.cache_type, CacheType::L1);
}

#[test]
fn test_service_config_l2_type() {
    let config = ServiceConfig::l2_only();
    assert_eq!(config.cache_type, CacheType::L2);
}

#[test]
fn test_service_config_two_level_type() {
    let config = ServiceConfig::two_level();
    assert_eq!(config.cache_type, CacheType::TwoLevel);
}

#[test]
fn test_service_config_with_ttl() {
    let config = ServiceConfig::two_level().with_ttl(600);
    assert_eq!(config.ttl, Some(600));
}

#[test]
fn test_unified_config_memory_backend() {
    let config = UnifiedConfig::memory_only().build();
    assert_eq!(config.backend.backend_type, oxcache::config::BackendType::Memory);
}

#[test]
fn test_unified_config_redis_backend() {
    let config = UnifiedConfig::redis_only().build();
    assert_eq!(config.backend.backend_type, oxcache::config::BackendType::Redis);
}

#[cfg(all(feature = "moka", feature = "redis"))]
#[test]
fn test_unified_config_tiered_backend() {
    let config = UnifiedConfig::tiered().build();
    assert_eq!(config.backend.backend_type, oxcache::config::BackendType::Tiered);
}