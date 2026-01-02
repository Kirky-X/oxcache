//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 配置单元测试

use oxcache::config::{CacheType, Config, L1Config, L2Config, ServiceConfig};
use std::collections::HashMap;

/// 测试从TOML配置文件加载配置
///
/// 验证能否正确解析TOML格式的配置文件并创建配置对象
#[test]
fn test_config_load_from_toml() {
    let config_str = r#"
        [global]
        default_ttl = 3600
        health_check_interval = 5
        serialization = "json"
        enable_metrics = true
        
        [services.test_service]
        cache_type = "twolevel"
        
        [services.test_service.two_level]
        promote_on_hit = true
        enable_batch_write = false
        batch_size = 100
        batch_interval_ms = 1000

        [services.test_service.l1]
        max_capacity = 10000

        [services.test_service.l2]
        mode = "standalone"
        connection_string = "redis://127.0.0.1:6379"
        connection_timeout_ms = 1000
        command_timeout_ms = 1000
        enable_tls = false
    "#;

    let config: Config = toml::from_str(config_str).expect("Failed to parse TOML");

    assert_eq!(config.global.default_ttl, 3600);
    assert!(config.services.contains_key("test_service"));

    let service = config.services.get("test_service").unwrap();
    assert_eq!(service.cache_type, CacheType::TwoLevel);
    assert_eq!(service.l1.as_ref().unwrap().max_capacity, 10000);
}

/// 测试手动创建配置结构
///
/// 验证能否通过编程方式正确创建配置对象
#[test]
fn test_config_structure_manual_creation() {
    let config = Config {
        config_version: Some(1),
        global: oxcache::config::GlobalConfig {
            default_ttl: 60,
            health_check_interval: 10,
            serialization: oxcache::config::SerializationType::Json,
            enable_metrics: false,
        },
        services: {
            let mut map = HashMap::new();
            map.insert(
                "manual_test".to_string(),
                ServiceConfig {
                    cache_type: CacheType::L1,
                    ttl: Some(600),
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 100,
                        ..Default::default()
                    }),
                    l2: None,
                    two_level: None,
                },
            );
            map
        },
    };

    assert_eq!(config.services.get("manual_test").unwrap().ttl, Some(600));
}

/// 测试TTL验证
///
/// 验证当L1 TTL > L2 TTL时，配置验证应失败
#[test]
fn test_config_validation_ttl() {
    // 1. 正常情况：L1 TTL <= L2 TTL
    let config_ok = Config {
        config_version: Some(1),
        global: oxcache::config::GlobalConfig {
            default_ttl: 60,
            health_check_interval: 10,
            serialization: oxcache::config::SerializationType::Json,
            enable_metrics: false,
        },
        services: {
            let mut map = HashMap::new();
            map.insert(
                "ok_service".to_string(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(60), // L1 TTL
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 100,
                        cleanup_interval_secs: 0, // 禁用清理以专注测试TTL
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        default_ttl: Some(100), // L2 TTL > L1 TTL
                        ..Default::default()
                    }),
                    two_level: None,
                },
            );
            map
        },
    };
    assert!(config_ok.validate().is_ok());

    // 2. 异常情况：L1 TTL > L2 TTL
    let config_fail = Config {
        config_version: Some(1),
        global: oxcache::config::GlobalConfig {
            default_ttl: 60,
            health_check_interval: 10,
            serialization: oxcache::config::SerializationType::Json,
            enable_metrics: false,
        },
        services: {
            let mut map = HashMap::new();
            map.insert(
                "fail_service".to_string(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(200), // L1 TTL
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 100,
                        cleanup_interval_secs: 0, // 禁用清理以专注测试TTL
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        default_ttl: Some(100), // L2 TTL < L1 TTL
                        ..Default::default()
                    }),
                    two_level: None,
                },
            );
            map
        },
    };
    assert!(config_fail.validate().is_err());
}

/// 测试无效的Redis模式解析
///
/// 验证当配置文件中包含无效的Redis模式时，解析应该失败
#[test]
fn test_invalid_redis_mode_parsing() {
    let config_str = r#"
        [global]
        default_ttl = 3600
        health_check_interval = 5
        serialization = "json"
        enable_metrics = true
        
        [services.test_service]
        cache_type = "twolevel"
        
        [services.test_service.l1]
        max_capacity = 10000

        [services.test_service.l2]
        mode = "invalid_mode"
        connection_string = "redis://127.0.0.1:6379"
        connection_timeout_ms = 1000
        command_timeout_ms = 1000
        enable_tls = false
    "#;

    // 解析应该失败，因为mode字段包含无效的Redis模式
    let result: Result<Config, _> = toml::from_str(config_str);
    assert!(result.is_err(), "应该无法解析无效的Redis模式");

    // 验证错误信息包含模式相关的提示
    if let Err(e) = result {
        let error_msg = e.to_string();
        assert!(
            error_msg.contains("mode")
                || error_msg.contains("invalid")
                || error_msg.contains("Redis"),
            "错误信息应该包含模式相关的提示: {}",
            error_msg
        );
    }
}
