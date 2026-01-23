//! 布隆过滤器示例
//!
//! 本示例演示如何使用 Oxcache 的布隆过滤器功能来防止缓存穿透。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_bloom_filter --all-features
//!

use oxcache::bloom_filter::{BloomFilter, BloomFilterOptions};
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

    // 创建缓存
    let cache: Cache<String, User> = Cache::new().await?;

    // 1. 初始化布隆过滤器
    println!("1. 初始化布隆过滤器");
    let expected_elements = 10000;
    let false_positive_rate = 0.01; // 1% 误判率

    let options = BloomFilterOptions::new(
        "user_cache".to_string(),
        expected_elements,
        false_positive_rate,
    );

    let mut bloom_filter = BloomFilter::new(options);

    println!("   预期元素数量: {}", expected_elements);
    println!("   误判率: {}%", false_positive_rate * 100.0);
    println!();

    // 2. 准备测试数据并添加到布隆过滤器
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

    // 将用户添加到缓存和布隆过滤器
    for user in &users {
        let key = format!("user:{}", user.id);
        cache.set_with_ttl(&key, user, Some(std::time::Duration::from_secs(3600))).await?;
        bloom_filter.add(key.as_bytes())?;
    }
    println!("   添加 {} 个用户到缓存和布隆过滤器", users.len());
    println!();

    // 3. 模拟查询（使用布隆过滤器判断是否可能存在）
    println!("3. 模拟查询场景");
    let queries = vec!["user:1", "user:2", "user:3", "user:999", "user:1000"];

    println!("   查询用户:");
    for query in &queries {
        let start = std::time::Instant::now();

        // 使用布隆过滤器判断是否可能存在
        if bloom_filter.contains(query.as_bytes()).unwrap_or(false) {
            // 可能存在，查询缓存
            if let Some(user) = cache.get(&query.to_string()).await? {
                println!(
                    "   ✓ {}: {} (耗时: {:?})",
                    query, user.name, start.elapsed()
                );
            } else {
                println!("   ✗ {}: 缓存未命中 (误判)", query);
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
    let start = std::time::Instant::now();

    // 大量查询（使用布隆过滤器优化）
    for i in 0..1000 {
        let query = format!("user:{}", i);
        if bloom_filter.contains(query.as_bytes()).unwrap_or(false) {
            let _ = cache.get(&query).await?;
        }
        // 否则跳过缓存查询
    }

    let elapsed = start.elapsed();
    println!("   执行 1000 次查询（带布隆过滤器优化），耗时: {:?}", elapsed);
    println!(
        "   平均查询时间: {:.2}µs",
        elapsed.as_secs_f64() * 1_000_000.0 / 1000.0
    );
    println!();

    // 5. 统计信息
    println!("5. 布隆过滤器统计");
    let stats = bloom_filter.get_stats();
    println!("   - 添加元素数: {}", stats.added_count);
    println!("   - 检查次数: {}", stats.checked_count);
    println!("   - 误判次数: {}", stats.false_positive_count);
    if stats.checked_count > 0 {
        let fp_rate = stats.false_positive_count as f64 / stats.checked_count as f64 * 100.0;
        println!("   - 实际误判率: {:.2}%", fp_rate);
    }
    println!("   - 位图利用率: {:.2}%", stats.utilization * 100.0);

    println!("\n6. 缓存统计");
    match cache.stats().await {
        Ok(stats_map) => {
            if let Some(item_count) = stats_map.get("item_count") {
                println!("   - 总条目数: {}", item_count);
            }
            if let Some(hit_count) = stats_map.get("hit_count") {
                println!("   - 命中次数: {}", hit_count);
            }
            if let Some(miss_count) = stats_map.get("miss_count") {
                println!("   - 未命中次数: {}", miss_count);
            }
            // 计算命中率
            if let (Some(hit), Some(miss)) = (stats_map.get("hit_count"), stats_map.get("miss_count")) {
                if let (Ok(hit_num), Ok(miss_num)) = (hit.parse::<u64>(), miss.parse::<u64>()) {
                    if hit_num + miss_num > 0 {
                        let hit_rate = hit_num as f64 / (hit_num + miss_num) as f64 * 100.0;
                        println!("   - 命中率: {:.2}%", hit_rate);
                    }
                }
            }
        }
        Err(e) => {
            println!("   - 无法获取缓存统计: {}", e);
        }
    }
    println!();

    // 7. 清理
    println!("7. 清理测试数据");
    cache.clear().await?;
    println!("   ✓ 测试数据已清理\n");

    println!("=== 布隆过滤器示例完成 ===");
    println!("   布隆过滤器的作用：");
    println!("   - 快速判断元素是否可能存在");
    println!("   - 防止缓存穿透（查询不存在的 key）");
    println!("   - 节省缓存查询资源");
    println!("   - 降低数据库负载");
    Ok(())
}