// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 综合集成测试 - 简化版本
//
// 测试新增功能：配置系统、智能策略、HTTP 缓存、TTL 控制等

#![allow(deprecated)]

// 使用新的统一配置 API
use oxcache::config::GlobalConfig;
use oxcache::smart_strategy::{HitRateCollector, SmartStrategyConfig, SmartStrategyManager};
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

    assert_eq!(config.global.default_ttl(), 600);
    assert!(config.services.contains_key("test_service"));

    let service = config.services.get("test_service").unwrap();
    assert_eq!(service.cache_type, CacheType::TwoLevel);
    assert_eq!(service.ttl, Some(3600));
}

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
    let collector = HitRateCollector::new(200);

    // 填充窗口
    for _ in 0..50 {
        collector.record_hit();
    }
    for _ in 0..50 {
        collector.record_miss();
    }

    assert_eq!(collector.hit_rate(), 0.5);
    assert_eq!(collector.recent_hit_rate(), 0.5);
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

#[tokio::test]
async fn test_comprehensive_integration() {
    // 综合测试依赖于 TwoLevelClient
    // 新 API 需要重新实现这些功能
    println!("注：综合集成测试需要新的 API 实现");
    println!("跳过完整的功能测试");
}
