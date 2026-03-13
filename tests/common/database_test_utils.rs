// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 数据库测试工具 - 提供数据库测试的通用工具函数

use chrono::{TimeZone, Utc};
use oxcache::database::partition::{
    PartitionConfig, PartitionInfo, PartitionManager, PartitionStrategy,
};
use oxcache::error::Result;
use std::sync::Arc;
use tempfile::NamedTempFile;

/// 测试配置结构体
///
/// 用于数据库测试的配置信息，包含 PostgreSQL 和 MySQL 连接信息。
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub postgres_url: String,
    pub mysql_url: String,
    pub partitioning_enabled: bool,
    pub strategy: PartitionStrategy,
    pub retention_months: usize,
}

impl TestConfig {
    /// 从环境变量加载测试配置
    ///
    /// 环境变量：
    /// - `TEST_POSTGRES_URL`: PostgreSQL 连接 URL
    /// - `TEST_MYSQL_URL`: MySQL 连接 URL
    pub fn from_file() -> Self {
        Self {
            postgres_url: std::env::var("TEST_POSTGRES_URL").unwrap_or_else(|_| {
                "postgresql://postgres:postgres@localhost:5432/test_db".to_string()
            }),
            mysql_url: std::env::var("TEST_MYSQL_URL")
                .unwrap_or_else(|_| "mysql://root:root@localhost:3306/test_db".to_string()),
            partitioning_enabled: true,
            strategy: PartitionStrategy::Monthly,
            retention_months: 12,
        }
    }
}

/// 创建分区配置
///
/// # Arguments
///
/// * `enabled` - 是否启用分区
/// * `strategy` - 分区策略
/// * `retention` - 保留月数
pub fn create_partition_config(
    enabled: bool,
    strategy: PartitionStrategy,
    retention: usize,
) -> PartitionConfig {
    PartitionConfig {
        enabled,
        strategy,
        retention_months: retention as u32,
        ..Default::default()
    }
}

/// 清理 PostgreSQL 表（使用 Docker 命令）
///
/// 通过 Docker 命令删除指定的表，用于测试清理。
///
/// # Arguments
///
/// * `container_name` - Docker 容器名称
/// * `db_name` - 数据库名称
/// * `user` - 数据库用户
/// * `table_name` - 要删除的表名
///
/// # Returns
///
/// 成功返回 `true`，失败返回 `false`
pub fn cleanup_postgres_table(
    container_name: &str,
    db_name: &str,
    user: &str,
    table_name: &str,
) -> bool {
    if !validate_table_name(table_name) {
        eprintln!("Invalid table name: {}", table_name);
        return false;
    }

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

/// 验证表名格式，防止 SQL 注入
///
/// 表名只能包含字母、数字、下划线，且不能以数字开头。
fn validate_table_name(table_name: &str) -> bool {
    !table_name.is_empty()
        && table_name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && !table_name.chars().next().unwrap().is_ascii_digit()
}

/// 创建临时 SQLite 数据库文件
///
/// 返回临时文件对象和 SQLite 连接字符串。
pub fn create_temp_sqlite_db() -> Result<(NamedTempFile, String)> {
    let temp_file = NamedTempFile::new()?;
    let sqlite_path = format!("sqlite:{}", temp_file.path().display());
    Ok((temp_file, sqlite_path))
}

/// 验证分区创建
///
/// 创建测试分区并验证分区结构。
///
/// # Arguments
///
/// * `manager` - 分区管理器
/// * `table_name` - 表名
/// * `_enabled` - 是否启用（未使用）
/// * `expected_partitions` - 预期分区数量
pub async fn verify_partition_creation<M: PartitionManager>(
    manager: &M,
    table_name: &str,
    _enabled: bool,
    expected_partitions: usize,
) -> Result<Vec<PartitionInfo>> {
    let test_date = Utc::now();
    manager
        .ensure_partition_exists(test_date, table_name)
        .await?;
    println!("✓ Partition created");

    let partitions = manager.get_partitions(table_name).await?;
    println!("✓ Partitions listed: {} found", partitions.len());

    assert!(!partitions.is_empty(), "Should have at least one partition");
    assert!(
        partitions.len() >= expected_partitions,
        "Should have at least {} partitions",
        expected_partitions
    );

    Ok(partitions)
}

/// 验证分区清理
///
/// 创建多个分区并验证清理功能。
///
/// # Arguments
///
/// * `manager` - 分区管理器
/// * `table_name` - 表名
/// * `retention_months` - 保留月数
pub async fn verify_partition_cleanup<M: PartitionManager>(
    manager: &M,
    table_name: &str,
    retention_months: usize,
) -> Result<()> {
    let dates = vec![
        Utc.with_ymd_and_hms(2023, 1, 15, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2023, 2, 15, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2023, 3, 15, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2023, 4, 15, 0, 0, 0).unwrap(),
    ];

    for date in &dates {
        let partition_info = PartitionInfo::new(*date, table_name)?;
        manager.create_partition(&partition_info).await?;
    }

    let partitions_before = manager.get_partitions(table_name).await?;
    println!("Partitions before cleanup: {}", partitions_before.len());

    let mut year = 2023;
    let mut month = 5;
    let retention = retention_months as u32;

    let years_sub = retention / 12;
    let months_sub = retention % 12;

    year -= years_sub as i32;
    if month > months_sub {
        month -= months_sub;
    } else {
        year -= 1;
        month = month + 12 - months_sub;
    }

    let cutoff_date = Utc.with_ymd_and_hms(year, month, 2, 0, 0, 0).unwrap();

    manager
        .cleanup_old_partitions(table_name, cutoff_date)
        .await?;

    let partitions_after = manager.get_partitions(table_name).await?;
    println!("Partitions after cleanup: {}", partitions_after.len());

    assert!(
        partitions_after.len() <= retention_months,
        "Should have at most {} partitions after cleanup",
        retention_months
    );

    Ok(())
}

/// 测试并发分区操作
///
/// 创建多个并发任务测试分区创建。
///
/// # Arguments
///
/// * `manager` - 分区管理器
/// * `table_name` - 表名
pub async fn test_concurrent_partition_operations<M: PartitionManager + 'static>(
    manager: Arc<M>,
    table_name: &str,
) -> Result<()> {
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
            let partition_info = PartitionInfo::new(date, &table_name_clone)?;
            manager_clone.create_partition(&partition_info).await
        });

        tasks.push(task);
    }

    for task in tasks {
        let result = task
            .await
            .map_err(|e| oxcache::error::CacheError::BackendError(e.to_string()))?;
        result?;
    }

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
