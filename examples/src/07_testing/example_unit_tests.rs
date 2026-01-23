//! 单元测试示例
//!
//! 本示例展示如何在 Oxcache 中编写单元测试。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_unit_tests
//!

use std::sync::Arc;
use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct TestUser {
    id: u64,
    name: String,
}

// 测试基本操作
async fn test_basic_operations() -> Result<(), Box<dyn std::error::Error>> {
    println!("   测试基本操作...");

    let cache: Cache<String, TestUser> = Cache::new().await?;

    // 测试设置和获取
    let user = TestUser {
        id: 1,
        name: "测试用户".to_string(),
    };
    cache.set("user:1", &user, None).await?;
    let retrieved = cache.get("user:1").await?;

    assert!(retrieved.is_some(), "应该获取到用户");
    assert_eq!(retrieved.unwrap().name, "测试用户", "用户名称应该匹配");
    println!("   ✓ 基本操作测试通过");

    Ok(())
}

// 测试 TTL 过期
async fn test_ttl_expiration() -> Result<(), Box<dyn std::error::Error>> {
    println!("   测试 TTL 过期...");

    let cache: Cache<String, String> = Cache::new().await?;

    // 设置 1 秒过期的数据
    cache.set("temp:data", "临时数据", Some(1)).await?;

    // 立即获取应该成功
    let retrieved = cache.get("temp:data").await?;
    assert!(retrieved.is_some(), "应该获取到临时数据");

    // 等待过期
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // 过期后应该获取不到
    let retrieved = cache.get("temp:data").await?;
    assert!(retrieved.is_none(), "数据应该已过期");
    println!("   ✓ TTL 过期测试通过");

    Ok(())
}

// 测试删除操作
async fn test_delete_operation() -> Result<(), Box<dyn std::error::Error>> {
    println!("   测试删除操作...");

    let cache: Cache<String, String> = Cache::new().await?;

    cache.set("delete:key", "删除测试", None).await?;
    let retrieved = cache.get("delete:key").await?;
    assert!(retrieved.is_some(), "应该获取到数据");

    cache.delete("delete:key").await?;
    let retrieved = cache.get("delete:key").await?;
    assert!(retrieved.is_none(), "数据应该已被删除");
    println!("   ✓ 删除操作测试通过");

    Ok(())
}

// 测试清空缓存
async fn test_clear_cache() -> Result<(), Box<dyn std::error::Error>> {
    println!("   测试清空缓存...");

    let cache: Cache<String, String> = Cache::new().await?;

    // 添加多个数据
    for i in 0..10 {
        cache.set(&format!("key:{}", i), &format!("value:{}", i), None).await?;
    }

    // 验证数据存在
    let count = cache.iter().await?.len();
    assert_eq!(count, 10, "应该有 10 条数据");

    // 清空缓存
    cache.clear().await?;

    // 验证数据已清空
    let count = cache.iter().await?.len();
    assert_eq!(count, 0, "应该没有数据");
    println!("   ✓ 清空缓存测试通过");

    Ok(())
}

// 测试并发操作
async fn test_concurrent_operations() -> Result<(), Box<dyn std::error::Error>> {
    println!("   测试并发操作...");

    let cache: Arc<Cache<String, String>> = Arc::new(Cache::new().await?);
    let mut handles = Vec::new();

    // 并发写入
    for i in 0..100 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            cache.set(&format!("concurrent:{}", i), &format!("value:{}", i), None).await?;
            Ok::<(), Box<dyn std::error::Error>>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    // 验证所有数据都写入成功
    let count = cache.iter().await?.len();
    assert_eq!(count, 100, "应该有 100 条数据");
    println!("   ✓ 并发操作测试通过");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 单元测试示例 ===\n");

    println!("运行测试套件...\n");

    // 运行测试
    test_basic_operations().await?;
    test_ttl_expiration().await?;
    test_delete_operation().await?;
    test_clear_cache().await?;
    test_concurrent_operations().await?;

    println!();
    println!("=== 所有单元测试通过 ===");
    Ok(())
}
