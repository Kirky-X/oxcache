//! 集成测试示例
//!
//! 本示例展示如何进行 Oxcache 的集成测试。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_integration_tests
//!

use std::sync::Arc;
use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct IntegrationTestUser {
    id: u64,
    username: String,
    email: String,
    created_at: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct IntegrationTestProduct {
    id: u64,
    name: String,
    price: f64,
    category: String,
}

// 测试场景 1: 用户会话管理
async fn test_user_session_management() -> Result<(), Box<dyn std::error::Error>> {
    println!("   测试场景 1: 用户会话管理...");

    let cache: Cache<String, IntegrationTestUser> = Cache::new().await?;

    // 模拟用户登录
    let user = IntegrationTestUser {
        id: 1,
        username: "test_user".to_string(),
        email: "test@example.com".to_string(),
        created_at: chrono::Local::now().to_rfc3339(),
    };

    // 存储会话
    cache.set("session:user:1", &user, Some(3600)).await?;

    // 验证会话存在
    let retrieved = cache.get("session:user:1").await?;
    assert!(retrieved.is_some(), "会话应该存在");
    assert_eq!(retrieved.unwrap().username, "test_user", "用户名应该匹配");

    // 模拟会话更新
    let updated_user = IntegrationTestUser {
        id: 1,
        username: "test_user".to_string(),
        email: "updated@example.com".to_string(),
        created_at: user.created_at.clone(),
    };
    cache.set("session:user:1", &updated_user, Some(3600)).await?;

    // 验证更新
    let retrieved = cache.get("session:user:1").await?;
    assert_eq!(retrieved.unwrap().email, "updated@example.com", "邮箱应该已更新");

    // 清理
    cache.delete("session:user:1").await?;
    println!("   ✓ 用户会话管理测试通过");

    Ok(())
}

// 测试场景 2: 产品目录缓存
async fn test_product_catalog_cache() -> Result<(), Box<dyn std::error::Error>> {
    println!("   测试场景 2: 产品目录缓存...");

    let cache: Cache<String, IntegrationTestProduct> = Cache::new().await?;

    // 模拟产品数据
    let products = vec![
        IntegrationTestProduct {
            id: 1,
            name: "产品 A".to_string(),
            price: 99.99,
            category: "电子产品".to_string(),
        },
        IntegrationTestProduct {
            id: 2,
            name: "产品 B".to_string(),
            price: 199.99,
            category: "服装".to_string(),
        },
        IntegrationTestProduct {
            id: 3,
            name: "产品 C".to_string(),
            price: 299.99,
            category: "家居".to_string(),
        },
    ];

    // 缓存产品
    for product in &products {
        cache
            .set(&format!("product:{}", product.id), product, Some(7200))
            .await?;
    }

    // 验证所有产品都缓存成功
    for product in &products {
        let retrieved = cache.get(&format!("product:{}", product.id)).await?;
        assert!(retrieved.is_some(), "产品应该存在");
        assert_eq!(retrieved.unwrap().name, product.name, "产品名称应该匹配");
    }

    // 清理
    cache.clear().await?;
    println!("   ✓ 产品目录缓存测试通过");

    Ok(())
}

// 测试场景 3: 并发缓存操作
async fn test_concurrent_cache_operations() -> Result<(), Box<dyn std::error::Error>> {
    println!("   测试场景 3: 并发缓存操作...");

    let cache: Arc<Cache<String, String>> = Arc::new(Cache::new().await?);
    let mut handles = Vec::new();

    // 并发写入
    for i in 0..50 {
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

    // 并发读取
    let mut handles = Vec::new();
    for i in 0..50 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            cache.get(&format!("concurrent:{}", i)).await?;
            Ok::<(), Box<dyn std::error::Error>>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    // 验证数据完整性
    let count = cache.iter().await?.len();
    assert_eq!(count, 50, "应该有 50 条数据");

    // 清理
    cache.clear().await?;
    println!("   ✓ 并发缓存操作测试通过");

    Ok(())
}

// 测试场景 4: 缓存过期策略
async fn test_cache_expiration_policy() -> Result<(), Box<dyn std::error::Error>> {
    println!("   测试场景 4: 缓存过期策略...");

    let cache: Cache<String, String> = Cache::new().await?;

    // 设置不同过期时间的缓存
    cache.set("short:1", "1秒过期", Some(1)).await?;
    cache.set("medium:1", "3秒过期", Some(3)).await?;
    cache.set("long:1", "10秒过期", Some(10)).await?;

    // 验证所有缓存存在
    assert!(cache.get("short:1").await?.is_some());
    assert!(cache.get("medium:1").await?.is_some());
    assert!(cache.get("long:1").await?.is_some());

    // 等待短缓存过期
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // 验证短缓存已过期
    assert!(cache.get("short:1").await?.is_none(), "短缓存应该已过期");
    // 中长缓存应该还存在
    assert!(cache.get("medium:1").await?.is_some(), "中缓存应该还存在");
    assert!(cache.get("long:1").await?.is_some(), "长缓存应该还存在");

    // 清理
    cache.clear().await?;
    println!("   ✓ 缓存过期策略测试通过");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 集成测试示例 ===\n");

    println!("运行集成测试套件...\n");

    test_user_session_management().await?;
    test_product_catalog_cache().await?;
    test_concurrent_cache_operations().await?;
    test_cache_expiration_policy().await?;

    println!();
    println!("=== 所有集成测试通过 ===");
    Ok(())
}
