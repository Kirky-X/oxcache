//! 功能性 UAT 测试示例
//!
//! 本示例展示用户验收测试 (UAT) 场景。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_functional_uat
//!

use std::sync::Arc;
use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct UatUser {
    id: u64,
    username: String,
    email: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct CartItem {
    product_id: u64,
    name: String,
    quantity: u32,
    price: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct ShoppingCart {
    user_id: u64,
    items: Vec<CartItem>,
    total: f64,
}

// 场景 1: 用户登录和会话管理
async fn test_user_login_and_session() -> Result<(), Box<dyn std::error::Error>> {
    println!("   场景 1: 用户登录和会话管理...");

    let cache: Cache<String, UatUser> = Cache::new().await?;

    // 用户登录
    let user = UatUser {
        id: 1,
        username: "test_user".to_string(),
        email: "test@example.com".to_string(),
    };
    cache.set("user:1", &user, Some(3600)).await?;

    // 验证会话
    let retrieved = cache.get("user:1").await?;
    assert!(retrieved.is_some(), "用户会话应该存在");
    assert_eq!(retrieved.unwrap().id, 1, "用户 ID 应该匹配");

    // 用户登出
    cache.delete("user:1").await?;
    let retrieved = cache.get("user:1").await?;
    assert!(retrieved.is_none(), "用户会话应该已删除");

    println!("   ✓ 用户登录和会话管理测试通过");
    Ok(())
}

// 场景 2: 购物车操作
async fn test_shopping_cart_operations() -> Result<(), Box<dyn std::error::Error>> {
    println!("   场景 2: 购物车操作...");

    let cache: Cache<String, ShoppingCart> = Cache::new().await?;

    // 添加购物车项
    let cart = ShoppingCart {
        user_id: 1,
        items: vec![
            CartItem {
                product_id: 1,
                name: "产品 A".to_string(),
                quantity: 2,
                price: 99.99,
            },
            CartItem {
                product_id: 2,
                name: "产品 B".to_string(),
                quantity: 1,
                price: 199.99,
            },
        ],
        total: 399.97,
    };
    cache.set("cart:1", &cart, Some(1800)).await?;

    // 验证购物车
    let retrieved = cache.get("cart:1").await?;
    assert!(retrieved.is_some(), "购物车应该存在");
    assert_eq!(retrieved.unwrap().items.len(), 2, "购物车应该有 2 个商品");

    // 更新数量
    let mut updated_cart = cart.clone();
    updated_cart.items[0].quantity = 3;
    updated_cart.total = 299.97 + 199.99;
    cache.set("cart:1", &updated_cart, Some(1800)).await?;

    // 验证更新
    let retrieved = cache.get("cart:1").await?;
    assert_eq!(retrieved.unwrap().items[0].quantity, 3, "商品数量应该已更新");

    // 清空购物车
    cache.delete("cart:1").await?;
    let retrieved = cache.get("cart:1").await?;
    assert!(retrieved.is_none(), "购物车应该已删除");

    println!("   ✓ 购物车操作测试通过");
    Ok(())
}

// 场景 3: 性能测试
async fn test_performance_requirements() -> Result<(), Box<dyn std::error::Error>> {
    println!("   场景 3: 性能要求验证...");

    let cache: Arc<Cache<String, String>> = Arc::new(Cache::new().await?);

    // 性能测试: 1000 次读写
    let iterations = 1000;
    let start = std::time::Instant::new();

    // 并发写入
    let mut handles = Vec::new();
    for i in 0..iterations {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            cache
                .set(&format!("perf:{}", i), &format!("value:{}", i), None)
                .await
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await?;
    }

    // 并发读取
    let mut handles = Vec::new();
    for i in 0..iterations {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            cache.get(&format!("perf:{}", i)).await
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await?;
    }

    let elapsed = start.elapsed();
    let throughput = iterations as f64 * 2.0 / elapsed.as_secs_f64();

    println!("     执行 {} 次读写，耗时: {:?}", iterations * 2, elapsed);
    println!("     吞吐量: {:.2} ops/sec", throughput);

    // 性能要求: 吞吐量应该 > 10000 ops/sec
    assert!(throughput > 10000.0, "吞吐量应该 > 10000 ops/sec");

    // 清理
    cache.clear().await?;
    println!("   ✓ 性能要求验证测试通过");

    Ok(())
}

// 场景 4: TTL 过期测试
async fn test_ttl_requirements() -> Result<(), Box<dyn std::error::Error>> {
    println!("   场景 4: TTL 过期要求验证...");

    let cache: Cache<String, String> = Cache::new().await?;

    // 设置 2 秒过期的数据
    cache.set("ttl:test", "测试数据", Some(2)).await?;

    // 立即获取应该成功
    let retrieved = cache.get("ttl:test").await?;
    assert!(retrieved.is_some(), "数据应该存在");

    // 等待 3 秒
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 数据应该已过期
    let retrieved = cache.get("ttl:test").await?;
    assert!(retrieved.is_none(), "数据应该已过期");

    println!("   ✓ TTL 过期要求验证测试通过");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 功能性 UAT 测试示例 ===\n");

    println!("运行 UAT 测试...\n");

    test_user_login_and_session().await?;
    test_shopping_cart_operations().await?;
    test_performance_requirements().await?;
    test_ttl_requirements().await?;

    println!();
    println!("=== 所有 UAT 测试通过 ===");
    Ok(())
}
