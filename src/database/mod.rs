//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 数据库分区管理模块
//!
//! 提供PostgreSQL和MySQL的按月分区功能

use crate::error::{CacheError, Result};
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod common;
pub mod connection_string;
pub mod mysql;
pub mod partition;
pub mod postgresql;
pub mod sqlite;

pub use connection_string::{
    ensure_database_directory, extract_sqlite_path, get_recommended_connection_string,
    is_test_connection_string, normalize_connection_string, validate_connection_string, DbType,
    ParsedConnectionString, ValidationResult,
};
pub use mysql::MySQLPartitionManager;
pub use partition::{PartitionManager, PartitionStrategy};
pub use postgresql::PostgresPartitionManager;
pub use sqlite::SQLitePartitionManager;

/// 数据库类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    SQLite, // 用于测试和开发
}

impl DatabaseType {
    /// 从URL字符串解析数据库类型
    pub fn from_url(url: &str) -> Self {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            DatabaseType::PostgreSQL
        } else if url.starts_with("mysql://") {
            DatabaseType::MySQL
        } else {
            DatabaseType::SQLite
        }
    }
}

/// 分区配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionConfig {
    /// 是否启用分区
    pub enabled: bool,
    /// 分区策略
    pub strategy: PartitionStrategy,
    /// 预创建分区数量
    pub precreate_months: u32,
    /// 保留分区数量（按月）
    pub retention_months: Option<u32>,
    /// 分区表前缀
    pub table_prefix: String,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strategy: PartitionStrategy::Monthly,
            precreate_months: 3,
            retention_months: Some(12),
            table_prefix: "partitioned_".to_string(),
        }
    }
}

/// 分区信息
#[derive(Debug, Clone)]
pub struct PartitionInfo {
    /// 分区名称
    pub name: String,
    /// 分区开始时间
    pub start_date: DateTime<Utc>,
    /// 分区结束时间
    pub end_date: DateTime<Utc>,
    /// 分区表名
    pub table_name: String,
    /// 是否已创建
    pub created: bool,
}

impl PartitionInfo {
    /// 创建新的分区信息
    pub fn new(date: DateTime<Utc>, table_prefix: &str) -> Self {
        let start_date = date
            .with_day(1)
            .expect("Day 1 should exist")
            .with_hour(0)
            .expect("Hour 0 should exist")
            .with_minute(0)
            .expect("Minute 0 should exist")
            .with_second(0)
            .expect("Second 0 should exist");
        let end_date = if date.month() == 12 {
            // 12月，下一年1月
            Utc.with_ymd_and_hms(date.year() + 1, 1, 1, 0, 0, 0)
                .single()
                .expect("January 1st should be a valid date")
        } else {
            // 其他月份，下月1日
            Utc.with_ymd_and_hms(date.year(), date.month() + 1, 1, 0, 0, 0)
                .single()
                .expect("First day of month should be a valid date")
        };

        let name = format!("{}_{}_{:02}", table_prefix, date.year(), date.month());
        let table_name = format!("{}_y{}m{:02}", table_prefix, date.year(), date.month());

        Self {
            name,
            start_date,
            end_date,
            table_name,
            created: false,
        }
    }
}

/// 数据库分区管理器工厂
pub struct PartitionManagerFactory;

impl PartitionManagerFactory {
    /// 创建分区管理器（异步版本）
    pub async fn create_manager(
        db_type: DatabaseType,
        connection_string: &str,
        config: PartitionConfig,
    ) -> Result<Arc<dyn PartitionManager + Send + Sync>> {
        match db_type {
            DatabaseType::PostgreSQL => {
                let manager = PostgresPartitionManager::new(connection_string, config).await?;
                Ok(Arc::new(manager))
            }
            DatabaseType::MySQL => {
                let manager = MySQLPartitionManager::new(connection_string, config).await?;
                Ok(Arc::new(manager))
            }
            DatabaseType::SQLite => {
                // SQLite使用简化实现
                let manager = SQLitePartitionManager::new(connection_string, config).await?;
                Ok(Arc::new(manager))
            }
        }
    }

    /// 创建分区管理器（同步版本，用于非异步环境）
    pub fn create_manager_sync(
        db_type: DatabaseType,
        connection_string: &str,
        config: PartitionConfig,
    ) -> Result<Arc<dyn PartitionManager + Send + Sync>> {
        match db_type {
            DatabaseType::PostgreSQL => {
                // PostgreSQL不支持同步创建，使用异步版本的阻塞调用
                let rt = tokio::runtime::Runtime::new().map_err(|e| {
                    CacheError::DatabaseError(format!("Failed to create runtime: {}", e))
                })?;
                let manager =
                    rt.block_on(PostgresPartitionManager::new(connection_string, config))?;
                Ok(Arc::new(manager))
            }
            DatabaseType::MySQL => {
                // MySQL不支持同步创建，使用异步版本的阻塞调用
                let rt = tokio::runtime::Runtime::new().map_err(|e| {
                    CacheError::DatabaseError(format!("Failed to create runtime: {}", e))
                })?;
                let manager = rt.block_on(MySQLPartitionManager::new(connection_string, config))?;
                Ok(Arc::new(manager))
            }
            DatabaseType::SQLite => {
                // SQLite支持同步创建
                let manager = SQLitePartitionManager::new_sync(connection_string, config)?;
                Ok(Arc::new(manager))
            }
        }
    }
}
