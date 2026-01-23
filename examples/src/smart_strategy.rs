//! 智能策略示例
//!
//! 本示例演示 Oxcache 的智能策略功能。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_smart_strategy
//!

use std::time::Duration;
use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    data: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 智能策略示例 ===\n");

    // 创建缓存
    let cache: Cache<String, User> = Cache::new().await?;

    // 1. 预取策略演示
    println!("1. 预取策略演示");

    // 预加载数据
    for i in 1..=100 {
        let user = User {
            id: i,
            name: format!("用户{}", i),
            data: format!("用户{}的详细数据", i),
        };
        cache
            .set_with_ttl(&format!("user:{}", i), &user, Some(Duration::from_secs(3600)))
            .await?;
    }
    println!("   预加载 100 个用户数据");

    // 模拟访问模式 - 访问前 10 个用户多次
    println!("   模拟访问模式...");
    for _ in 0..10 {
        for i in 1..=10 {
            let _ = cache.get(&format!("user:{}", i)).await?;
        }
    }
    println!("   完成 100 次访问");

    // 2. 压缩策略演示
    println!("\n2. 压缩策略演示");
    println!("   智能策略会自动根据数据大小决定是否压缩");
    println!("   (对于大型数据项启用压缩，小型数据保持未压缩)");

    // 添加不同大小的数据
    let small_data = User {
        id: 1,
        name: "小数据".to_string(),
        data: "small".to_string(),
    };
    let large_data = User {
        id: 2,
        name: "大数据".to_string(),
        data: "x".repeat(10000), // 10KB 数据
    };

    cache.set_with_ttl(&"small".to_string(), &small_data, Some(Duration::from_secs(3600))).await?;
    cache.set_with_ttl(&"large".to_string(), &large_data, Some(Duration::from_secs(3600))).await?;
    println!("   添加小型和大型数据各一个");

    // 3. 缓存效率演示
    println!("\n3. 缓存效率演示");

    // 访问数据
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        for i in 1..=100 {
            let _ = cache.get(&format!("user:{}", i)).await?;
        }
    }
    let elapsed = start.elapsed();
    let total_ops = 1000 * 100;

    println!("   执行 {} 次读取操作", total_ops);
    println!("   耗时: {:?}", elapsed);
    println!(
        "   吞吐量: {:.2} ops/sec",
        total_ops as f64 / elapsed.as_secs_f64()
    );

    // 4. 缓存统计
    println!("\n4. 缓存统计");
    println!("   - 缓存操作已执行");
    println!("   - 详情请查看日志输出");

    // 清理
    println!("\n5. 清理测试数据");
    cache.clear().await?;
    println!("   ✓ 测试数据已清理\n");

    println!("=== 智能策略示例完成 ===");
    println!("   智能策略功能：");
    println!("   - 自动预取热点数据");
    println!("   - 智能压缩决策");
    println!("   - 访问模式优化");
    Ok(())
}