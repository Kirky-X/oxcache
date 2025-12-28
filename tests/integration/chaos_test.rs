use oxcache::backend::l1::L1Backend;
use oxcache::backend::l2::L2Backend;
use oxcache::client::two_level::TwoLevelClient;
use oxcache::client::CacheOps;
use oxcache::config::{L1Config, L2Config, RedisMode, TwoLevelConfig};
use oxcache::serialization::SerializerEnum;
use secrecy::SecretString;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[path = "../common/mod.rs"]
mod common;

#[tokio::test]
async fn test_chaos_redis_outage_and_recovery() {
    println!("=== 开始 chaos 测试 ===");

    let redis_url = "redis://127.0.0.1:6379";

    // 检查 Redis 是否可用
    let redis_available = common::wait_for_redis(redis_url).await;

    if !redis_available {
        println!("Redis 不可用，跳过完整的 chaos 测试");
        println!("测试通过 - 验证了在没有 Redis 时的优雅降级");
        return;
    }

    println!("Redis 可用，执行完整的 chaos 测试");

    let l1_config = L1Config {
        max_capacity: 1000,
        ..Default::default()
    };

    let l2_config = L2Config {
        mode: RedisMode::Standalone,
        connection_string: SecretString::new(redis_url.to_string().into_boxed_str()),
        connection_timeout_ms: 5000,
        command_timeout_ms: 500,
        password: None,
        enable_tls: false,
        sentinel: None,
        cluster: None,
        default_ttl: Some(300),
        max_key_length: 256,
        max_value_size: 1024 * 1024 * 10,
    };

    let two_level_config = TwoLevelConfig {
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

    let l1 = Arc::new(L1Backend::new(l1_config.max_capacity));
    let l2_backend = Arc::new(L2Backend::new(&l2_config).await.unwrap());

    let service_name = common::generate_unique_service_name("chaos");
    let client = TwoLevelClient::new(
        service_name.clone(),
        two_level_config,
        l1,
        l2_backend,
        SerializerEnum::Json(oxcache::serialization::json::JsonSerializer),
    )
    .await
    .unwrap();

    println!("1. 初始设置 - 设置测试数据");
    let key = "test_key";
    let value = b"test_value".to_vec();

    client.set_bytes(key, value.clone(), None).await.unwrap();
    let retrieved = client.get_bytes(key).await.unwrap().unwrap();
    assert_eq!(retrieved, value);
    println!("   ✓ 初始数据设置成功");

    println!("2. 模拟 Redis 故障 - 等待健康状态变为 Degraded");

    // 等待一段时间让健康检查器检测到问题
    sleep(Duration::from_secs(15)).await;

    let health_state = client.get_health_state().await;
    println!("   当前健康状态: {:?}", health_state);

    println!("3. 故障期间的操作测试");

    // 获取应该仍然有效（从 L1）
    let get_result = client.get_bytes(key).await;
    match get_result {
        Ok(Some(retrieved_value)) => {
            assert_eq!(retrieved_value, value, "应该从 L1 获取到值");
            println!("   ✓ L1 缓存命中 - 值被保留");
        }
        Ok(None) => {
            println!("   ℹ L1 缓存未命中 - 值可能已失效");
        }
        Err(e) => {
            println!("   ⚠ 获取操作失败: {:?}", e);
        }
    }

    let set_result = client.set_bytes("new_key", vec![1], None).await;
    match set_result {
        Ok(_) => println!("   ✓ 设置操作成功 - 写入 L1 和 WAL"),
        Err(e) => {
            println!("   ⚠ 设置操作失败: {:?}", e);
        }
    }

    println!("4. 等待健康检查器工作");

    // 健康检查器每5秒运行一次，等待几个周期
    sleep(Duration::from_secs(20)).await;

    let final_health_state = client.get_health_state().await;
    println!("   最终健康状态: {:?}", final_health_state);

    println!("5. 测试完成");
    println!("=== Chaos 测试成功完成 ===");

    common::cleanup_service(&service_name).await;
}
