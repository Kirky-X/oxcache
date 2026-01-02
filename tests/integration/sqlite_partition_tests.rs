//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! SQLite分区测试

use chrono::{TimeZone, Utc};
use oxcache::database::sqlite::SQLitePartitionManager;
use oxcache::database::{PartitionConfig, PartitionInfo, PartitionManager, PartitionStrategy};
use oxcache::error::{CacheError, Result};
use std::fs::File;
use tempfile::TempDir;

fn create_unique_db_path() -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("/tmp/test_oxcache_{}.db", timestamp)
}

fn cleanup_test_db(db_path: &str) {
    let _ = std::fs::remove_file(db_path);
}

fn cleanup_partition_tables() {
    for year in 2023..=2025 {
        for month in 1..=12 {
            let partition_db = format!("/tmp/test_oxcache_y{}m{}.db", year, month);
            let _ = std::fs::remove_file(partition_db);
        }
    }
}

mod basic_functionality_tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_partitioning_basic() -> Result<()> {
        let db_path = create_unique_db_path();
        println!("Testing SQLite partitioning with database: {}", db_path);

        cleanup_partition_tables();
        let _ = std::fs::remove_file(&db_path);

        match File::create(&db_path) {
            Ok(_) => println!("✓ Database file pre-created: {}", db_path),
            Err(e) => {
                println!("✗ Failed to pre-create database file: {}", e);
                return Err(CacheError::DatabaseError(format!(
                    "Failed to create database file: {}",
                    e
                )));
            }
        }

        let partition_config = PartitionConfig {
            enabled: true,
            strategy: PartitionStrategy::Monthly,
            retention_months: Some(6),
            ..Default::default()
        };

        let connection_string = format!("sqlite:{}", db_path);
        let manager = SQLitePartitionManager::new(&connection_string, partition_config).await?;

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

        let initial_partitions = manager.get_partitions(test_table).await?;
        println!("✓ Initial partitions: {}", initial_partitions.len());
        for (i, p) in initial_partitions.iter().enumerate() {
            println!("  Partition {}: {}", i, p.name);
        }

        let partitions = manager.get_partitions(test_table).await?;
        println!("✓ SQLite partitions listed: {} found", partitions.len());
        assert!(!partitions.is_empty(), "Should have at least one partition");

        let test_dates = vec![
            Utc.with_ymd_and_hms(2023, 1, 15, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2023, 2, 15, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2023, 3, 15, 0, 0, 0).unwrap(),
        ];

        for date in &test_dates {
            let partition_info = PartitionInfo::new(*date, test_table);
            manager.create_partition(&partition_info).await?;
        }

        let all_partitions = manager.get_partitions(test_table).await?;
        println!("✓ Total partitions: {}", all_partitions.len());

        let cleaned_count = manager.cleanup_old_partitions(test_table, 2).await?;
        println!("✓ Cleaned up {} old partitions", cleaned_count);

        let remaining_partitions = manager.get_partitions(test_table).await?;
        println!("✓ Partitions after cleanup: {}", remaining_partitions.len());

        use std::path::Path;
        assert!(Path::new(&db_path).exists(), "Database file should exist");

        cleanup_test_db(&db_path);
        println!("✓ SQLite partitioning test completed");
        Ok(())
    }

    #[tokio::test]
    async fn test_sqlite_without_partitioning() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_no_partition.db");

        match File::create(&db_path) {
            Ok(_) => println!("✓ Database file pre-created: {}", db_path.display()),
            Err(e) => {
                println!("✗ Failed to pre-create database file: {}", e);
                return Err(CacheError::DatabaseError(format!(
                    "Failed to create database file: {}",
                    e
                )));
            }
        }

        let partition_config = PartitionConfig {
            enabled: false,
            strategy: PartitionStrategy::Monthly,
            retention_months: Some(6),
            ..Default::default()
        };

        let connection_string = format!("sqlite:{}", db_path.to_str().unwrap());
        let manager = SQLitePartitionManager::new(&connection_string, partition_config).await?;

        let test_table = "simple_cache";
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
        println!("✓ SQLite table initialized without partitioning");

        let test_date = Utc::now();
        let partition_name = manager
            .ensure_partition_exists(test_date, test_table)
            .await?;
        println!(
            "✓ Partition creation called (partitioning disabled): {}",
            partition_name
        );

        let partitions = manager.get_partitions(test_table).await?;
        println!("✓ Partitions listed: {} found", partitions.len());
        Ok(())
    }

    #[tokio::test]
    async fn test_sqlite_error_handling() -> Result<()> {
        let invalid_path = "/invalid/path/that/does/not/exist/test.db";

        let partition_config = PartitionConfig {
            enabled: true,
            strategy: PartitionStrategy::Monthly,
            retention_months: Some(6),
            ..Default::default()
        };

        let connection_string = invalid_path.to_string();
        let result = SQLitePartitionManager::new(&connection_string, partition_config).await;

        assert!(result.is_err(), "Should fail with invalid database path");
        println!("✓ SQLite correctly handles invalid database path");
        Ok(())
    }
}

