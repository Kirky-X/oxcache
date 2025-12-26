//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 故障恢复集成测试

use crate::common::{generate_unique_service_name, setup_cache};
use oxcache::config::{
    CacheType, Config, GlobalConfig, L1Config, L2Config, RedisMode, SerializationType,
    ServiceConfig, TwoLevelConfig,
};
use oxcache::CacheExt;
use std::collections::HashMap;
use std::time::Duration;
use secrecy::SecretBox;

#[path = "../common/mod.rs"]
mod common;

/// 测试降级逻辑
///
/// 验证当L2缓存不可用时，系统能否正确降级并继续工作
#[tokio::test]
async fn test_degradation_logic() {
    // 此测试配置无效的Redis地址以强制降级
    let service_name = generate_unique_service_name("recovery_test");

    let config = Config {
        global: GlobalConfig {
            default_ttl: 60,
            health_check_interval: 1, // 快速检查
            serialization: SerializationType::Json,
            enable_metrics: true,
        },
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(60),
                    serialization: None,
                    l1: Some(L1Config { max_capacity: 100 }),
                    l2: Some(L2Config {
                        mode: RedisMode::Standalone,
                        // 无效端口以强制连接失败
                        connection_string: "redis://127.0.0.1:9999".to_string().into(),
                        connection_timeout_ms: 100,
                        command_timeout_ms: 100,
                        password: None,
                        enable_tls: false,
                        sentinel: None,
                        cluster: None,
                        default_ttl: None,
                    }),
                    two_level: Some(TwoLevelConfig {
                        promote_on_hit: true,
                        enable_batch_write: false,
                        batch_size: 10,
                        batch_interval_ms: 100,
                        invalidation_channel: None,
                    }),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).expect("未找到客户端");

    // 等待健康检查标记为降级
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 写入应该成功（回退到WAL/L1）
    let result = client.set("degraded_key", &"value", Some(60)).await;
    assert!(result.is_ok(), "在降级模式下写入应通过WAL成功");

    // 读取L1应该工作
    let val: Option<String> = client.get("degraded_key").await.unwrap();
    assert_eq!(val, Some("value".to_string()));

    // 清理WAL文件
    let _ = std::fs::remove_file(format!("{}_wal.db", service_name));
}
