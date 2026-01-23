//! MySQL 数据库连接示例
//!
//! 本示例演示如何连接 MySQL 数据库并使用分区功能。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_mysql_cache
//!

use oxcache::database::mysql::MySQLPartitionManager;
use oxcache::database::partition::PartitionConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MySQL 数据库连接示例 ===\n");

    // 创建分区配置
    println!("1. 创建分区配置...");
    let config = PartitionConfig {
        enabled: true,
        strategy: oxcache::database::partition::PartitionStrategy::Monthly,
        retention_months: 12,
        precreate_months: 3,
    };
    println!("   ✓ 分区配置创建成功\n");

    // 创建 MySQL 分区管理器
    println!("2. 创建 MySQL 分区管理器...");
    match MySQLPartitionManager::new(
        "mysql://root:password@localhost:3306/test_db",
        config,
    ).await {
        Ok(manager) => {
            println!("   ✓ 分区管理器创建成功\n");

            // 健康检查
            println!("3. 执行健康检查...");
            let healthy = manager.health_check().await;
            if healthy {
                println!("   ✓ 数据库连接正常");
            } else {
                println!("   ✗ 数据库连接异常");
            }
            println!();

            // Ping 测试
            println!("4. Ping 测试...");
            match manager.ping().await {
                Ok(()) => println!("   ✓ Ping 成功"),
                Err(e) => println!("   ✗ Ping 失败: {}", e),
            }
            println!();

            // 连接池状态
            println!("5. 连接池状态...");
            let stats = manager.pool_stats().await;
            println!("   - 活跃连接: {}", stats.active_connections);
            println!("   - 空闲连接: {}", stats.idle_connections);
            println!();
        }
        Err(e) => {
            println!("   ✗ 分区管理器创建失败: {}", e);
            println!("   (这可能是因为数据库连接失败，请确保 MySQL 正在运行)");
        }
    }

    println!("=== MySQL 数据库连接示例完成 ===");
    println!("\n注意: 此示例需要 MySQL 数据库运行在 localhost:3306");
    println!("      请确保数据库连接字符串正确。");
    Ok(())
}
