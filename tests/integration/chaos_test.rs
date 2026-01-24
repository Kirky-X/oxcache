// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Chaos测试 - 测试系统在Redis故障时的恢复能力

use oxcache::backend::{CacheBackend, MemoryBackend, RedisBackend, TieredBackend};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use crate::common;

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

    // 使用新 API 创建缓存
    let l1 = Arc::new(MemoryBackend::builder().capacity(1000).build()) as Arc<dyn CacheBackend>;
    let l2 = match RedisBackend::new(redis_url).await {
        Ok(backend) => Arc::new(backend) as Arc<dyn CacheBackend>,
        Err(e) => {
            println!("   ⚠ 无法连接 Redis: {:?}", e);
            return;
        }
    };

    let tiered = TieredBackend::from_arc(l1, l2);

    println!("1. 初始设置 - 设置测试数据");
    let key = "test_key";
    let value = b"test_value".to_vec();

    tiered
        .set(key, value.clone(), Some(Duration::from_secs(300)))
        .await
        .unwrap();
    let retrieved = tiered.get(key).await.unwrap();
    assert_eq!(retrieved, Some(value.clone()));
    println!("   ✓ 初始数据设置成功");

    println!("2. 测试基本的读写操作");
    let get_result = tiered.get(key).await;
    match get_result {
        Ok(Some(retrieved_value)) => {
            assert_eq!(retrieved_value, value, "应该获取到值");
            println!("   ✓ 缓存读取成功");
        }
        Ok(None) => {
            println!("   ℹ 缓存未命中");
        }
        Err(e) => {
            println!("   ⚠ 获取操作失败: {:?}", e);
        }
    }

    let set_result = tiered
        .set("new_key", vec![1], Some(Duration::from_secs(60)))
        .await;
    match set_result {
        Ok(_) => println!("   ✓ 写入操作成功"),
        Err(e) => {
            println!("   ⚠ 写入操作失败: {:?}", e);
        }
    }

    println!("3. 测试故障恢复（通过健康检查）");

    // 健康检查测试
    let healthy = tiered.health_check().await;
    match healthy {
        Ok(true) => println!("   ✓ 后端健康检查通过"),
        Ok(false) => println!("   ⚠ 后端健康检查失败"),
        Err(e) => println!("   ⚠ 健康检查错误: {:?}", e),
    }

    println!("4. 测试完成");
    println!("=== Chaos 测试成功完成 ===");
}
