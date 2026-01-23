//! 速率限制示例
//!
//! 本示例演示如何使用 Oxcache 实现速率限制。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_rate_limiting
//!

use oxcache::rate_limiting::{GlobalRateLimiter, RateLimitConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 速率限制示例 ===\n");

    // 1. 创建速率限制器
    println!("1. 创建速率限制器");
    let config = RateLimitConfig {
        max_requests_per_second: 10,
        burst_capacity: 20,
        block_duration_secs: 60,
    };

    let limiter = GlobalRateLimiter::new(Some(config));

    println!("   配置参数:");
    println!("     - 每秒最大请求数: {}", 10);
    println!("     - 突发容量: {}", 20);
    println!("     - 阻塞持续时间: {} 秒", 60);
    println!();

    // 2. 模拟用户请求
    println!("2. 模拟用户请求速率限制");
    let user_id = 1001;

    println!("   用户 {} 的请求测试:", user_id);
    println!();

    // 模拟 25 次请求（超过速率限制）
    for i in 1..=25 {
        let key = format!("user:{}", user_id);

        match limiter.inner().check_rate_limit(&key, 1).await {
            Ok(()) => {
                println!("   请求 #{}: ✓ 通过", i);
            }
            Err(wait_time) => {
                println!(
                    "   请求 #{}: ❌ 已达到速率限制，需等待 {:?}",
                    i, wait_time
                );
            }
        }

        // 控制请求速度（每 100ms 一个请求）
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    println!();

    // 3. 测试不同用户的速率限制
    println!("3. 测试多用户速率限制");
    let users = vec![1002, 1003, 1004];

    for user_id in users {
        let key = format!("user:{}", user_id);

        // 每个用户发送 15 个请求
        println!("   用户 {}:", user_id);
        for i in 1..=15 {
            match limiter.inner().check_rate_limit(&key, 1).await {
                Ok(()) => print!("✓"),
                Err(_) => print!("✗"),
            }
        }
        println!(); // 换行
    }
    println!();

    // 4. 测试突发流量
    println!("4. 测试突发流量处理");
    let burst_user = 2001;
    let key = format!("user:{}", burst_user);

    println!("   用户 {} 快速发送 25 个请求:", burst_user);
    for i in 1..=25 {
        match limiter.inner().check_rate_limit(&key, 1).await {
            Ok(()) => print!("✓"),
            Err(_) => print!("✗"),
        }
    }
    println!(); // 换行
    println!();

    // 5. 等待并重试
    println!("5. 等待令牌桶恢复后重试");
    println!("   等待 2 秒...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("   重试 5 个请求:");
    for i in 1..=5 {
        match limiter.inner().check_rate_limit(&key, 1).await {
            Ok(()) => {
                println!("   重试 #{}: ✓ 通过", i);
            }
            Err(wait_time) => {
                println!("   重试 #{}: ❌ 仍被限制，需等待 {:?}", i, wait_time);
            }
        }
    }
    println!();

    println!("=== 速率限制示例完成 ===");
    println!("   速率限制的作用：");
    println!("   - 防止 DoS 攻击");
    println!("   - 保护后端服务");
    println!("   - 控制资源使用");
    println!("   - 提供公平的访问控制");
    Ok(())
}