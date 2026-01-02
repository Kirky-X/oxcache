//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 跨数据库集成测试

use chrono::Utc;
use oxcache::database::{
    DatabaseType, PartitionConfig, PartitionInfo, PartitionManager, PartitionManagerFactory,
};
use std::sync::Arc;

async fn setup_partition_manager(
    database_url: &str,
) -> Result<Arc<dyn PartitionManager + Send + Sync>, Box<dyn std::error::Error>> {
    let db_type = DatabaseType::from_url(database_url);
    let config = PartitionConfig {
        table_prefix: "test_".to_string(),
        ..Default::default()
    };

    let manager = PartitionManagerFactory::create_manager(db_type, database_url, config).await?;

    Ok(manager)
}

#[tokio::test]
async fn test_cross_database_partition_consistency() -> Result<(), Box<dyn std::error::Error>> {
    // 检查是否启用了数据库集成测试
    if std::env::var("DATABASE_INTEGRATION_TEST_ENABLED").is_err() {
        println!("数据库集成测试未启用，跳过测试");
        return Ok(());
    }
    println!("=== Cross-Database Partition Consistency Test ===");

    let test_configs = [
        ("MySQL", "mysql://root:password@localhost:3307/oxcache_test"),
        (
            "PostgreSQL",
            "postgres://user:password@localhost:5433/crawlrs_db",
        ),
        (
            "SQLite",
            "sqlite:///home/project/aybss/crates/infra/oxcache/test_cross_db.db",
        ),
    ];

    let test_table = "cross_test_entries";
    let test_date = Utc::now();
    let mut results = Vec::new();

    for (db_name, url) in test_configs {
        println!("\nTesting {}...", db_name);

        let manager = setup_partition_manager(url).await?;

        // Generate database-specific schema
        let schema = match db_name {
            "MySQL" => format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    id INT AUTO_INCREMENT,
                    `key` VARCHAR(255) NOT NULL,
                    value TEXT,
                    created_at DATE DEFAULT (CURDATE()),
                    PRIMARY KEY (id, created_at)
                )",
                test_table
            ),
            "PostgreSQL" => format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    id SERIAL,
                    key VARCHAR(255) NOT NULL,
                    value TEXT,
                    created_at DATE DEFAULT CURRENT_DATE,
                    PRIMARY KEY (id, created_at)
                )",
                test_table
            ),
            "SQLite" => format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    key TEXT NOT NULL,
                    value TEXT,
                    created_at DATE DEFAULT (date('now'))
                )",
                test_table
            ),
            _ => return Err("Unsupported database".into()),
        };

        manager.initialize_table(test_table, &schema).await?;
        println!("✓ {} table initialized", db_name);

        let partition_info = PartitionInfo::new(test_date, test_table);
        manager.create_partition(&partition_info).await?;
        println!(
            "✓ {} partition created: {}",
            db_name, partition_info.table_name
        );

        results.push((db_name, partition_info.table_name.clone()));

        let partitions = manager.get_partitions(test_table).await?;
        let found_partition = partitions
            .iter()
            .find(|p| p.table_name == partition_info.table_name);
        assert!(
            found_partition.is_some(),
            "Partition not found in {} database",
            db_name
        );
        println!("✓ {} partition verification passed", db_name);
    }

    println!("\n=== Partition Naming Consistency Check ===");
    let first_partition = &results[0].1;
    for (db_name, partition_name) in &results {
        if partition_name != first_partition {
            println!(
                "⚠ {} partition name differs: {} vs {}",
                db_name, partition_name, first_partition
            );
        } else {
            println!(
                "✓ {} partition name consistent: {}",
                db_name, partition_name
            );
        }
    }

    println!("\n✓ Cross-database partition consistency test completed");
    Ok(())
}

#[tokio::test]
async fn test_cross_database_partition_cleanup() -> Result<(), Box<dyn std::error::Error>> {
    // 检查是否启用了数据库集成测试
    if std::env::var("DATABASE_INTEGRATION_TEST_ENABLED").is_err() {
        println!("数据库集成测试未启用，跳过测试");
        return Ok(());
    }
    println!("\n=== Cross-Database Partition Cleanup Test ===");

    let test_configs = vec![
        ("MySQL", "mysql://root:password@localhost:3307/oxcache_test"),
        (
            "PostgreSQL",
            "postgres://user:password@localhost:5433/crawlrs_db",
        ),
        (
            "SQLite",
            "sqlite:///home/project/aybss/crates/infra/oxcache/test_cross_cleanup.db",
        ),
    ];

    let test_table = "cross_cleanup_entries";

    for (db_name, url) in test_configs {
        println!("\nTesting {} cleanup...", db_name);

        let manager = setup_partition_manager(url).await?;

        // Generate database-specific schema for cleanup test
        let schema = match db_name {
            "MySQL" => format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    id INT AUTO_INCREMENT,
                    `key` VARCHAR(255) NOT NULL,
                    value TEXT,
                    created_at DATE DEFAULT (CURDATE()),
                    PRIMARY KEY (id, created_at)
                )",
                test_table
            ),
            "PostgreSQL" => format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    id SERIAL,
                    key VARCHAR(255) NOT NULL,
                    value TEXT,
                    created_at DATE DEFAULT CURRENT_DATE,
                    PRIMARY KEY (id, created_at)
                )",
                test_table
            ),
            "SQLite" => format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    key TEXT NOT NULL,
                    value TEXT,
                    created_at DATE DEFAULT (date('now'))
                )",
                test_table
            ),
            _ => return Err("Unsupported database".into()),
        };

        manager.initialize_table(test_table, &schema).await?;
        println!("✓ {} table initialized", db_name);

        // Create partitions for different months
        let old_date = Utc::now() - chrono::Duration::days(100); // ~3 months ago
        let recent_date = Utc::now() - chrono::Duration::days(30); // ~1 month ago

        let old_partition = PartitionInfo::new(old_date, test_table);
        let recent_partition = PartitionInfo::new(recent_date, test_table);

        manager.create_partition(&old_partition).await?;
        manager.create_partition(&recent_partition).await?;
        println!("✓ {} partitions created", db_name);

        let partitions_before = manager.get_partitions(test_table).await?;
        println!(
            "✓ {} has {} partitions before cleanup",
            db_name,
            partitions_before.len()
        );

        // Test cleanup with 2 months retention
        let cleaned_count = manager.cleanup_old_partitions(test_table, 2).await?;
        println!("✓ {} cleaned up {} old partitions", db_name, cleaned_count);

        let partitions_after = manager.get_partitions(test_table).await?;
        println!(
            "✓ {} has {} partitions after cleanup",
            db_name,
            partitions_after.len()
        );

        // Verify that recent partition still exists
        let recent_exists = partitions_after
            .iter()
            .any(|p| p.table_name == recent_partition.table_name);
        assert!(
            recent_exists,
            "Recent partition should still exist after cleanup in {} database",
            db_name
        );
        println!("✓ {} recent partition preserved", db_name);
    }

    println!("\n✓ Cross-database partition cleanup test completed");
    Ok(())
}
