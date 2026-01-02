//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! MySQL调试测试

use oxcache::database::mysql::MySQLPartitionManager;
use oxcache::database::{PartitionConfig, PartitionManager, PartitionStrategy};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn debug_mysql_initialize_table() {
    println!("=== Debug MySQL Initialize Table ===");

    let config = PartitionConfig {
        enabled: true,
        strategy: PartitionStrategy::Monthly,
        precreate_months: 3,
        retention_months: Some(6),
        table_prefix: "test_cache".to_string(),
    };

    let connection_string = "mysql://user:password@localhost:3307/oxcache_test";

    println!("1. Creating MySQL partition manager...");
    let manager = match MySQLPartitionManager::new(connection_string, config).await {
        Ok(mgr) => {
            println!("✓ MySQL partition manager created successfully!");
            mgr
        }
        Err(e) => {
            println!("✗ Failed to create manager: {}", e);
            return;
        }
    };

    let table_name = "debug_test_table";
    let schema = "CREATE TABLE IF NOT EXISTS debug_test_table (
        id INT AUTO_INCREMENT PRIMARY KEY,
        created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
        data TEXT
    )";

    println!("2. Testing table initialization...");
    println!("   Table: {}", table_name);
    println!("   Schema: {}", schema);

    // 测试1: 基础连接测试
    println!("3. Testing basic connection...");
    let connection_test = timeout(Duration::from_secs(10), async {
        // 简单的查询测试
        let result = manager.get_partitions("information_schema.tables").await;
        match result {
            Ok(partitions) => println!(
                "✓ Connection test passed, found {} partitions",
                partitions.len()
            ),
            Err(e) => println!("✗ Connection test failed: {}", e),
        }
    })
    .await;

    if connection_test.is_err() {
        println!("✗ Connection test timed out");
        return;
    }

    // 测试2: 表创建（无分区）
    println!("4. Testing table creation without partitioning...");
    let create_table_test = timeout(Duration::from_secs(30), async {
        let simple_schema = "CREATE TABLE IF NOT EXISTS debug_simple_table (
            id INT AUTO_INCREMENT PRIMARY KEY,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            data VARCHAR(100)
        )";

        match manager
            .initialize_table("debug_simple_table", simple_schema)
            .await
        {
            Ok(_) => println!("✓ Simple table creation passed"),
            Err(e) => println!("✗ Simple table creation failed: {}", e),
        }
    })
    .await;

    if create_table_test.is_err() {
        println!("✗ Simple table creation timed out");
    }

    // 测试3: 分区表创建
    println!("5. Testing partitioned table creation...");
    let partition_test = timeout(Duration::from_secs(30), async {
        match manager.initialize_table(table_name, schema).await {
            Ok(_) => println!("✓ Partitioned table creation passed"),
            Err(e) => println!("✗ Partitioned table creation failed: {}", e),
        }
    })
    .await;

    if partition_test.is_err() {
        println!("✗ Partitioned table creation timed out");
    }

    println!("=== Debug Test Complete ===");
}
