//! PostgreSQL分区管理器实现

//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了PostgreSQL分区管理器的实现。

use crate::error::{CacheError, Result};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::{common::*, PartitionConfig, PartitionInfo, PartitionManager};

/// 连接池统计信息
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    pub active_connections: u32,
    pub idle_connections: u32,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_acquire_ms: f64,
    pub last_acquire: Option<Instant>,
}

impl PoolStats {
    pub fn utilization_rate(&self) -> f64 {
        if self.max_connections > 0 {
            self.active_connections as f64 / self.max_connections as f64
        } else {
            0.0
        }
    }
}

/// PostgreSQL分区管理器
pub struct PostgresPartitionManager {
    config: PartitionConfig,
    connection: Arc<DatabaseConnection>,
    pool_stats: Arc<Mutex<PoolStats>>,
}

impl PostgresPartitionManager {
    /// 创建新的PostgreSQL分区管理器
    pub async fn new(connection_string: &str, config: PartitionConfig) -> Result<Self> {
        let mut opt = ConnectOptions::new(connection_string.to_string());
        opt.max_connections(10)
            .min_connections(2)
            .connect_timeout(Duration::from_secs(5))
            .idle_timeout(Duration::from_secs(8))
            .max_lifetime(Duration::from_secs(1800))
            .acquire_timeout(Duration::from_secs(10));

        let start = Instant::now();
        let connection = Database::connect(opt)
            .await
            .map_err(|e| CacheError::DatabaseError(format!(
                "Failed to connect to PostgreSQL: {}. Please check your connection string and ensure the database server is running.",
                e
            )))?;

        let acquire_duration = start.elapsed();
        info!(
            "PostgreSQL connection established in {:?}",
            acquire_duration
        );

        if acquire_duration > Duration::from_secs(3) {
            warn!(
                "PostgreSQL connection took longer than expected: {:?}",
                acquire_duration
            );
        }

        Ok(Self {
            config,
            connection: Arc::new(connection),
            pool_stats: Arc::new(Mutex::new(PoolStats {
                active_connections: 1,
                idle_connections: 1,
                max_connections: 10,
                min_connections: 2,
                connection_acquire_ms: acquire_duration.as_secs_f64() * 1000.0,
                last_acquire: Some(Instant::now()),
            })),
        })
    }

    /// 获取连接池统计信息
    pub async fn get_pool_stats(&self) -> PoolStats {
        self.pool_stats.lock().await.clone()
    }

    /// 验证 PostgreSQL 标识符，防止 SQL 注入
    fn validate_identifier(&self, identifier: &str) -> Result<()> {
        if identifier.is_empty() {
            return Err(CacheError::DatabaseError(
                "Identifier cannot be empty".to_string(),
            ));
        }

        // PostgreSQL 标识符规则：只能包含字母、数字、下划线
        if !identifier.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(CacheError::DatabaseError(format!(
                "Invalid identifier '{}': only alphanumeric characters and underscores are allowed",
                identifier
            )));
        }

        // 检查长度限制（PostgreSQL 最大 63 字符）
        if identifier.len() > 63 {
            return Err(CacheError::DatabaseError(
                "Identifier exceeds maximum length of 63 characters".to_string(),
            ));
        }

