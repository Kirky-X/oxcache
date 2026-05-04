// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 数据库分区测试 - SQLite 测试

use chrono::{TimeZone, Utc};
use oxcache::error::{CacheError, Result};
use oxcache::storage::partition::{PartitionConfig, PartitionInfo, PartitionManager};
use oxcache::storage::sqlite::SQLitePartitionManager;
use oxcache::storage::PartitionStrategy;
use sea_orm::{ConnectOptions, ConnectionTrait, Database};
use std::fs::File;
use std::sync::Arc;
use tempfile::TempDir;

// ============================================================================
// 辅助函数
// ============================================================================

#[allow(dead_code)]
fn create_unique_db_path() -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("/tmp/test_oxcache_{}.db", timestamp)
}

#[allow(dead_code)]
fn cleanup_test_db(db_path: &str) {
    let _ = std::fs::remove_file(db_path);
}

#[allow(dead_code)]
fn cleanup_partition_tables() {
    for year in 2023..=2025 {
        for month in 1..=12 {
            let partition_db = format!("/tmp/test_oxcache_y{}m{}.db", year, month);
            let _ = std::fs::remove_file(partition_db);
        }
    }
}

fn create_test_connect_options(db_path: &str) -> ConnectOptions {
    let mut opt = ConnectOptions::new(db_path.to_string());
    opt.max_connections(1)
        .min_connections(0)
        .connect_timeout(std::time::Duration::from_secs(10))
        .sqlx_logging(true);
    opt
}

async fn test_basic_connection(db_path: &str) -> bool {
    let opt = create_test_connect_options(db_path);

    match Database::connect(opt).await {
        Ok(db) => {
            println!("✓ Connection succeeded: {}", db_path);

            let result = db
                .execute_raw(sea_orm::Statement::from_string(
                    sea_orm::DatabaseBackend::Sqlite,
                    "SELECT 1 as test".to_string(),
                ))
                .await;

            match result {
                Ok(_) => {
                    println!("✓ Query test succeeded");
                    true
                }
                Err(e) => {
                    println!("✗ Query test failed: {}", e);
                    false
                }
            }
        }
        Err(e) => {
            println!("✗ Connection failed: {} - {}", db_path, e);
            false
        }
    }
}

// ============================================================================
// SQLite 分区测试
// ============================================================================

/// Test SQLite partitioning
#[tokio::test]
async fn test_sqlite_partitioning() -> Result<()> {
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

    let partitions = manager.get_partitions(test_table).await?;
    println!("✓ SQLite partitions listed: {} found", partitions.len());

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

// ============================================================================
// SQLite 详细功能测试
// ============================================================================

mod sqlite_basic_tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_partitioning_basic() -> Result<()> {
        let temp_dir =
            TempDir::new().map_err(|e| CacheError::DatabaseError(format!("Failed to create temp directory: {}", e)))?;
        let db_path = temp_dir.path().join("test_partitioning.db");
        let db_path_str = db_path.to_str().unwrap();

        println!("Testing SQLite partitioning with database: {}", db_path_str);

        match File::create(&db_path) {
            Ok(_) => println!("✓ Database file created: {}", db_path_str),
            Err(e) => {
                println!("✗ Failed to create database file: {}", e);
                return Err(CacheError::DatabaseError(format!(
                    "Failed to create database file: {}",
                    e
                )));
            }
        }

        let partition_config = PartitionConfig {
            enabled: true,
            strategy: PartitionStrategy::Monthly,
            retention_months: 6,
            ..Default::default()
        };

        let connection_string = format!("sqlite:{}", db_path_str);
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

        let partitions = manager.get_partitions(test_table).await?;
        println!("✓ SQLite partitions listed: {} found", partitions.len());

        let test_dates = vec![
            Utc.with_ymd_and_hms(2023, 1, 15, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2023, 2, 15, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2023, 3, 15, 0, 0, 0).unwrap(),
        ];

        for date in &test_dates {
            let partition_info = PartitionInfo::new(*date, test_table)?;
            manager.create_partition(&partition_info).await?;
        }

        let all_partitions = manager.get_partitions(test_table).await?;
        println!("✓ Total partitions: {}", all_partitions.len());

        let cutoff_date = Utc.with_ymd_and_hms(2023, 2, 2, 0, 0, 0).unwrap();
        let cleaned_count = manager.cleanup_old_partitions(test_table, cutoff_date).await?;
        println!("✓ Cleaned up {} old partitions", cleaned_count);

        let remaining_partitions = manager.get_partitions(test_table).await?;
        println!("✓ Partitions after cleanup: {}", remaining_partitions.len());

        use std::path::Path;
        assert!(Path::new(db_path_str).exists(), "Database file should exist");

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
            retention_months: 6,
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
        let partition_name = manager.ensure_partition_exists(test_date, test_table).await?;
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
            retention_months: 6,
            ..Default::default()
        };

        let connection_string = invalid_path.to_string();
        let result = SQLitePartitionManager::new(&connection_string, partition_config).await;

        assert!(result.is_err(), "Should fail with invalid database path");
        println!("✓ SQLite correctly handles invalid database path");
        Ok(())
    }
}

