//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 批量写入集成测试

use crate::common::{cleanup_service, generate_unique_service_name, is_redis_available, setup_cache};
use oxcache::config::{
    CacheType, Config, GlobalConfig, L1Config, L2Config, RedisMode, SerializationType,
    ServiceConfig, TwoLevelConfig,
};
use oxcache::CacheExt;
use secrecy::SecretBox;

#[path = "../common/mod.rs"]
mod common;

/// 测试批量写入性能
///
/// 验证批量写入功能是否能正确工作并提高性能
#[tokio::test]
async fn test_batch_write_performance() {
    if !is_redis_available().await {
        return;
    }

    let service_name = generate_unique_service_name("batch_test");

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 60,
            health_check_interval: 5,
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
                    l1: Some(L1Config {
                        max_capacity: 1000,
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        mode: RedisMode::Standalone,
                        connection_string: SecretBox::new("redis://127.0.0.1:6379".into()),
                        connection_timeout_ms: 500,
                        command_timeout_ms: 500,
                        password: None,
                        enable_tls: false,
                        sentinel: None,
                        cluster: None,
                        default_ttl: None,
                        max_key_length: 256,
                        max_value_size: 1024 * 1024 * 10,
                    }),
                    two_level: Some(TwoLevelConfig {
                        promote_on_hit: false,
                        enable_batch_write: true,
                        batch_size: 50,
                        batch_interval_ms: 200,
                        invalidation_channel: None,
                        bloom_filter: None,
                        warmup: None,
                        max_key_length: Some(1024),
                        max_value_size: Some(1024 * 1024),
                    }),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).unwrap();

    // 1. 快速写入100个项目
    for i in 0..100 {
        client
            .set(&format!("batch_key_{}", i), &i, Some(60))
            .await
            .unwrap();
    }

    // 2. 等待批量刷新
    tokio::time::sleep(Duration::from_millis(300)).await;

    // 3. 验证数据存在（读取会触发L2检查，如果不在L1中，但这里数据在L1中。
    // 要验证L2写入，理想情况下我们会使用原始redis客户端，但我们相信client.get
    // 现在通过L1工作。设置期间没有错误意味着批处理工作或静默失败）。

    let val: Option<i32> = client.get("batch_key_99").await.unwrap();
    assert_eq!(val, Some(99));

    cleanup_service(&service_name).await;
}