        // 检查是否以数字开头（PostgreSQL 不允许）
        if identifier
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            return Err(CacheError::DatabaseError(
                "Identifier cannot start with a digit".to_string(),
            ));
        }

        // 检查是否是 PostgreSQL 保留关键字
        const RESERVED_KEYWORDS: &[&str] = &[
            "select",
            "from",
            "where",
            "insert",
            "update",
            "delete",
            "create",
            "drop",
            "alter",
            "table",
            "index",
            "view",
            "grant",
            "revoke",
            "commit",
            "rollback",
            "union",
            "intersect",
            "except",
            "join",
            "inner",
            "outer",
            "left",
            "right",
            "order",
            "by",
            "group",
            "having",
            "limit",
            "offset",
            "distinct",
            "all",
            "exists",
            "in",
            "between",
            "like",
            "is",
            "null",
            "and",
            "or",
            "not",
            "true",
            "false",
            "case",
            "when",
            "then",
            "else",
            "end",
        ];

        let lower_identifier = identifier.to_lowercase();
        if RESERVED_KEYWORDS.contains(&lower_identifier.as_str()) {
            return Err(CacheError::DatabaseError(format!(
                "Identifier '{}' is a reserved keyword and cannot be used",
                identifier
            )));
        }

        Ok(())
    }

    /// 验证连接健康状态
    pub async fn health_check(&self) -> bool {
        if let Err(e) = self.ping().await {
            warn!("PostgreSQL health check failed: {}", e);
            return false;
        }
        true
    }

    /// Ping数据库以验证连接
    async fn ping(&self) -> Result<()> {
        let conn = self.connection.as_ref();
        conn.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT 1".to_string(),
        ))
        .await
        .map_err(|e| CacheError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 重新建立连接（用于恢复）
    pub async fn reconnect(&mut self, connection_string: &str) -> Result<()> {
        info!("Attempting to reconnect to PostgreSQL...");

        let mut opt = ConnectOptions::new(connection_string.to_string());
        opt.max_connections(10)
            .min_connections(2)
            .connect_timeout(Duration::from_secs(10))
            .idle_timeout(Duration::from_secs(8));

        let start = Instant::now();
        let connection = Database::connect(opt).await.map_err(|e| {
            CacheError::DatabaseError(format!(
                "Failed to reconnect to PostgreSQL: {}. Please check your database server.",
                e
            ))
        })?;

        let acquire_duration = start.elapsed();
        info!(
            "PostgreSQL reconnection established in {:?}",
            acquire_duration
        );

        self.connection = Arc::new(connection);

        let mut stats = self.pool_stats.lock().await;
        stats.connection_acquire_ms = acquire_duration.as_secs_f64() * 1000.0;
        stats.last_acquire = Some(Instant::now());

        Ok(())
    }

    /// 创建分区表的主表（使用声明式分区）
    async fn create_partitioned_table(&self, table_name: &str, schema: &str) -> Result<()> {
        let conn = self.connection.as_ref();

        // 修改schema为分区表格式
        let partition_schema = schema;

        // 查找表结构中的分区列
        let partition_column = if schema.contains("created_at") {
            "created_at"
        } else if schema.contains("timestamp") {
            "timestamp"
        } else if schema.contains("date_column") {
            "date_column"
        } else {
            "created_at"
        };

        // 移除末尾的右括号并添加分区子句
        let partition_sql = if partition_schema.trim().ends_with(')') {
            format!(
                "{}) PARTITION BY RANGE ({})",
                partition_schema.trim().trim_end_matches(')'),
                partition_column
            )
        } else {
            format!(
                "{}) PARTITION BY RANGE ({})",
                partition_schema, partition_column
            )
        };

        debug!("Generated partition SQL: {}", partition_sql);

        conn.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            partition_sql,
        ))
            .await
            .map_err(|e| {
                CacheError::DatabaseError(format!(
                    "Failed to create partitioned table: {}. Please check if the table schema is valid.",
                    e
                ))
            })?;

        // 创建默认分区
        let default_partition_sql = format!(
            "CREATE TABLE IF NOT EXISTS {}_default PARTITION OF {} DEFAULT",
            table_name, table_name
        );

        conn.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            default_partition_sql,
        ))
        .await
        .map_err(|e| {
            CacheError::DatabaseError(format!("Failed to create default partition: {}", e))
        })?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl PartitionManager for PostgresPartitionManager {
    async fn initialize_table(&self, table_name: &str, schema: &str) -> Result<()> {
        if self.config.enabled {
            self.create_partitioned_table(table_name, schema).await
        } else {
            let conn = self.connection.as_ref();
            conn.execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                schema.to_string(),
            ))
            .await
            .map_err(|e| {
                CacheError::DatabaseError(format!(
                    "Failed to initialize table: {}. Please check the table schema.",
                    e
                ))
            })?;
            Ok(())
        }
    }

    async fn create_partition(&self, partition: &PartitionInfo) -> Result<()> {
        let conn = self.connection.as_ref();

        debug!("Creating partition with name: {}", partition.name);
        debug!("Partition table_name: {}", partition.table_name);

        let parts: Vec<&str> = partition.name.rsplitn(3, '_').collect();
        let base_table_name = if parts.len() >= 3 {
            parts[2..].join("_")
        } else {
            partition.name.clone()
        };

        let partition_table_name = format!(
            "{}_p{:04}{:02}",
            base_table_name,
            partition.start_date.year(),
            partition.start_date.month()
        );

        debug!("Final partition table name: {}", partition_table_name);

        // 验证所有标识符以防止 SQL 注入
        self.validate_identifier(&base_table_name)?;
        self.validate_identifier(&partition_table_name)?;

        // 使用双引号包裹标识符以避免关键字冲突
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS \"{}\" PARTITION OF \"{}\" FOR VALUES FROM ('{}') TO ('{}')",
            partition_table_name,
            base_table_name,
            partition.start_date.format("%Y-%m-%d"),
            partition.end_date.format("%Y-%m-%d")
        );

        debug!("SQL: {}", sql);

        conn.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            sql,
        ))
        .await
        .map_err(|e| {
            CacheError::DatabaseError(format!(
                "Failed to create partition {}: {}",
                partition_table_name, e
            ))
        })?;

        Ok(())
    }

    async fn get_partitions(&self, table_name: &str) -> Result<Vec<PartitionInfo>> {
        let conn = self.connection.as_ref();

        let sql = "SELECT
                child.relname AS partition_name,
                pg_get_expr(child.relpartbound, child.oid) AS partition_range
             FROM pg_inherits
             JOIN pg_class parent ON pg_inherits.inhparent = parent.oid
             JOIN pg_class child ON pg_inherits.inhrelid = child.oid
             WHERE parent.relname = $1"
            .to_string();

        let statement = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        let result = conn.query_all(statement).await.map_err(|e| {
            CacheError::DatabaseError(format!(
                "Failed to get partitions: {}. Please check if the table exists.",
                e
            ))
        })?;

        let mut partitions = Vec::new();
        for row in result {
            let partition_name: String = row.try_get("", "partition_name")?;
            let partition_range: Option<String> = row.try_get("", "partition_range")?;

            if let Some(range_str) = partition_range {
                if let Some(info) =
                    self.parse_postgres_partition_range(&partition_name, &range_str, table_name)
                {
                    partitions.push(info);
                }
            }
        }

        Ok(partitions)
    }

    async fn drop_partition(&self, _table_name: &str, partition_name: &str) -> Result<()> {
        let conn = self.connection.as_ref();

        // 验证标识符以防止 SQL 注入
        self.validate_identifier(partition_name)?;

        // 使用双引号包裹标识符以避免关键字冲突
        let sql = format!("DROP TABLE IF EXISTS \"{}\"", partition_name);
        debug!("Executing drop SQL: {}", sql);

        conn.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            sql,
        ))
        .await
        .map_err(|e| {
            CacheError::DatabaseError(format!(
                "Failed to drop partition {}: {}",
                partition_name, e
            ))
        })?;

        debug!("Successfully dropped partition: {}", partition_name);
        Ok(())
    }

    async fn cleanup_old_partitions(
        &self,
        table_name: &str,
        retention_months: u32,
    ) -> Result<usize> {
        common_cleanup_old_partitions(
            self,
            table_name,
            retention_months,
            &self.config,
            |m, t| m.get_partitions(t),
            |m, t, p| m.drop_partition(t, p),
        )
        .await
    }

    async fn ensure_partition_exists(
        &self,
        date: DateTime<Utc>,
        table_name: &str,
    ) -> Result<String> {
        let partition = PartitionInfo::new(date, table_name);

        // 检查分区是否已存在
        let existing_partitions = self.get_partitions(table_name).await?;
        let exists = existing_partitions.iter().any(|p| p.name == partition.name);

        if !exists {
            self.create_partition(&partition).await?;
        }

        Ok(partition.table_name)
    }

    async fn precreate_partitions(&self, table_name: &str, months_ahead: u32) -> Result<()> {
        common_precreate_partitions(self, table_name, months_ahead, &self.config, |m, d, t| {
            PartitionManager::ensure_partition_exists(m, d, t)
        })
        .await
    }
}

