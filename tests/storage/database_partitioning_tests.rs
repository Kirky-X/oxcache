// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 数据库分区测试
//
// 这些测试需要外部数据库连接（SQLite）。
// 默认情况下跳过这些测试，除非设置了环境变量：
// - OXCACHE_TEST_DATABASE=1 启用数据库测试
// - 或使用 --features database 特性标志

use chrono::Utc;
use oxcache::error::Result;
use oxcache::storage::partition::{PartitionConfig, PartitionManager};
use oxcache::storage::sqlite::SQLitePartitionManager;
use oxcache::storage::PartitionStrategy;
use std::sync::Arc;

use crate::storage_test_utils::*;

// 检查是否启用数据库测试
fn should_run_database_tests() -> bool {
    std::env::var("OXCACHE_TEST_DATABASE")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}

/// Test SQLite partitioning
#[tokio::test]
async fn test_sqlite_partitioning() -> Result<()> {
    // Skip test if database tests are not enabled
    if !should_run_database_tests() {
        println!("⚠️  Database tests are disabled. Set OXCACHE_TEST_DATABASE=1 to enable.");
        return Ok(());
    }

    let db_path = "sqlite::memory:";

    println!("Testing SQLite partitioning with in-memory database");

    let partition_config = PartitionConfig {
        enabled: true,
        strategy: PartitionStrategy::Monthly,
        retention_months: 6,
        ..Default::default()
    };

    let manager = SQLitePartitionManager::new(db_path, partition_config).await?;

    let test_table = "cache_entries";
    let schema = format!(
        "CREATE TABLE IF NOT EXISTS {} (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            key TEXT NOT NULL,
            value TEXT,
            timestamp TEXT DEFAULT CURRENT_TIMESTAMP
        )",
        test_table
    );

    manager.initialize_table(test_table, &schema).await?;
    println!("✓ SQLite table initialized with partitioning");

    let mut partitions = manager.get_partitions(test_table).await?;
    println!("✓ SQLite partitions listed: {} found", partitions.len());

    // 如果没有分区，先创建一个再检查
    if partitions.is_empty() {
        let test_date = Utc::now();
        let partition_name = manager.ensure_partition_exists(test_date, test_table).await?;
        println!("✓ SQLite partition ensured: {}", partition_name);

        partitions = manager.get_partitions(test_table).await?;
        println!("✓ SQLite partitions listed after creation: {} found", partitions.len());
    }

    for partition in &partitions {
        println!(
            "  Partition: {} ({} to {})",
            partition.name,
            partition.start_date.format("%Y-%m-%d"),
            partition.end_date.format("%Y-%m-%d")
        );
    }

    let test_date = Utc::now();
    let partition_name = manager.ensure_partition_exists(test_date, test_table).await?;
    println!("✓ SQLite partition ensured: {}", partition_name);

    let all_partitions = manager.get_partitions(test_table).await?;
    println!("✓ Total partitions: {}", all_partitions.len());

    println!("✓ SQLite partitioning test completed successfully");

    Ok(())
}

/// Test partition retention cleanup
#[tokio::test]
async fn test_partition_retention() -> Result<()> {
    // Skip test if database tests are not enabled
    if !should_run_database_tests() {
        println!("⚠️  Database tests are disabled. Set OXCACHE_TEST_DATABASE=1 to enable.");
        return Ok(());
    }

    let partition_config = create_partition_config(true, PartitionStrategy::Monthly, 2);

    let db_path = "sqlite::memory:";
    let manager = SQLitePartitionManager::new(db_path, partition_config).await?;

    let test_table = "test_retention_entries";

    let schema = format!(
        "CREATE TABLE IF NOT EXISTS {} (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            key TEXT NOT NULL,
            value TEXT,
            timestamp TEXT DEFAULT CURRENT_TIMESTAMP
        )",
        test_table
    );

    manager.initialize_table(test_table, &schema).await?;

    // Verify partition cleanup with retention policy
    verify_partition_cleanup(&manager, test_table, 2).await?;

    Ok(())
}

/// Test concurrent partition operations
#[tokio::test]
async fn test_concurrent_operations() -> Result<()> {
    // Skip test if database tests are not enabled
    if !should_run_database_tests() {
        println!("⚠️  Database tests are disabled. Set OXCACHE_TEST_DATABASE=1 to enable.");
        return Ok(());
    }

    let partition_config = create_partition_config(true, PartitionStrategy::Monthly, 12);

    let db_path = "sqlite::memory:";
    let manager = Arc::new(SQLitePartitionManager::new(db_path, partition_config).await?);

    let test_table = "test_concurrent_entries";

    let schema = format!(
        "CREATE TABLE IF NOT EXISTS {} (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            key TEXT NOT NULL,
            value TEXT,
            timestamp TEXT DEFAULT CURRENT_TIMESTAMP
        )",
        test_table
    );

    manager.initialize_table(test_table, &schema).await?;

    // Test concurrent partition operations
    test_concurrent_partition_operations(manager, test_table).await?;

    Ok(())
}
