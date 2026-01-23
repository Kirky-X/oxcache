//! MySQL 数据库分区管理示例
//!
//! 本示例演示如何使用 Oxcache 的 MySQL 分区管理器。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_mysql_cache
//!

use oxcache::database::mysql::MySQLPartitionManager;
use oxcache::database::partition::{PartitionConfig, TimeUnit};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MySQL 数据库分区管理示例 ===\n");

    // 创建 MySQL 分区管理器
    println!("1. 创建 MySQL 分区管理器...");
    let partition_manager = MySQLPartitionManager::new(
        "mysql://root:password@localhost:3306/test_db"
    ).await?;
    println!("   ✓ 分区管理器创建成功\n");

    // 创建时间分区配置（按月分区）
    println!("2. 创建时间分区配置（按月分区）...");
    let time_config = PartitionConfig::time_based(TimeUnit::Month);
    println!("   ✓ 时间分区配置创建成功\n");

    // 创建哈希分区配置
    println!("3. 创建哈希分区配置（4 个分片）...");
    let hash_config = PartitionConfig::hash_based(4);
    println!("   ✓ 哈希分区配置创建成功\n");

    // 列出所有分区
    println!("4. 列出所有分区...");
    match partition_manager.list_partitions().await {
        Ok(partitions) => {
            println!("   现有分区:");
            for partition in partitions {
                println!("     - {}", partition);
            }
        }
        Err(e) => {
            println!("   ⚠ 无法列出分区: {}", e);
            println!("   (这可能是因为数据库表尚未创建)");
        }
    }
    println!();

    // 创建新分区
    println!("5. 创建新分区...");
    match partition_manager.create_partition("users_2026_01").await {
        Ok(()) => println!("   ✓ 分区创建成功"),
        Err(e) => println!("   ⚠ 分区创建失败: {}", e),
    }
    println!();

    // 查询分区信息
    println!("6. 查询分区信息...");
    match partition_manager.get_partition_info("users_2026_01").await {
        Ok(info) => {
            println!("   分区信息:");
            println!("     - 名称: {}", info.name);
            println!("     - 类型: {:?}", info.partition_type);
            println!("     - 创建时间: {:?}", info.created_at);
        }
        Err(e) => {
            println!("   ⚠ 无法获取分区信息: {}", e);
        }
    }
    println!();

    // 删除分区
    println!("7. 删除分区...");
    match partition_manager.drop_partition("users_2026_01").await {
        Ok(()) => println!("   ✓ 分区删除成功"),
        Err(e) => println!("   ⚠ 分区删除失败: {}", e),
    }
    println!();

    println!("=== MySQL 数据库分区管理示例完成 ===");
    println!("\n注意: 此示例需要 MySQL 数据库运行在 localhost:3306");
    println!("      请确保数据库连接字符串正确，并已创建相应的表结构。");
    Ok(())
}