impl PostgresPartitionManager {
    fn parse_postgres_partition_range(
        &self,
        partition_name: &str,
        range_str: &str,
        table_name: &str,
    ) -> Option<PartitionInfo> {
        // PostgreSQL分区范围格式: FOR VALUES FROM ('2024-01-01') TO ('2024-02-01')
        debug!("Parsing partition range for: {}", partition_name);

        // More flexible regex to match various PostgreSQL date formats with optional time and timezone
        let re = regex::Regex::new(r"FROM\s+\('(\d{4}-\d{2}-\d{2})(?:[^)]+)?'\)\s+TO\s+\('(\d{4}-\d{2}-\d{2})(?:[^)]+)?'\)")
            .ok()?;

        if let Some(captures) = re.captures(range_str) {
            let start_date_str = captures.get(1)?.as_str();
            let end_date_str = captures.get(2)?.as_str();

            debug!("Parsed start date: {}", start_date_str);
            debug!("Parsed end date: {}", end_date_str);

            // Parse the dates properly
            let start_date = NaiveDate::parse_from_str(start_date_str, "%Y-%m-%d")
                .ok()?
                .and_hms_opt(0, 0, 0)?
                .and_utc();

            let end_date = NaiveDate::parse_from_str(end_date_str, "%Y-%m-%d")
                .ok()?
                .and_hms_opt(0, 0, 0)?
                .and_utc();

            debug!("Parsed start date as DateTime: {}", start_date);
            debug!("Parsed end date as DateTime: {}", end_date);

            // Create PartitionInfo using the table name from the partition
            let mut info = PartitionInfo::new(start_date, table_name);
            info.name = partition_name.to_string();
            info.start_date = start_date;
            info.end_date = end_date;
            info.created = true;

            debug!("Successfully created PartitionInfo");
            return Some(info);
        }

        debug!("Failed to parse partition range");
        None
    }
}

impl PartitionManagerExt for PostgresPartitionManager {
    async fn cleanup_old_partitions(
        &self,
        table_name: &str,
        retention_months: u32,
    ) -> Result<usize> {
        common_cleanup_old_partitions(
            self,
            table_name,
            retention_months,
            &self.config,
            |m, t| m.get_partitions(t),
            |m, t, p| m.drop_partition(t, p),
        )
        .await
    }

    async fn ensure_partition_exists(
        &self,
        date: DateTime<Utc>,
        table_name: &str,
    ) -> Result<String> {
        let partition = PartitionInfo::new(date, table_name);

        // 检查分区是否已存在
        let existing_partitions = self.get_partitions(table_name).await?;
        let exists = existing_partitions.iter().any(|p| p.name == partition.name);

        if !exists {
            self.create_partition(&partition).await?;
        }

        Ok(partition.table_name)
    }

    fn get_config(&self) -> &PartitionConfig {
        &self.config
    }
}
