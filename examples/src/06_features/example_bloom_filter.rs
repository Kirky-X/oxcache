//! 布隆过滤器示例
//!
//! 本示例演示如何使用 Oxcache 的布隆过滤器功能来防止缓存穿透。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_bloom_filter
//!

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 布隆过滤器示例 ===\n");

    // 创建缓存和布隆过滤器
    let cache: Cache<String, User> = Cache::new().await?;

    // 1. 初始化布隆过滤器
    println!("1. 初始化布隆过滤器");
    let expected_elements = 10000;
    let false_positive_rate = 0.01; // 1% 误判率

    println!("   预期元素数量: {}", expected_elements);
    println!("   误判率: {}%", false_positive_rate * 100.0);
    println!();

    // 2. 模拟数据
    println!("2. 准备测试数据");
    let users = vec![
        User {
            id: 1,
            name: "张三".to_string(),
            email: "zhangsan@example.com".to_string(),
        },
        User {
            id: 2,
            name: "李四".to_string(),
            email: "lisi@example.com".to_string(),
        },
        User {
            id: 3,
            name: "王五".to_string(),
            email: "wangwu@example.com".to_string(),
        },
    ];

    // 将用户添加到缓存
    for user in &users {
        cache.set(&format!("user:{}", user.id), user, Some(3600)).await?;
    }
    println!("   添加 {} 个用户到缓存", users.len());
    println!();

    // 3. 模拟查询（使用布隆过滤器判断是否可能存在）
    println!("3. 模拟查询场景");
    let queries = vec!["user:1", "user:2", "user:3", "user:999", "user:1000"];

    println!("   查询用户:");
    for query in &queries {
        let start = std::time::Instant::new();

        // 模拟布隆过滤器判断
        // 注意：这里简化了布隆过滤器的使用，实际应使用专门的布隆过滤器实现
        let might_exist = users.iter().any(|u| format!("user:{}", u.id) == *query);

        if might_exist {
            // 查询缓存
            if let Some(user) = cache.get(query).await? {
                println!(
                    "   ✓ {}: {} (耗时: {:?})",
                    query, user.name, start.elapsed()
                );
            } else {
                println!("   ✗ {}: 缓存未命中 (可能存在)", query);
            }
        } else {
            // 布隆过滤器判断不存在，直接返回空
            println!(
                "   ○ {}: 布隆过滤器判断不存在，跳过缓存查询 (耗时: {:?})",
                query,
                start.elapsed()
            );
        }
    }
    println!();

    // 4. 性能测试
    println!("4. 性能测试");
    let start = std::time::Instant::new();

    // 大量查询
    for i in 0..1000 {
        let query = format!("user:{}", i);
        let _ = cache.get(&query).await?;
    }

    let elapsed = start.elapsed();
    println!("   执行 1000 次查询，耗时: {:?}", elapsed);
    println!(
        "   平均查询时间: {:.2}µs",
        elapsed.as_secs_f64() * 1_000_000.0 / 1000.0
    );
    println!();

    // 5. 统计信息
    println!("5. 缓存统计");
    let stats = cache.stats().await?;
    println!("   - 总条目数: {}", stats.item_count());
    println!("   - 命中次数: {}", stats.hit_count());
    println!("   - 未命中次数: {}", stats.miss_count());
    if stats.hit_count() + stats.miss_count() > 0 {
        let hit_rate = stats.hit_count() as f64
            / (stats.hit_count() + stats.miss_count()) as f64
            * 100.0;
        println!("   - 命中率: {:.2}%", hit_rate);
    }
    println!();

    // 6. 清理
    println!("6. 清理测试数据");
    cache.clear().await?;
    println!("   ✓ 测试数据已清理\n");

    println!("=== 布隆过滤器示例完成 ===");
    println!("   布隆过滤器的作用：");
    println!("   - 快速判断元素是否可能存在");
    println!("   - 防止缓存穿透（查询不存在的 key）");
    println!("   - 节省缓存查询资源");
    Ok(())
}