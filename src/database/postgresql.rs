//! PostgreSQL分区管理器实现

use crate::error::{CacheError, Result};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{common::*, PartitionConfig, PartitionInfo, PartitionManager};

/// PostgreSQL分区管理器
pub struct PostgresPartitionManager {
    config: PartitionConfig,
    connection: Arc<Mutex<DatabaseConnection>>,
}

impl PostgresPartitionManager {
    /// 创建新的PostgreSQL分区管理器
    pub async fn new(connection_string: &str, config: PartitionConfig) -> Result<Self> {
        let mut opt = ConnectOptions::new(connection_string.to_string());
        opt.max_connections(10)
            .min_connections(2)
            .connect_timeout(std::time::Duration::from_secs(5))
            .idle_timeout(std::time::Duration::from_secs(8));

        let connection = Database::connect(opt)
            .await
            .map_err(|e| CacheError::DatabaseError(e.to_string()))?;

        Ok(Self {
            config,
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// 创建分区表的主表（使用声明式分区）
    async fn create_partitioned_table(&self, table_name: &str, schema: &str) -> Result<()> {
        let conn = self.connection.lock().await;

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

        println!("Generated partition SQL: {}", partition_sql);

        conn.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            partition_sql,
        ))
        .await?;

        // 创建默认分区
        let default_partition_sql = format!(
            "CREATE TABLE IF NOT EXISTS {}_default PARTITION OF {} DEFAULT",
            table_name, table_name
        );

        conn.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            default_partition_sql,
        ))
        .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl PartitionManager for PostgresPartitionManager {
    async fn initialize_table(&self, table_name: &str, schema: &str) -> Result<()> {
        if self.config.enabled {
            self.create_partitioned_table(table_name, schema).await
        } else {
            let conn = self.connection.lock().await;
            conn.execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                schema.to_string(),
            ))
            .await?;
            Ok(())
        }
    }

    async fn create_partition(&self, partition: &PartitionInfo) -> Result<()> {
        let conn = self.connection.lock().await;

        println!("DEBUG: Creating partition with name: {}", partition.name);
        println!("DEBUG: Partition table_name: {}", partition.table_name);
        println!("DEBUG: Start date: {}", partition.start_date);
        println!("DEBUG: End date: {}", partition.end_date);

        // PostgreSQL分区命名约定 - 使用分区名称中的基础表名
        // Split the partition name to extract the base table name
        let parts: Vec<&str> = partition.name.rsplitn(3, '_').collect();
        println!("DEBUG: Parts after rsplitn: {:?}", parts);
        let base_table_name = if parts.len() >= 3 {
            // For format like "test_cache_entries_2025_12", the base name is "test_cache_entries"
            parts[2..].join("_")
        } else {
            // Fallback to the full name if we can't parse it
            partition.name.clone()
        };
        println!("DEBUG: Base table name: {}", base_table_name);

        let partition_table_name = format!(
            "{}_p{:04}{:02}",
            base_table_name,
            partition.start_date.year(),
            partition.start_date.month()
        );

        println!(
            "DEBUG: Final partition table name: {}",
            partition_table_name
        );

        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} PARTITION OF {} FOR VALUES FROM ('{}') TO ('{}')",
            partition_table_name,
            base_table_name,
            partition.start_date.format("%Y-%m-%d"),
            partition.end_date.format("%Y-%m-%d")
        );

        println!("DEBUG: SQL: {}", sql);

        conn.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            sql,
        ))
        .await?;

        Ok(())
    }

    async fn get_partitions(&self, table_name: &str) -> Result<Vec<PartitionInfo>> {
        let conn = self.connection.lock().await;

        let sql = format!(
            "SELECT 
                child.relname AS partition_name,
                pg_get_expr(child.relpartbound, child.oid) AS partition_range
             FROM pg_inherits 
             JOIN pg_class parent ON pg_inherits.inhparent = parent.oid
             JOIN pg_class child ON pg_inherits.inhrelid = child.oid
             WHERE parent.relname = '{}'",
            table_name
        );

        println!("DEBUG: get_partitions SQL: {}", sql);

        let statement = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        let result = conn.query_all(statement).await?;

        println!("DEBUG: Found {} partition rows", result.len());

        let mut partitions = Vec::new();
        for row in result {
            let partition_name: String = row.try_get("", "partition_name")?;
            let partition_range: Option<String> = row.try_get("", "partition_range")?;

            println!("DEBUG: Partition name: {}", partition_name);
            println!("DEBUG: Partition range: {:?}", partition_range);

            // 解析分区范围信息
            if let Some(range_str) = partition_range {
                if let Some(info) =
                    self.parse_postgres_partition_range(&partition_name, &range_str, table_name)
                {
                    partitions.push(info);
                }
            }
        }

        println!("DEBUG: Total partitions parsed: {}", partitions.len());
        Ok(partitions)
    }

    async fn drop_partition(&self, _table_name: &str, partition_name: &str) -> Result<()> {
        let conn = self.connection.lock().await;

        let sql = format!("DROP TABLE IF EXISTS {}", partition_name);
        println!("DEBUG: Executing drop SQL: {}", sql);

        let result = conn
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                sql,
            ))
            .await;

        match &result {
            Ok(_) => println!("DEBUG: Successfully dropped partition: {}", partition_name),
            Err(e) => println!("DEBUG: Failed to drop partition {}: {}", partition_name, e),
        }

        result?;
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
        println!("DEBUG: Parsing partition range: {}", range_str);

        // More flexible regex to match various PostgreSQL date formats with optional time and timezone
        let re = regex::Regex::new(r"FROM\s+\('(\d{4}-\d{2}-\d{2})(?:[^)]+)?'\)\s+TO\s+\('(\d{4}-\d{2}-\d{2})(?:[^)]+)?'\)")
            .ok()?;

        if let Some(captures) = re.captures(range_str) {
            let start_date_str = captures.get(1)?.as_str();
            let end_date_str = captures.get(2)?.as_str();

            println!("DEBUG: Parsed start date: {}", start_date_str);
            println!("DEBUG: Parsed end date: {}", end_date_str);

            // Parse the dates properly
            let start_date = NaiveDate::parse_from_str(start_date_str, "%Y-%m-%d")
                .ok()?
                .and_hms_opt(0, 0, 0)?
                .and_utc();

            let end_date = NaiveDate::parse_from_str(end_date_str, "%Y-%m-%d")
                .ok()?
                .and_hms_opt(0, 0, 0)?
                .and_utc();

            println!("DEBUG: Parsed start date as DateTime: {}", start_date);
            println!("DEBUG: Parsed end date as DateTime: {}", end_date);

            // Create PartitionInfo using the table name from the partition
            let mut info = PartitionInfo::new(start_date, table_name);
            info.name = partition_name.to_string();
            info.start_date = start_date;
            info.end_date = end_date;
            info.created = true;

            println!("DEBUG: Successfully created PartitionInfo");
            return Some(info);
        }

        println!("DEBUG: Failed to parse partition range");
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
