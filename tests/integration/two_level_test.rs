//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 双层缓存集成测试

use crate::common::{generate_unique_service_name, is_redis_available, setup_cache};
use oxcache::config::{
    CacheType, Config, GlobalConfig, L1Config, L2Config, RedisMode, SerializationType,
    ServiceConfig, TwoLevelConfig,
};
use oxcache::CacheExt;
use std::collections::HashMap;
use secrecy::SecretBox;

#[path = "../common/mod.rs"]
mod common;

/// 测试双层缓存流程
///
/// 验证双层缓存系统的基本工作流程
#[tokio::test]
async fn test_two_level_cache_flow() {
    if !is_redis_available().await {
        println!("跳过test_two_level_cache_flow：Redis不可用");
        return;
    }

    let service_name = generate_unique_service_name("flow_test");

    let config = Config {
        global: GlobalConfig {
            default_ttl: 60,
            health_check_interval: 1,
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
                        connection_string: SecretBox::new("redis://127.0.0.1:6379".into()),
                        connection_timeout_ms: 500,
                        command_timeout_ms: 500,
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

    // 1. 写入数据
    let test_val = "value1".to_string();
    client.set("key1", &test_val, Some(60)).await.unwrap();

    // 2. 验证L1命中（立即读取）
    let val: String = client.get("key1").await.unwrap().unwrap();
    assert_eq!(val, "value1");

    // 3. 通过仅从L1删除来模拟L1未命中（未在公共API中暴露，
    // 因此我们依赖L2持久性和新的客户端实例，或者如果我们能模拟时间的话等待驱逐。
    // 对于此测试，我们相信L2已被写入。如果有原始访问权限，我们可以通过直接检查L2来验证，
    // 但这里我们正在测试公共API契约。）

    // 让我们删除并确保它从两个地方都消失了
    client.delete("key1").await.unwrap();
    let val: Option<String> = client.get("key1").await.unwrap();
    assert!(val.is_none());
}
