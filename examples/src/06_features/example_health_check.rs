//! 健康检查示例
//!
//! 本示例演示如何使用 Oxcache 进行健康检查。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_health_check
//!

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct HealthStatus {
    service: String,
    status: String,
    latency_ms: f64,
    checked_at: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 健康检查示例 ===\n");

    // 创建缓存用于存储健康状态
    let cache: Cache<String, HealthStatus> = Cache::new().await?;

    // 1. 检查多个服务的健康状态
    println!("1. 服务健康检查");
    let services = vec![
        ("api-gateway", "healthy"),
        ("user-service", "healthy"),
        ("order-service", "degraded"),
        ("payment-service", "healthy"),
    ];

    println!("   检查服务健康状态...");
    for (service, status) in &services {
        let start = std::time::Instant::now();
        // 模拟健康检查延迟
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let latency = start.elapsed().as_secs_f64() * 1000.0;

        let health_status = HealthStatus {
            service: service.to_string(),
            status: status.to_string(),
            latency_ms: latency,
            checked_at: chrono::Local::now().to_rfc3339(),
        };

        cache.set(service, &health_status, Some(60)).await?;
        let icon = if *status == "healthy" { "✓" } else { "⚠" };
        println!("   {} {} - {} (延迟: {:.2}ms)", icon, service, status, latency);
    }
    println!();

    // 2. 获取所有服务的健康状态
    println!("2. 获取所有服务健康状态");
    let all_statuses = cache.iter().await?;
    println!("   服务健康状态汇总:");
    let mut healthy_count = 0;
    let mut degraded_count = 0;

    for (key, status) in all_statuses {
        let icon = if status.status == "healthy" { "✓" } else { "⚠" };
        println!("     {} {}: {} (延迟: {:.2}ms)", icon, key, status.status, status.latency_ms);

        if status.status == "healthy" {
            healthy_count += 1;
        } else {
            degraded_count += 1;
        }
    }
    println!();
    println!("   健康: {}, 异常: {}", healthy_count, degraded_count);
    println!();

    // 3. 模拟定期健康检查
    println!("3. 模拟定期健康检查");
    println!("   正在进行第 1 轮检查...");

    // 更新所有服务状态
    for (service, _) in &services {
        let start = std::time::Instant::new();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let latency = start.elapsed().as_secs_f64() * 1000.0;

        // 模拟状态变化
        let new_status = if rand::random::<f32>() < 0.9 {
            "healthy"
        } else {
            "degraded"
        };

        let health_status = HealthStatus {
            service: service.to_string(),
            status: new_status.to_string(),
            latency_ms: latency,
            checked_at: chrono::Local::now().to_rfc3339(),
        };

        cache.set(service, &health_status, Some(60)).await?;
        let icon = if new_status == "healthy" { "✓" } else { "⚠" };
        println!("     {} {}: {}", icon, service, new_status);
    }
    println!("   第 1 轮检查完成\n");

    // 4. 清理
    println!("4. 清理测试数据");
    cache.clear().await?;
    println!("   ✓ 测试数据已清理\n");

    println!("=== 健康检查示例完成 ===");
    Ok(())
}