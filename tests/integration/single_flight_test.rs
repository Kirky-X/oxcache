//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 单飞模式集成测试

#[path = "../common/mod.rs"]
mod common;

use common::setup_logging;
use oxcache::backend::l1::L1Backend;
use oxcache::backend::l2::L2Backend;
use oxcache::client::two_level::TwoLevelClient;
use oxcache::config::{L2Config, TwoLevelConfig};
use oxcache::serialization::SerializerEnum;
use oxcache::CacheOps;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Barrier;

// 模拟并发请求
#[tokio::test]
async fn test_single_flight_deduplication() {
    setup_logging();

    if !common::is_redis_available().await {
        println!("Skipping test_single_flight_deduplication because Redis is not available");
        return;
    }

    let service_name = common::generate_unique_service_name("single_flight");

    // 配置L2，使用Redis
    // 优先使用环境变量 REDIS_URL，如果没有则使用本地无密码连接
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let l2_config = L2Config {
        connection_string: redis_url.into(),
        connection_timeout_ms: 10000, // Increase timeout to 10 seconds
        ..Default::default()
    };
    let l2 = Arc::new(
        L2Backend::new(&l2_config)
            .await
            .expect("Failed to create L2 backend"),
    );

    // 清理Redis中的测试key
    let _ = l2.delete("hot_key").await;

    // 配置L1
    let l1 = Arc::new(L1Backend::new(1000));

    // 创建客户端，启用promote_on_hit以触发Single-Flight逻辑
    let config = TwoLevelConfig {
        promote_on_hit: true,
        enable_batch_write: false,
        batch_size: 100,
        batch_interval_ms: 100,
        invalidation_channel: None,
        bloom_filter: None,
        warmup: None,
        max_key_length: Some(1024),
        max_value_size: Some(1024 * 1024),
    };

    let client = Arc::new(
        TwoLevelClient::new(
            service_name.clone(),
            config,
            l1.clone(),
            l2.clone(),
            SerializerEnum::Json(oxcache::serialization::json::JsonSerializer),
        )
        .await
        .expect("Failed to create client"),
    );

    // 预先在L2中设置一个值
    l2.set_with_version("hot_key", b"hot_value".to_vec(), None)
        .await
        .expect("Failed to set L2 value");

    // 模拟高并发请求
    let concurrency = 50;
    let barrier = Arc::new(Barrier::new(concurrency));
    let mut handles = vec![];

    for _ in 0..concurrency {
        let c = client.clone();
        let b = barrier.clone();
        handles.push(tokio::spawn(async move {
            b.wait().await;
            c.get_bytes("hot_key").await
        }));
    }

    let mut success_count = 0;
    for handle in handles {
        if let Ok(Ok(Some(val))) = handle.await {
            if val == b"hot_value" {
                success_count += 1;
            }
        }
    }

    assert_eq!(success_count, concurrency, "All requests should succeed");

    // 验证L1中已经有了值，等待异步promote完成
    tokio::time::sleep(Duration::from_millis(500)).await;
    let l1_val = l1.get_with_metadata("hot_key").await.unwrap();
    assert!(l1_val.is_some(), "L1 should be populated after L2 hit");
    assert_eq!(l1_val.unwrap().0, b"hot_value");

    // 清理
    let _ = l2.delete("hot_key").await;
    common::cleanup_service(&service_name).await;
}
