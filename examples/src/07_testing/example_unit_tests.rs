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
    cache.set(&"user:1".to_string(), &user).await?;
    let retrieved = cache.get(&"user:1".to_string()).await?;

    assert!(retrieved.is_some(), "应该获取到用户");
    assert_eq!(retrieved.unwrap().name, "测试用户", "用户名称应该匹配");
    println!("   ✓ 基本操作测试通过");

    Ok(())
}

// 测试删除操作
async fn test_delete_operation() -> Result<(), Box<dyn std::error::Error>> {
    println!("   测试删除操作...");

    let cache: Cache<String, String> = Cache::new().await?;

    cache.set(&"delete:key".to_string(), &"删除测试".to_string()).await?;
    let retrieved = cache.get(&"delete:key".to_string()).await?;
    assert!(retrieved.is_some(), "应该获取到数据");

    cache.delete(&"delete:key".to_string()).await?;
    let retrieved = cache.get(&"delete:key".to_string()).await?;
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
        cache.set(&format!("key:{}", i), &format!("value:{}", i)).await?;
    }

    // 验证数据存在
    let retrieved = cache.get(&"key:5".to_string()).await?;
    assert!(retrieved.is_some(), "应该获取到数据");

    // 清空缓存
    cache.clear().await?;

    // 验证数据已清空
    let retrieved = cache.get(&"key:5".to_string()).await?;
    assert!(retrieved.is_none(), "应该没有数据");
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
            cache.set(&format!("concurrent:{}", i), &format!("value:{}", i)).await?;
            Ok::<(), oxcache::error::CacheError>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    // 验证所有数据都写入成功
    let retrieved = cache.get(&"concurrent:50".to_string()).await?;
    assert!(retrieved.is_some(), "应该获取到数据");
    println!("   ✓ 并发操作测试通过");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 单元测试示例 ===\n");

    println!("运行测试套件...\n");

    // 运行测试
    test_basic_operations().await?;
    test_delete_operation().await?;
    test_clear_cache().await?;
    test_concurrent_operations().await?;

    println!();
    println!("=== 所有单元测试通过 ===");
    Ok(())
}