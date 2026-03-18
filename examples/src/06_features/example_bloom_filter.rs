// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//

//! 布隆过滤器使用示例
//!
//! 本示例演示了 Oxcache 的布隆过滤器功能：
//! - 防止缓存穿透攻击
//! - 快速判断键是否存在
//! - 可配置的误判率
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_bloom_filter
//! ```

use oxcache::bloom_filter::{BloomFilter, BloomFilterOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 布隆过滤器使用示例 ===
");

    // 1. 创建布隆过滤器配置
    println!("1. 创建布隆过滤器配置");
    let options = BloomFilterOptions::new(
        "user_bloom_filter".to_string(),
        100000,                             // 预期元素数量
        0.01,                               // 误判率 1%
    );

    println!("   预期元素数量: {}", options.expected_elements);
    println!("   误判率: {:.2}%", options.false_positive_rate * 100.0);
    println!();

    // 2. 创建布隆过滤器实例
    println!("2. 创建布隆过滤器实例");

    let mut filter = BloomFilter::new(options);
    println!("   ✓ 布隆过滤器创建成功");
    println!("   名称: {}", filter.get_stats().name);
    println!();

    // 3. 添加元素到过滤器
    println!("3. 添加元素到过滤器");
    let keys_to_add = vec![
        "user:123", "user:456", "user:789",
        "product:abc", "product:def", "order:xyz"
    ];

    for key in &keys_to_add {
        filter.add(key.as_bytes())?;
        println!("   添加键: {}", key);
    }
    println!();

    // 4. 检查存在的键
    println!("4. 检查已添加的键");
    for key in &keys_to_add {
        let might_exist = filter.contains(key.as_bytes())?;
        println!("   键 '{}' 可能存在: {}", key, might_exist);
    }
    println!();

    // 5. 检查不存在的键
    println!("5. 检查未添加的键 (模拟缓存穿透)");
    let non_existent_keys = vec![
        "user:999", "user:888", "user:777",
        "nonexistent:123", "fake:key"
    ];

    for key in &non_existent_keys {
        let might_exist = filter.contains(key.as_bytes())?;
        println!("   键 '{}' 可能存在: {} (实际上不存在)", key, might_exist);

        // 模拟缓存穿透防护逻辑
        if !might_exist {
            println!("     -> 跳过缓存查询，直接返回 None");
        } else {
            println!("     -> 继续缓存查询流程");
        }
    }
    println!();

    // 6. 实际应用示例：缓存穿透防护
    println!("6. 实际应用示例：缓存穿透防护");

    // 模拟一个缓存查询函数
    async fn query_cache_with_bloom_filter(
        filter: &BloomFilter,
        key: &str
    ) -> Result<String, String> {
        // 首先检查布隆过滤器
        let exists = filter.contains(key.as_bytes()).map_err(|e| e.to_string())?;
        if !exists {
            println!("   布隆过滤器判定键 '{}' 不存在，跳过缓存查询", key);
            return Err("Key definitely not in cache".to_string());
        }

        println!("   布隆过滤器判定键 '{}' 可能存在，继续缓存查询", key);

        // 这里通常是实际的缓存查询逻辑
        // 为了演示，我们模拟查询过程
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // 模拟查询结果
        if key.starts_with("user:") {
            Ok(format!("Cached data for {}", key))
        } else {
            Err("Key not found in cache".to_string())
        }
    }

    // 测试存在的键
    println!("   测试存在的键 'user:123':");
    match query_cache_with_bloom_filter(&filter, "user:123").await {
        Ok(data) => println!("     → 查询成功: {}", data),
        Err(e) => println!("     → 查询失败: {}", e),
    }

    // 测试不存在的键（缓存穿透防护）
    println!("   测试不存在的键 'user:999':");
    match query_cache_with_bloom_filter(&filter, "user:999").await {
        Ok(data) => println!("     → 查询成功: {}", data),
        Err(e) => println!("     → {}", e),
    }
    println!();

    // 7. 性能对比示例
    println!("7. 性能对比示例");

    let start = std::time::Instant::now();
    for i in 0..10000 {
        let _ = filter.contains(format!("test:{}", i).as_bytes())?;
    }
    let bloom_check_time = start.elapsed();

    println!("   布隆过滤器检查 10000 次耗时: {:?}", bloom_check_time);
    println!("   平均每次检查耗时: {:?}", bloom_check_time / 10000);
    println!();

    // 8. 统计信息
    println!("8. 布隆过滤器统计信息");
    let stats = filter.get_stats();
    println!("   总添加次数: {}", stats.added_count);
    println!("   总检查次数: {}", stats.checked_count);
    println!("   误判次数: {}", stats.false_positive_count);
    println!("   位数组利用率: {:.2}%", stats.utilization * 100.0);

    if stats.checked_count > 0 {
        let actual_false_positive_rate =
            stats.false_positive_count as f64 /
            stats.checked_count as f64;
        println!("   实际误判率: {:.2}%", actual_false_positive_rate * 100.0);
    }
    println!();

    println!("=== 布隆过滤器示例完成 ===");
    println!("   布隆过滤器的主要优势:");
    println!("   - 防止缓存穿透：快速判断键是否存在");
    println!("   - 内存效率：相比存储完整键集合节省大量内存");
    println!("   - 高性能：O(1) 时间复杂度");
    println!("   - 可配置：根据需求调整误判率和容量");

    Ok(())
}
