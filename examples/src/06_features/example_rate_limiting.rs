//! 速率限制示例
//!
//! 本示例演示如何使用 Oxcache 实现速率限制。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_rate_limiting
//!

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct RateLimitConfig {
    user_id: u64,
    max_requests: u32,
    window_secs: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct RateLimitStatus {
    user_id: u64,
    current: u32,
    remaining: u32,
    reset_at: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 速率限制示例 ===\n");

    // 创建缓存用于存储速率限制状态
    let cache: Cache<String, RateLimitStatus> = Cache::new().await?;

    // 1. 模拟用户请求
    println!("1. 模拟用户请求速率限制");
    let user_id = 1001;
    let max_requests = 10;
    let window_secs = 60;

    println!("   用户 {} 的速率限制: {} 次/{} 秒", user_id, max_requests, window_secs);
    println!();

    // 模拟 15 次请求
    for i in 1..=15 {
        let key = format!("ratelimit:{}", user_id);

        // 获取当前计数
        let mut status = cache.get(&key).await?.unwrap_or(RateLimitStatus {
            user_id,
            current: 0,
            remaining: max_requests,
            reset_at: 0,
        });

        if status.current >= max_requests {
            println!("   请求 #{}: ❌ 已达到速率限制", i);
        } else {
            status.current += 1;
            status.remaining = max_requests - status.current;
            cache.set(&key, &status, Some(window_secs)).await?;
            println!(
                "   请求 #{}: ✓ 通过 (当前: {}/{}, 剩余: {})",
                i, status.current, max_requests, status.remaining
            );
        }
    }
    println!();

    // 2. 查看速率限制状态
    println!("2. 查看速率限制状态");
    let key = format!("ratelimit:{}", user_id);
    if let Some(status) = cache.get(&key).await? {
        println!("   用户 {} 的速率限制状态:", status.user_id);
        println!("     - 当前请求数: {}", status.current);
        println!("     - 最大请求数: {}", max_requests);
        println!("     - 剩余请求数: {}", status.remaining);
    }
    println!();

    // 3. 清理
    println!("3. 清理测试数据");
    cache.clear().await?;
    println!("   ✓ 测试数据已清理\n");

    println!("=== 速率限制示例完成 ===");
    Ok(())
}