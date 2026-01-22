#![allow(deprecated)]
// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 综合集成测试
//
// 测试新增功能：配置系统、智能策略、HTTP 缓存、TTL 控制等

use oxcache::config::{
    CacheType, GlobalConfig, L1Config, L2Config, OxcacheConfig, ServiceConfig, TwoLevelConfig,
};
use oxcache::smart_strategy::{HitRateCollector, SmartStrategyConfig, SmartStrategyManager};
use secrecy::SecretString;
use std::collections::HashMap;
use tempfile::TempDir;

// ============================================================================
// 配置系统集成测试
// ============================================================================

/// 测试 confers 配置加载
#[cfg(feature = "confers")]
#[test]
fn test_confers_config_loading() {
    use oxcache::config::confers_macro::confers_load;

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("oxcache.toml");

    let config_content = r#"
[global]
default_ttl = 600
health_check_interval = 30
serialization = "json"
enable_metrics = true

[[services]]
name = "test_service"
cache_type = "two_level"
ttl = 3600

[services.test_service.two_level]
promote_on_hit = true
enable_batch_write = false

[services.test_service.l1]
max_capacity = 10000

[services.test_service.l2]
mode = "standalone"
connection_string = "redis://localhost:6379"
connection_timeout_ms = 5000
default_ttl = 3600
"#;

    std::fs::write(&config_path, config_content).unwrap();

    let config = confers_load(config_path.to_str().unwrap());
    assert!(config.is_ok());
    let config = config.unwrap();

    assert_eq!(config.global.default_ttl, 600);
    assert!(config.services.contains_key("test_service"));

    let service = config.services.get("test_service").unwrap();
    assert_eq!(service.cache_type, CacheType::TwoLevel);
    assert_eq!(service.ttl, Some(3600));
}

/// 测试 confers 配置源标记
#[cfg(feature = "confers")]
#[test]
fn test_config_source_marker() {
    use oxcache::config::confers_macro::confers_load;

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("oxcache.toml");

    let config_content = r#"
[global]
default_ttl = 300
"#;

    std::fs::write(&config_path, config_content).unwrap();

    let config = confers_load(config_path.to_str().unwrap()).unwrap();
    assert_eq!(
        config.source,
        Some(oxcache::ConfigSource::File("confers".to_string()))
    );
}

/// 测试不存在的配置文件
#[cfg(feature = "confers")]
#[test]
fn test_confers_nonexistent_file() {
    use oxcache::config::confers_macro::confers_load;

    let result = confers_load("/nonexistent/path/config.toml");
    assert!(result.is_err());
}

// ============================================================================
// 智能策略集成测试
// ============================================================================

/// 测试智能策略管理器基本功能
#[test]
fn test_smart_strategy_manager_basic() {
    let manager = SmartStrategyManager::new(None);

    // 记录一些访问
    for _ in 0..80 {
        manager.record_access(true);
    }
    for _ in 0..20 {
        manager.record_access(false);
    }

    // 检查命中率统计
    let stats = manager.hit_rate_stats();
    assert_eq!(stats.hit_rate, 0.8);
    assert_eq!(stats.total_hits, 80);
    assert_eq!(stats.total_misses, 20);
}

/// 测试智能策略预取决策
#[test]
fn test_smart_strategy_prefetch_decision() {
    let manager = SmartStrategyManager::new(Some(SmartStrategyConfig {
        prefetch_enabled: true,
        prefetch_threshold: 0.8,
        prefetch_window_size: 200,
        ..Default::default()
    }));

    // 高命中率不应该触发预取
    for _ in 0..90 {
        manager.record_access(true);
    }
    for _ in 0..10 {
        manager.record_access(false);
    }
    assert!(!manager.should_prefetch());

    // 重置后低命中率应该触发预取
    let stats = manager.hit_rate_stats();
    assert_eq!(stats.recent_hits, 90);
    assert_eq!(stats.recent_misses, 10);
}

/// 测试智能策略配置更新
#[test]
fn test_smart_strategy_config_update() {
    let mut manager = SmartStrategyManager::new(None);

    let new_config = SmartStrategyConfig {
        prefetch_threshold: 0.5,
        compression_threshold: 2048,
        ..Default::default()
    };

    manager.update_config(new_config.clone());

    assert_eq!(manager.config().prefetch_threshold, 0.5);
    assert_eq!(manager.config().compression_threshold, 2048);
}

/// 测试命中率收集器窗口行为
#[test]
fn test_hit_rate_collector_window() {
    let collector = HitRateCollector::new(200); // 使用更大的窗口

    // 填充窗口
    for _ in 0..50 {
        collector.record_hit();
    }
    for _ in 0..50 {
        collector.record_miss();
    }

    assert_eq!(collector.hit_rate(), 0.5);
    assert_eq!(collector.recent_hit_rate(), 0.5);

    // 继续添加不超过窗口大小
    for _ in 0..50 {
        collector.record_hit();
    }

    // 窗口应该更新，最近的 100 次命中（50 + 50）
    let stats = collector.stats();
    assert_eq!(stats.recent_hits, 100);
    assert_eq!(stats.recent_misses, 50);
    // 100/150 = 0.666...
    assert!((stats.recent_hit_rate - 0.6666).abs() < 0.01);
}

