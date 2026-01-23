//! 综合使用示例 - 展示 Oxcache 的完整功能
//!
//! 本示例演示了 Oxcache 的各种功能，包括：
//! - 创建不同类型的缓存
//! - 基本 CRUD 操作
//! - TTL 控制
//! - 统计信息获取
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_comprehensive_usage
//! ```

use std::sync::Arc;
use tokio::time::{sleep, Duration};
use oxcache::Cache;
use oxcache::error::Result;

// 简单的用户结构体
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
    age: u32,
}

// 商品结构体
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Product {
    id: u64,
    name: String,
    price: f64,
    description: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Oxcache 综合使用示例 ===\n");

    // 1. 创建内存缓存 (L1 only)
    println!("1. 创建内存缓存 (L1)");
    let memory_cache: Cache<String, User> = Cache::new().await?;
    println!("   ✓ 内存缓存创建成功\n");

    // 2. 创建 Redis 缓存 (L2 only)
    println!("2. 创建 Redis 缓存 (L2)");
    let redis_cache: Cache<String, User> = Cache::redis("redis://127.0.0.1:6379").await?;
    println!("   ✓ Redis 缓存创建成功\n");

    // 3. 创建分层缓存 (L1 + L2)
    println!("3. 创建分层缓存 (L1 + L2)");
    let tiered_cache: Cache<String, User> = Cache::tiered(10000, "redis://127.0.0.1:6379").await?;
    println!("   ✓ 分层缓存创建成功\n");

    // 4. 基本 CRUD 操作
    println!("4. 基本 CRUD 操作演示");
    let cache: Cache<String, User> = Cache::new().await?;

    let user = User {
        id: 1,
        name: "张三".to_string(),
        email: "zhangsan@example.com".to_string(),
        age: 28,
    };

    // Create
    println!("   添加用户...");
    cache.set(&"user:1".to_string(), &user).await?;
    println!("   ✓ 用户添加成功: {:?}", user.name);

    // Read
    println!("   获取用户...");
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(u) => println!("   ✓ 用户获取成功: {:?}", u.name),
        None => println!("   ✗ 用户未找到"),
    }

    // Update
    println!("   更新用户...");
    let updated_user = User {
        id: 1,
        name: "张三丰".to_string(),
        email: "zhangsanfeng@example.com".to_string(),
        age: 30,
    };
    cache.set(&"user:1".to_string(), &updated_user).await?;
    println!("   ✓ 用户更新成功");

    // Read updated
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(u) => println!("   ✓ 更新后用户: {:?}", u.name),
        None => println!("   ✗ 用户未找到"),
    }

    // Delete
    println!("   删除用户...");
    cache.delete(&"user:1".to_string()).await?;
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(_) => println!("   ✗ 用户删除失败"),
        None => println!("   ✓ 用户删除成功"),
    }

    println!();

    // 5. 批量操作
    println!("5. 批量操作演示");
    let cache: Cache<String, Product> = Cache::new().await?;

    println!("   批量添加商品...");
    let products = vec![
        Product {
            id: 1,
            name: "笔记本电脑".to_string(),
            price: 5999.99,
            description: "高性能轻薄本".to_string(),
        },
        Product {
            id: 2,
            name: "智能手机".to_string(),
            price: 3999.99,
            description: "旗舰手机".to_string(),
        },
        Product {
            id: 3,
            name: "平板电脑".to_string(),
            price: 2999.99,
            description: "大屏平板".to_string(),
        },
    ];

    for product in &products {
        cache.set(&format!("product:{}", product.id), product).await?;
    }
    println!("   ✓ 批量添加成功 ({} 个商品)", products.len());

    // 批量获取
    println!("   批量获取商品...");
    let mut fetched = Vec::new();
    for i in 1..=3 {
        if let Some(product) = cache.get(&format!("product:{}", i)).await? {
            fetched.push(product);
        }
    }
    println!("   ✓ 批量获取成功 ({} 个商品)", fetched.len());

    // 批量删除
    println!("   批量删除商品...");
    for i in 1..=3 {
        cache.delete(&format!("product:{}", i)).await?;
    }
    println!("   ✓ 批量删除成功\n");

    // 6. TTL 控制
    println!("6. TTL 控制演示");
    let cache: Cache<String, String> = Cache::new().await?;

    println!("   添加 3 秒过期的数据...");
    cache.set_with_ttl(&"temp:1".to_string(), &"短期数据".to_string(), Some(Duration::from_secs(3))).await?;

    println!("   立即获取: {:?}", cache.get(&"temp:1".to_string()).await?);
    println!("   等待 4 秒后获取...");
    sleep(Duration::from_secs(4)).await;
    println!("   4 秒后获取: {:?}\n", cache.get(&"temp:1".to_string()).await?);

    // 7. 性能测试
    println!("7. 性能测试");
    let cache: Arc<Cache<String, i32>> = Arc::new(Cache::new().await?);

    // 并发写入测试
    println!("   并发写入 10000 条数据...");
    let start = std::time::Instant::now();
    let mut handles = Vec::new();
    for i in 0..100 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for j in 0..100 {
                cache.set(&format!("key:{}:{}", i, j), &(i * 100 + j)).await.unwrap();
            }
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await.unwrap();
    }
    let write_time = start.elapsed();
    println!("   ✓ 写入耗时: {:?}", write_time);

    // 并发读取测试
    println!("   并发读取 10000 条数据...");
    let start = std::time::Instant::now();
    let mut handles = Vec::new();
    for i in 0..100 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for j in 0..100 {
                cache.get(&format!("key:{}:{}", i, j)).await.unwrap();
            }
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await.unwrap();
    }
    let read_time = start.elapsed();
    println!("   ✓ 读取耗时: {:?}", read_time);
    println!();

    // 8. 统计信息
    println!("8. 统计信息");
    if let Ok(stats) = cache.stats().await {
        println!("   缓存统计:");
        for (key, value) in stats {
            println!("     - {}: {}", key, value);
        }
    }
    println!();

    println!("=== 示例运行完成 ===");
    Ok(())
}
