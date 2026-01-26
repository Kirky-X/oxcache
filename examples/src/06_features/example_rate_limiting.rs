// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//

//! 限流保护使用示例
//!
//! 本示例演示了 Oxcache 的限流保护功能：
//! - 令牌桶算法
//! - 客户端维度限流
//! - 全局限流
//! - 防止 DoS 攻击
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_rate_limiting
//! ```

use oxcache::rate_limiting::{GlobalRateLimiter, ClientRateLimiter, RateLimitConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 限流保护使用示例 ===
");

    // 1. 创建限流器配置
    println!("1. 创建限流器配置");
    let config = RateLimitConfig {
        max_requests_per_second: 10,    // 每秒最大请求数
        burst_capacity: 20,             // 突发容量
        block_duration_secs: 1,         // 阻塞时长（秒）
    };
    
    println!("   每秒最大请求数: {}", config.max_requests_per_second);
    println!("   突发容量: {}", config.burst_capacity);
    println!("   阻塞时长: {}秒", config.block_duration_secs);
    println!();

    // 2. 创建限流器实例
    println!("2. 创建限流器实例");
    let limiter = GlobalRateLimiter::new(Some(config.clone()));
    let inner_limiter = limiter.inner().clone();
    println!("   ✓ 限流器创建成功");
    println!();

    // 3. 基础限流测试
    println!("3. 基础限流测试");
    println!("   发送 5 个请求（低于限制）:");
    
    for i in 1..=5 {
        let result = inner_limiter.check_rate_limit("client_1", 1).await;
        match result {
            Ok(()) => println!("   请求 {}: 允许通过", i),
            Err(_) => println!("   请求 {}: 被限流", i),
        }
    }
    println!();

    // 4. 达到限流阈值测试
    println!("4. 达到限流阈值测试");
    println!("   发送超过限制的请求:");
    
    for i in 6..=15 {
        let result = inner_limiter.check_rate_limit("client_1", 1).await;
        match result {
            Ok(()) => println!("   请求 {}: 允许通过", i),
            Err(_) => println!("   请求 {}: 被限流", i),
        }
    }
    println!();

    // 5. 多客户端限流测试
    println!("5. 多客户端限流测试");
    let clients = vec!["client_a", "client_b", "client_c"];
    
    for client in &clients {
        println!("   测试客户端 '{}':", client);
        
        for req in 1..=8 {
            let result = inner_limiter.check_rate_limit(client, 1).await;
            match result {
                Ok(()) => print!("     请求{}: ✓ ", req),
                Err(_) => print!("     请求{}: ⚠ ", req),
            }
            
            if req % 4 == 0 {
                println!(); // 每4个请求换行
            }
        }
        println!();
    }
    println!();

    // 6. 实际应用场景：API 请求限流
    println!("6. 实际应用场景：API 请求限流");
    
    // 模拟一个API端点的限流逻辑
    async fn simulate_api_call(
        limiter: &ClientRateLimiter,
        client_id: &str,
        endpoint: &str
    ) -> Result<String, String> {
        let result = limiter.check_rate_limit(client_id, 1).await;
        
        match result {
            Ok(()) => {
                println!("   客户端 '{}' 访问 '{}' - 请求允许", client_id, endpoint);
                
                // 模拟 API 处理
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                
                Ok(format!("Response from {}", endpoint))
            },
            Err(_) => {
                println!("   客户端 '{}' 访问 '{}' - 请求被限流", client_id, endpoint);
                Err("Rate limit exceeded".to_string())
            }
        }
    }
    
    // 模拟客户端请求
    println!("   模拟客户端 'api_client' 访问 '/users' 端点:");
    for i in 1..=12 {
        let result = simulate_api_call(&*inner_limiter, "api_client", "/users").await;
        match result {
            Ok(response) => println!("     第 {} 次访问: {}", i, response),
            Err(error) => println!("     第 {} 次访问: 错误 - {}", i, error),
        }
        
        // 添加一点延迟以模拟真实请求间隔
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    println!();

    // 7. 统计信息
    // 注意：当前实现没有提供统计信息API
    println!("7. 限流器统计信息");
    println!("   限流器统计信息功能暂时不可用");
    println!();

    // 8. 不同配置的限流器比较
        
    // 严格的限流器
    let strict_config = RateLimitConfig {
        max_requests_per_second: 5,
        burst_capacity: 5,
        block_duration_secs: 2,
    };
    let strict_limiter = GlobalRateLimiter::new(Some(strict_config));
    let strict_inner = strict_limiter.inner().clone();
        
    println!("   严格限流器 (5 req/s, 5 burst):");
    for i in 1..=10 {
        let result = strict_inner.check_rate_limit("strict_client", 1).await;
        match result {
            Ok(()) => print!("A "),
            Err(_) => print!("L "),
        }
    }
    println!();
        
    // 宽松的限流器
    let loose_config = RateLimitConfig {
        max_requests_per_second: 50,
        burst_capacity: 100,
        block_duration_secs: 1,
    };
    let loose_limiter = GlobalRateLimiter::new(Some(loose_config));
    let loose_inner = loose_limiter.inner().clone();
        
    println!("   宽松限流器 (50 req/s, 100 burst):");
    for i in 1..=10 {
        let result = loose_inner.check_rate_limit("loose_client", 1).await;
        match result {
            Ok(()) => print!("A "),
            Err(_) => print!("L "),
        }
    }
    println!();

    // 9. 性能测试
    println!("9. 性能测试");
    let start = std::time::Instant::now();
    
    let mut allowed_count = 0;
    let mut limited_count = 0;
    
    for i in 0..1000 {
        match inner_limiter.check_rate_limit(&format!("perf_test_{}", i % 10), 1).await {
            Ok(()) => allowed_count += 1,
            Err(_) => limited_count += 1,
        }
    }
    
    let elapsed = start.elapsed();
    println!("   1000 次限流检查耗时: {:?}", elapsed);
    println!("   平均每次检查耗时: {:?}", elapsed / 1000);
    println!("   允许请求数: {}, 限流请求数: {}", allowed_count, limited_count);
    println!();

    println!("=== 限流保护示例完成 ===");
    println!("   限流保护的主要优势:");
    println!("   - 防止 DoS 攻击：限制恶意请求");
    println!("   - 保护后端服务：控制请求流量");
    println!("   - 令牌桶算法：支持突发流量");
    println!("   - 客户端隔离：避免单个客户端影响整体");

    Ok(())
}