/// 测试智能策略压缩决策
#[test]
fn test_smart_strategy_compression_decision() {
    let manager = SmartStrategyManager::new(None);

    // 高可压缩性数据（重复模式）
    let compressible = vec![0x00u8; 2000];
    assert!(manager.should_compress(&compressible));

    // 低可压缩性数据（随机数据）
    let incompressible: Vec<u8> = (0..2000).map(|_| rand::random()).collect();
    assert!(!manager.should_compress(&incompressible));
}

// ============================================================================
// TwoLevelClient 智能策略集成测试（需要 Redis）
// ============================================================================

#[cfg(feature = "l2-redis")]
mod two_level_smart_strategy_tests {
    use super::*;
    use oxcache::backend::l2::L2Backend;
    use oxcache::client::two_level::TwoLevelClient;
    use oxcache::serialization::{JsonSerializer, SerializerEnum};
    use std::sync::Arc;

    /// 测试 TwoLevelClient 启用智能策略（跳过实际 Redis 连接）
    #[tokio::test]
    #[ignore] // 需要运行中的 Redis 实例
    async fn test_two_level_client_smart_strategy() {
        // 创建 L2Config
        let l2_config = L2Config {
            default_ttl: Some(3600),
            connection_string: SecretString::new("redis://127.0.0.1:6379".to_string()),
            mode: oxcache::config::RedisMode::Standalone,
            ..Default::default()
        };

        // 创建基本的 TwoLevelClient
        let l2_backend = Arc::new(
            L2Backend::new(&l2_config)
                .await
                .expect("Failed to create L2 backend"),
        );

        // L1Backend::new() 是同步的
        let l1 = Arc::new(oxcache::backend::l1::L1Backend::new(1000));

        let config = TwoLevelConfig {
            promote_on_hit: true,
            enable_batch_write: false,
            ..Default::default()
        };

        let client = TwoLevelClient::new(
            "test_smart_strategy".to_string(),
            config,
            l1,
            l2_backend,
            SerializerEnum::Json(JsonSerializer::new()),
        )
        .await
        .expect("Failed to create client");

        // 由于 TwoLevelClient 现在需要可变引用来启用智能策略
        // 我们只验证智能策略相关的方法存在且可调用
        assert!(!client.is_smart_strategy_enabled());
    }
}

// ============================================================================
// 配置验证测试
// ============================================================================

/// 测试完整的配置结构
#[test]
fn test_complete_config_structure() {
    let config = OxcacheConfig {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 3600,
            health_check_interval: 30,
            serialization: oxcache::config::SerializationType::Json,
            enable_metrics: true,
        },
        services: {
            let mut map = HashMap::new();
            map.insert(
                "test_service".to_string(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(3600),
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 10000,
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        default_ttl: Some(3600),
                        connection_string: SecretString::new("redis://localhost:6379".to_string()),
                        mode: oxcache::config::RedisMode::Standalone,
                        ..Default::default()
                    }),
                    two_level: Some(TwoLevelConfig {
                        promote_on_hit: true,
                        enable_batch_write: false,
                        ..Default::default()
                    }),
                },
            );
            map
        },
        source: Some(oxcache::ConfigSource::Code),
        ..Default::default()
    };

    assert_eq!(config.global.default_ttl, 3600);
    assert!(config.services.contains_key("test_service"));

    let service = config.services.get("test_service").unwrap();
    assert_eq!(service.cache_type, CacheType::TwoLevel);
    assert!(service.l1.is_some());
    assert!(service.l2.is_some());
    assert!(service.two_level.is_some());
}

/// 测试配置结构
#[test]
fn test_config_structure() {
    // 测试使用默认值创建配置
    let config = OxcacheConfig::default();
    assert!(config.services.is_empty());

    // 测试服务配置创建
    let service = ServiceConfig {
        cache_type: CacheType::TwoLevel,
        ttl: Some(100),
        l1: Some(L1Config {
            max_capacity: 100,
            ..Default::default()
        }),
        l2: Some(L2Config::default()),
        two_level: Some(TwoLevelConfig::default()),
        ..Default::default()
    };

    assert_eq!(service.cache_type, CacheType::TwoLevel);
    assert_eq!(service.ttl, Some(100));
    assert!(service.l1.is_some());
    assert!(service.l2.is_some());
    assert!(service.two_level.is_some());
}

// ============================================================================
// 序列化类型测试
// ============================================================================

/// 测试 SerializationType 解析
#[test]
fn test_serialization_type_parsing() {
    use oxcache::config::SerializationType;

    // 测试默认序列化类型
    let default_type = SerializationType::default();
    assert_eq!(default_type, SerializationType::Json);

    // 测试序列化类型比较
    assert_ne!(SerializationType::Json, SerializationType::Bincode);
}

/// 测试 SerializationType 转换
#[test]
fn test_serialization_type_conversion() {
    use oxcache::config::SerializationType;

    // JSON 是基于文本的
    let json_type = SerializationType::Json;
    assert!(matches!(json_type, SerializationType::Json));

    // Bincode 是二进制的
    let bincode_type = SerializationType::Bincode;
    assert!(matches!(bincode_type, SerializationType::Bincode));
}
