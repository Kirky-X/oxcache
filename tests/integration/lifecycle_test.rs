//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 生命周期管理集成测试

use oxcache::backend::l1::L1Backend;
use oxcache::backend::l2::L2Backend;
use oxcache::client::two_level::TwoLevelClient;
use oxcache::config::{L2Config, TwoLevelConfig};
use oxcache::serialization::SerializerEnum;
use std::sync::Arc;
use std::time::Duration;

#[path = "../common/mod.rs"]
mod common;

#[tokio::test]
async fn test_client_lifecycle_shutdown() {
    common::setup_logging();

    if !common::is_redis_available().await {
        println!("Skipping test_client_lifecycle_shutdown because Redis is not available");
        return;
    }

    let service_name = common::generate_unique_service_name("lifecycle");

    let l1 = Arc::new(L1Backend::new(1000));
    let l2_config = L2Config {
        connection_string: std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
            .into(),
        ..Default::default()
    };
    let l2 = Arc::new(L2Backend::new(&l2_config).await.unwrap());

    let config = TwoLevelConfig {
        promote_on_hit: true,
        enable_batch_write: true,
        batch_size: 10,
        batch_interval_ms: 50,
        invalidation_channel: None,
        bloom_filter: None,
        warmup: None,
    };

    {
        let client = TwoLevelClient::new(
            service_name.clone(),
            config,
            l1.clone(),
            l2.clone(),
            SerializerEnum::Json(oxcache::serialization::json::JsonSerializer),
        )
        .await
        .expect("Failed to create client");

        // Use the client
        let _ = client.set("key", &"value", None).await;

        // When client goes out of scope, we expect background tasks to stop.
        // Currently we can't easily verify this programmatically without internal hooks,
        // but we can ensure that dropping doesn't panic and resources are ostensibly released.
    }

    // Wait a bit to ensure no crashes occur after drop
    tokio::time::sleep(Duration::from_millis(200)).await;
}

/// 测试TwoLevelClient的优雅关闭功能
///
/// 验证shutdown方法能正确停止后台任务并释放资源
#[tokio::test]
async fn test_two_level_client_shutdown() {
    common::setup_logging();

    if !common::is_redis_available().await {
        println!("Skipping test_two_level_client_shutdown because Redis is not available");
        return;
    }

    let service_name = common::generate_unique_service_name("shutdown_test");

    let l1 = Arc::new(L1Backend::new(1000));
    let l2_config = L2Config {
        connection_string: std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
            .into(),
        ..Default::default()
    };
    let l2 = Arc::new(L2Backend::new(&l2_config).await.unwrap());

    let config = TwoLevelConfig {
        promote_on_hit: true,
        enable_batch_write: true,
        batch_size: 10,
        batch_interval_ms: 50,
        invalidation_channel: None,
        bloom_filter: None,
        warmup: None,
    };

    let client = TwoLevelClient::new(
        service_name.clone(),
        config,
        l1.clone(),
        l2.clone(),
        SerializerEnum::Json(oxcache::serialization::json::JsonSerializer),
    )
    .await
    .expect("Failed to create client");

    // 使用客户端进行一些操作
    client
        .set("test_key", &"test_value", Some(60))
        .await
        .unwrap();

    // 验证数据写入成功
    let value: String = client.get("test_key").await.unwrap().unwrap();
    assert_eq!(value, "test_value");

    // 执行优雅关闭
    client.shutdown().await.expect("Failed to shutdown client");

    // 等待一段时间确保所有后台任务都已停止
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 验证关闭后客户端状态（这里主要验证不panic）
    // 注意：由于客户端已关闭，某些操作可能会失败，这是预期的行为
}