mod manager_tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_partition_manager_basic() {
        let db_path = "/tmp/test_sqlite_partition_manager.db";
        let _ = std::fs::remove_file(db_path);

        match File::create(db_path) {
            Ok(_) => println!("✓ Database file created: {}", db_path),
            Err(e) => {
                println!("✗ Failed to create database file: {}", e);
                return;
            }
        }

        println!("Testing SQLite partition manager with: {}", db_path);

        let config = PartitionConfig {
            enabled: true,
            strategy: PartitionStrategy::Monthly,
            precreate_months: 3,
            retention_months: Some(12),
            table_prefix: "partitioned_".to_string(),
        };

        let connection_string = format!("sqlite:{}", db_path);

        match SQLitePartitionManager::new(&connection_string, config).await {
            Ok(manager) => {
                println!("✓ SQLite partition manager created successfully!");

                let table_name = "test_cache";
                let schema = r#"
                    CREATE TABLE IF NOT EXISTS test_cache (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        key TEXT NOT NULL UNIQUE,
                        value TEXT NOT NULL,
                        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                    )
                "#;

                match manager.initialize_table(table_name, schema).await {
                    Ok(_) => println!("✓ Table initialization succeeded"),
                    Err(e) => println!("✗ Table initialization failed: {}", e),
                }

                let now = Utc::now();
                match manager.ensure_partition_exists(now, table_name).await {
                    Ok(partition_name) => {
                        println!("✓ Partition creation succeeded: {}", partition_name);

                        match manager.get_partitions(table_name).await {
                            Ok(partitions) => {
                                println!("✓ Get partitions succeeded: {} found", partitions.len());
                            }
                            Err(e) => println!("✗ Get partitions failed: {}", e),
                        }
                    }
                    Err(e) => println!("✗ Partition creation failed: {}", e),
                }
            }
            Err(e) => println!("✗ Failed to create SQLite partition manager: {}", e),
        }

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_sqlite_partition_manager_sync() {
        let db_path = "/tmp/test_sqlite_partition_manager_sync.db";
        let _ = std::fs::remove_file(db_path);

        match File::create(db_path) {
            Ok(_) => println!("✓ Database file created: {}", db_path),
            Err(e) => {
                println!("✗ Failed to create database file: {}", e);
                return;
            }
        }

        println!("Testing SQLite partition manager (sync) with: {}", db_path);

        let config = PartitionConfig {
            enabled: true,
            strategy: PartitionStrategy::Monthly,
            precreate_months: 3,
            retention_months: Some(12),
            table_prefix: "partitioned_".to_string(),
        };

        let connection_string = format!("sqlite:{}", db_path);

        match SQLitePartitionManager::new_sync(&connection_string, config) {
            Ok(manager) => {
                println!("✓ SQLite partition manager (sync) created successfully!");

                let table_name = "test_cache_sync";
                let schema = r#"
                    CREATE TABLE IF NOT EXISTS test_cache_sync (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        key TEXT NOT NULL UNIQUE,
                        value TEXT NOT NULL,
                        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                    )
                "#;

                match tokio::runtime::Runtime::new() {
                    Ok(rt) => {
                        match rt
                            .block_on(async { manager.initialize_table(table_name, schema).await })
                        {
                            Ok(_) => println!("✓ Table initialization (sync) succeeded"),
                            Err(e) => println!("✗ Table initialization (sync) failed: {}", e),
                        }
                    }
                    Err(e) => println!("✗ Failed to create runtime: {}", e),
                }
            }
            Err(e) => println!("✗ Failed to create SQLite partition manager (sync): {}", e),
        }

        let _ = std::fs::remove_file(db_path);
    }
}
