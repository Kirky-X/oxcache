//! Redis Cluster 模式示例
//!
//! 本示例演示如何使用 Oxcache 连接 Redis Cluster。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_cluster
//!

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Product {
    id: u64,
    name: String,
    price: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Redis Cluster 模式示例 ===\n");

    // 创建 Redis Cluster 缓存
    println!("创建 Redis Cluster 缓存连接...");
    let cache: Cache<String, Product> = Cache::redis("redis://127.0.0.1:6379").await?;
    println!("✓ Redis Cluster 连接成功\n");

    // 基本操作
    println!("1. 产品数据操作");
    let products = vec![
        Product {
            id: 1,
            name: "笔记本电脑".to_string(),
            price: 5999.99,
        },
        Product {
            id: 2,
            name: "智能手机".to_string(),
            price: 3999.99,
        },
        Product {
            id: 3,
            name: "平板电脑".to_string(),
            price: 2999.99,
        },
    ];

    // 添加产品
    println!("   添加产品...");
    for product in &products {
        cache
            .set(&format!("product:{}", product.id), product, Some(3600))
            .await?;
        println!("   ✓ 产品 {}: {} (¥{:.2})", product.id, product.name, product.price);
    }
    println!();

    // 获取产品
    println!("   获取产品...");
    for product in &products {
        if let Some(p) = cache.get(&format!("product:{}", product.id)).await? {
            println!("   ✓ 产品 {}: {} (¥{:.2})", p.id, p.name, p.price);
        }
    }
    println!();

    // 批量操作测试 (验证跨槽操作)
    println!("2. 批量操作测试");
    let keys: Vec<String> = (1..=10).map(|i| format!("product:{}", i)).collect();

    println!("   并发读取 10 个产品...");
    let start = std::time::Instant::now();
    let mut handles = Vec::new();
    for key in &keys {
        let cache = cache.clone();
        let k = key.clone();
        let handle = tokio::spawn(async move {
            cache.get(&k).await
        });
        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(Ok(Some(product))) = handle.await {
            results.push(product);
        }
    }
    let elapsed = start.elapsed();

    println!("   ✓ 读取 {} 个产品，耗时: {:?}", results.len(), elapsed);
    println!();

    // 清空测试数据
    println!("3. 清理测试数据");
    for product in &products {
        cache.delete(&format!("product:{}", product.id)).await?;
    }
    println!("   ✓ 测试数据已清理\n");

    // 统计信息
    println!("4. 缓存统计");
    let stats = cache.stats().await?;
    println!("   - 总条目数: {}", stats.item_count());
    println!("   - 命中次数: {}", stats.hit_count());
    println!("   - 未命中次数: {}", stats.miss_count());
    println!();

    println!("=== Redis Cluster 模式示例完成 ===");
    Ok(())
}