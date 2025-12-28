//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了数据库测试的通用工具函数和设置。

#![allow(dead_code)]

use chrono::{TimeZone, Utc};
use oxcache::database::{PartitionConfig, PartitionInfo, PartitionManager, PartitionStrategy};
use oxcache::error::Result;
use std::sync::Arc;
use tempfile::NamedTempFile;

/// Test configuration structure
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TestConfig {
    pub postgres_url: String,
    pub mysql_url: String,
    pub partitioning_enabled: bool,
    pub strategy: PartitionStrategy,
    pub retention_months: usize,
}

impl TestConfig {
    #[allow(dead_code)]
    pub fn from_file() -> Self {
        Self {
            postgres_url: "postgresql://postgres:postgres@localhost:5432/test_db".to_string(),
            mysql_url: "mysql://root:root@localhost:3306/test_db".to_string(),
            partitioning_enabled: true,
            strategy: PartitionStrategy::Monthly,
            retention_months: 12,
        }
    }
}

/// Create partition configuration
pub fn create_partition_config(
    enabled: bool,
    strategy: PartitionStrategy,
    retention: usize,
) -> PartitionConfig {
    PartitionConfig {
        enabled,
        strategy,
        retention_months: Some(retention as u32),
        ..Default::default()
    }
}

/// Clean up existing table using Docker command (for PostgreSQL)
#[allow(dead_code)]
pub fn cleanup_postgres_table(
    container_name: &str,
    db_name: &str,
    user: &str,
    table_name: &str,
) -> bool {
    let cleanup_result = std::process::Command::new("docker")
        .args([
            "exec",
            container_name,
            "psql",
            "-U",
            user,
            "-d",
            db_name,
            "-c",
            &format!("DROP TABLE IF EXISTS {} CASCADE", table_name),
        ])
        .output();

    match cleanup_result {
        Ok(output) if output.status.success() => {
            println!("✓ Cleaned up existing PostgreSQL table");
            true
        }
        Ok(output) => {
            println!(
                "Warning: Failed to clean up existing table: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            false
        }
        Err(e) => {
            println!("Warning: Could not execute cleanup command: {}", e);
            false
        }
    }
}

/// Create a temporary SQLite database file
pub fn create_temp_sqlite_db() -> Result<(NamedTempFile, String)> {
    let temp_file = NamedTempFile::new()?;
    let sqlite_path = format!("sqlite:{}", temp_file.path().display());
    Ok((temp_file, sqlite_path))
}

/// Common partition verification function
pub async fn verify_partition_creation<M: PartitionManager>(
    manager: &M,
    table_name: &str,
    _enabled: bool,
    expected_partitions: usize,
) -> Result<Vec<PartitionInfo>> {
    // Create a test partition
    let test_date = Utc::now();
    manager
        .ensure_partition_exists(test_date, table_name)
        .await?;
    println!("✓ Partition created");

    // List partitions
    let partitions = manager.get_partitions(table_name).await?;
    println!("✓ Partitions listed: {} found", partitions.len());

    // Verify partition structure
    assert!(!partitions.is_empty(), "Should have at least one partition");
    assert!(
        partitions.len() >= expected_partitions,
        "Should have at least {} partitions",
        expected_partitions
    );

    Ok(partitions)
}

/// Common partition cleanup verification function
pub async fn verify_partition_cleanup<M: PartitionManager>(
    manager: &M,
    table_name: &str,
    retention_months: usize,
) -> Result<()> {
    // Create partitions for different months
    let dates = vec![
        Utc.with_ymd_and_hms(2023, 1, 15, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2023, 2, 15, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2023, 3, 15, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2023, 4, 15, 0, 0, 0).unwrap(),
    ];

    for date in &dates {
        let partition_info = PartitionInfo::new(*date, table_name);
        manager.create_partition(&partition_info).await?;
    }

    // List partitions before cleanup
    let partitions_before = manager.get_partitions(table_name).await?;
    println!("Partitions before cleanup: {}", partitions_before.len());

    // Clean up old partitions
    manager
        .cleanup_old_partitions(table_name, retention_months.try_into().unwrap())
        .await?;

    // List partitions after cleanup
    let partitions_after = manager.get_partitions(table_name).await?;
    println!("Partitions after cleanup: {}", partitions_after.len());

    // Should have only retention_months partitions remaining
    assert!(
        partitions_after.len() <= retention_months,
        "Should have at most {} partitions after cleanup",
        retention_months
    );

    Ok(())
}

/// Common concurrent partition operations test
pub async fn test_concurrent_partition_operations<M: PartitionManager + 'static>(
    manager: Arc<M>,
    table_name: &str,
) -> Result<()> {
    // Create multiple tasks that try to create partitions concurrently
    let mut tasks = vec![];
    let dates = vec![
        Utc.with_ymd_and_hms(2023, 1, 15, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2023, 2, 15, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2023, 3, 15, 0, 0, 0).unwrap(),
    ];

    for date in dates {
        let manager_clone = Arc::clone(&manager);
        let table_name_clone = table_name.to_string();

        let task = tokio::spawn(async move {
            let partition_info = PartitionInfo::new(date, &table_name_clone);
            manager_clone.create_partition(&partition_info).await
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    for task in tasks {
        let result = task
            .await
            .map_err(|e| oxcache::error::CacheError::BackendError(e.to_string()))?;
        result?;
    }

    // Verify all partitions were created
    let partitions: Vec<PartitionInfo> = manager.get_partitions(table_name).await?;
    println!(
        "Concurrent operations completed: {} partitions created",
        partitions.len()
    );

    assert!(
        !partitions.is_empty(),
        "Should have created partitions concurrently"
    );

    Ok(())
}