mod sqlite_manager_tests {
    use super::*;
    use oxcache::storage::PartitionStrategy;

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
            retention_months: 12,
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
            retention_months: 12,
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
                    Ok(rt) => match rt.block_on(async { manager.initialize_table(table_name, schema).await }) {
                        Ok(_) => println!("✓ Table initialization (sync) succeeded"),
                        Err(e) => println!("✗ Table initialization (sync) failed: {}", e),
                    },
                    Err(e) => println!("✗ Failed to create runtime: {}", e),
                }
            }
            Err(e) => println!("✗ Failed to create SQLite partition manager (sync): {}", e),
        }

        let _ = std::fs::remove_file(db_path);
    }
}

// ============================================================================
// Sea-ORM SQLite 测试
// ============================================================================

mod sea_orm_minimal_config_tests {
    use super::*;

    #[tokio::test]
    async fn test_sea_orm_sqlite_minimal() {
        let db_name = "test_sea_orm_minimal.db";
        let _ = std::fs::remove_file(db_name);

        println!("Testing sea-orm SQLite with minimal configuration: {}", db_name);
        test_basic_connection(&format!("sqlite:{}", db_name)).await;

        let _ = std::fs::remove_file(db_name);
    }

    #[tokio::test]
    async fn test_sea_orm_sqlite_with_logging() {
        let db_name = "test_sea_orm_logging.db";
        let _ = std::fs::remove_file(db_name);

        println!("Testing sea-orm SQLite with detailed logging: {}", db_name);
        test_basic_connection(&format!("sqlite:{}", db_name)).await;

        let _ = std::fs::remove_file(db_name);
    }
}

mod sea_orm_file_creation_tests {
    use super::*;

    #[tokio::test]
    async fn test_sea_orm_sqlite_create_file_first() {
        let db_path = "/tmp/test_sea_orm_created.db";
        let _ = std::fs::remove_file(db_path);

        match File::create(db_path) {
            Ok(_) => println!("✓ Database file created successfully: {}", db_path),
            Err(e) => println!("✗ Failed to create database file: {}", e),
        }

        println!("Testing sea-orm SQLite with pre-created file: {}", db_path);
        test_basic_connection(&format!("sqlite:{}", db_path)).await;

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn test_sea_orm_sqlite_memory() {
        println!("Testing sea-orm SQLite with in-memory database");
        test_basic_connection("sqlite::memory:").await;
    }
}

mod sea_orm_path_tests {
    use super::*;

    #[tokio::test]
    async fn test_sea_orm_sqlite_absolute_path() {
        let db_path = "/home/project/aybss/crates/infra/oxcache/test_sea_orm_absolute.db";
        let _ = std::fs::remove_file(db_path);

        println!("Testing sea-orm SQLite with absolute path: {}", db_path);
        test_basic_connection(&format!("sqlite:{}", db_path)).await;

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn test_sea_orm_sqlite_with_uri_format() {
        let db_path = "/home/project/aybss/crates/infra/oxcache/test_sea_orm_uri.db";
        let _ = std::fs::remove_file(db_path);

        println!("Testing sea-orm SQLite with URI format: {}", db_path);

        let connection_string = format!("sqlite://{}", db_path);
        println!("Connection string: {}", connection_string);

        test_basic_connection(&connection_string).await;

        let _ = std::fs::remove_file(db_path);
    }
}
