//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了SQLite分区管理器的实现。

use super::{
    common::*,
    connection_string::{ensure_database_directory, normalize_connection_string},
    PartitionConfig, PartitionInfo, PartitionManager,
};
use crate::error::{CacheError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Timelike, Utc};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use std::sync::Arc;
use tracing::debug;

pub struct SQLitePartitionManager {
    config: PartitionConfig,
    connection: Arc<DatabaseConnection>,
}

impl SQLitePartitionManager {
    /// 验证SQL标识符是否安全（防止SQL注入）
    /// SQLite标识符规则：只能包含字母、数字、下划线，且不能以数字开头
    fn validate_identifier(&self, identifier: &str) -> Result<()> {
        if identifier.is_empty() {
            return Err(CacheError::DatabaseError(
                "Identifier cannot be empty".to_string(),
            ));
        }

        // 检查长度限制
        if identifier.len() > 128 {
            return Err(CacheError::DatabaseError(
                "Identifier exceeds maximum length of 128 characters".to_string(),
            ));
        }

        let mut chars = identifier.chars();
        let first = chars
            .next()
            .ok_or_else(|| CacheError::DatabaseError("Invalid identifier: empty".to_string()))?;

        // 第一个字符必须是字母或下划线
        if !first.is_ascii_alphabetic() && first != '_' {
            return Err(CacheError::DatabaseError(format!(
                "Invalid identifier '{}': must start with a letter or underscore",
                identifier
            )));
        }

        // 其他字符只能是字母、数字或下划线
        for c in chars {
            if !c.is_ascii_alphanumeric() && c != '_' {
                return Err(CacheError::DatabaseError(format!(
                    "Invalid identifier '{}': only alphanumeric characters and underscores are allowed",
                    identifier
                )));
            }
        }

        // 检查是否是SQLite保留关键字
        let reserved_keywords = [
            "SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE", "TABLE", "INDEX",
            "WHERE", "FROM", "JOIN", "UNION", "OR", "AND", "NOT", "NULL", "TRUE", "FALSE", "IS",
            "IN", "LIKE", "BETWEEN", "ORDER", "BY", "GROUP", "HAVING", "LIMIT", "OFFSET",
            "DISTINCT", "COUNT", "SUM", "AVG", "MAX", "MIN", "VIEW", "TRIGGER", "PRAGMA",
        ];

        let upper_identifier = identifier.to_uppercase();
        if reserved_keywords.contains(&upper_identifier.as_str()) {
            return Err(CacheError::DatabaseError(format!(
                "Invalid identifier '{}': reserved keyword",
                identifier
            )));
        }

        Ok(())
    }

    /// 转义SQL标识符（使用双引号）
    fn escape_identifier(&self, identifier: &str) -> String {
        format!("\"{}\"", identifier)
    }

    pub async fn new(connection_string: &str, config: PartitionConfig) -> Result<Self> {
        let connection_string = ensure_database_directory(connection_string)?;
        let normalized = normalize_connection_string(&connection_string);
        let _parsed = super::ParsedConnectionString::parse(&normalized);

        let mut opt = ConnectOptions::new(normalized.clone());
        opt.max_connections(1)
            .min_connections(1)
            .connect_timeout(std::time::Duration::from_secs(30));

        let connection = Database::connect(opt)
            .await
            .map_err(|e| CacheError::DatabaseError(format!("Failed to open database: {}", e)))?;

        Ok(Self {
            config,
            connection: Arc::new(connection),
        })
    }

    pub fn new_sync(connection_string: &str, config: PartitionConfig) -> Result<Self> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(Self::new(connection_string, config))
    }

    async fn execute(&self, sql: &str) -> Result<()> {
        (*self.connection)
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                sql.to_string(),
            ))
            .await
            .map_err(|e| CacheError::DatabaseError(format!("SQL execution failed: {}", e)))?;
        Ok(())
    }

    async fn query_one<T, F>(&self, sql: &str, mapper: F) -> Result<Option<T>>
    where
        F: Fn(sea_orm::QueryResult) -> Result<T>,
    {
        let result = (*self.connection)
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                sql.to_string(),
            ))
            .await
            .map_err(|e| CacheError::DatabaseError(format!("SQL query failed: {}", e)))?;

        match result {
            Some(row) => mapper(row).map(Some),
            None => Ok(None),
        }
    }

    async fn query_all<T, F>(&self, sql: &str, mapper: F) -> Result<Vec<T>>
    where
        F: Fn(sea_orm::QueryResult) -> Result<T>,
    {
        let results = (*self.connection)
            .query_all(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                sql.to_string(),
            ))
            .await
            .map_err(|e| CacheError::DatabaseError(format!("SQL query failed: {}", e)))?;

        let mut items = Vec::new();
        for row in results {
            items.push(mapper(row)?);
        }
        Ok(items)
    }
}

#[async_trait]
impl PartitionManagerExt for SQLitePartitionManager {
    fn ensure_partition_exists(
        &self,
        date: DateTime<Utc>,
        table_name: &str,
    ) -> impl std::future::Future<Output = Result<String>> + Send {
        Box::pin(
            async move { PartitionManager::ensure_partition_exists(self, date, table_name).await },
        )
    }

    fn get_config(&self) -> &PartitionConfig {
        &self.config
    }
}

#[async_trait]
impl PartitionManager for SQLitePartitionManager {
    async fn initialize_table(&self, table_name: &str, schema: &str) -> Result<()> {
        // 验证表名
        self.validate_identifier(table_name)?;

        let escaped_main_table = self.escape_identifier(&format!("{}_main", table_name));

        // 替换schema中的表名为主表名
        let main_table_sql = schema.replace(
            &format!("CREATE TABLE IF NOT EXISTS {}", table_name),
            &format!("CREATE TABLE IF NOT EXISTS {}", escaped_main_table),
        );

        debug!("Creating main table with SQL: {}", main_table_sql);
        self.execute(&main_table_sql).await?;

        let now = Utc::now();
        let partition_table_name = self.generate_partition_table_name(table_name, &now);
        self.validate_identifier(&partition_table_name)?;
        let escaped_partition_table = self.escape_identifier(&partition_table_name);

        // 替换schema中的表名为分区表名
        let partition_schema = schema.replace(
            &format!("CREATE TABLE IF NOT EXISTS {}", table_name),
            &format!("CREATE TABLE IF NOT EXISTS {}", escaped_partition_table),
        );

        debug!(
            "Creating partition table {} with SQL: {}",
            partition_table_name, partition_schema
        );
        self.execute(&partition_schema).await?;

        // 使用参数化查询检查视图是否存在
        let view_check = "SELECT name FROM sqlite_master WHERE type='view' AND name = ?";
        let view_exists = self
            .query_one::<String, _>(view_check, |row| {
                row.try_get::<String>("", "name")
                    .map_err(|e| CacheError::DatabaseError(e.to_string()))
            })
            .await?
            .is_some();

        if !view_exists {
            let escaped_table = self.escape_identifier(table_name);
            let view_sql = format!(
                "CREATE VIEW IF NOT EXISTS {} AS SELECT * FROM {} UNION ALL SELECT * FROM {}",
                escaped_table, escaped_main_table, escaped_partition_table
            );

            self.execute(&view_sql).await?;
        }

        Ok(())
    }

    async fn create_partition(&self, partition: &PartitionInfo) -> Result<()> {
        let base_table = self.extract_base_table(&partition.table_name);

        // 验证所有标识符
        self.validate_identifier(&base_table)?;
        self.validate_identifier(&partition.table_name)?;

        let escaped_base_table = self.escape_identifier(&base_table);
        let escaped_main_table = self.escape_identifier(&format!("{}_main", base_table));
        let escaped_partition_table = self.escape_identifier(&partition.table_name);

        // 先删除已存在的view（如果存在）
        let drop_view_sql = format!("DROP VIEW IF EXISTS {}", escaped_base_table);
        self.execute(&drop_view_sql).await?;

        // 再删除已存在的table（如果存在）
        let drop_table_sql = format!("DROP TABLE IF EXISTS {}", escaped_base_table);
        self.execute(&drop_table_sql).await?;

        // 使用参数化查询检查主表是否存在
        let main_table_check = "SELECT name FROM sqlite_master WHERE type='table' AND name = ?";
        let result = self
            .query_one::<String, _>(main_table_check, |row| {
                row.try_get::<String>("", "name")
                    .map_err(|e| CacheError::DatabaseError(e.to_string()))
            })
            .await?;

        if result.is_none() {
            let create_main_sql = format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    key TEXT NOT NULL,
                    value TEXT,
                    timestamp TEXT DEFAULT CURRENT_TIMESTAMP
                )",
                escaped_main_table
            );
            self.execute(&create_main_sql).await?;
        }

        let create_sql = format!(
            "CREATE TABLE IF NOT EXISTS {} AS SELECT * FROM {} WHERE 0",
            escaped_partition_table, escaped_main_table
        );

        self.execute(&create_sql).await?;

        // 使用参数化查询获取分区表列表
        let partition_tables_query = "SELECT name FROM sqlite_master
             WHERE type='table'
             AND name LIKE ? || '_%'
             AND name != ?
             ORDER BY name";

        let partition_tables: Vec<String> = self
            .query_all::<String, _>(partition_tables_query, |row| {
                row.try_get::<String>("", "name")
                    .map_err(|e| CacheError::DatabaseError(e.to_string()))
            })
            .await?;

        // 验证所有分区表名
        for table_name in &partition_tables {
            self.validate_identifier(table_name)?;
        }

        if !partition_tables.is_empty() {
            let union_parts: Vec<String> = partition_tables
                .iter()
                .map(|t| {
                    let escaped = self.escape_identifier(t);
                    format!("SELECT * FROM {}", escaped)
                })
                .collect();
            let union_sql = union_parts.join(" UNION ALL ");

            // 先删除已存在的表或视图
            let drop_table_sql = format!("DROP TABLE IF EXISTS {}", escaped_base_table);
            self.execute(&drop_table_sql).await?;

            let drop_view_sql = format!("DROP VIEW IF EXISTS {}", escaped_base_table);
            self.execute(&drop_view_sql).await?;

            let create_view_sql = format!(
                "CREATE VIEW IF NOT EXISTS {} AS SELECT * FROM {} UNION ALL {}",
                escaped_base_table, escaped_main_table, union_sql
            );

            self.execute(&create_view_sql).await?;
        } else {
            // 先删除已存在的表或视图
            let drop_table_sql = format!("DROP TABLE IF EXISTS {}", escaped_base_table);
            self.execute(&drop_table_sql).await?;

            let drop_view_sql = format!("DROP VIEW IF EXISTS {}", escaped_base_table);
            self.execute(&drop_view_sql).await?;

            let create_view_sql = format!(
                "CREATE VIEW IF NOT EXISTS {} AS SELECT * FROM {}",
                escaped_base_table, escaped_main_table
            );

            self.execute(&create_view_sql).await?;
        }

        Ok(())
    }

    async fn get_partitions(&self, table_name: &str) -> Result<Vec<PartitionInfo>> {
        // 验证表名
        self.validate_identifier(table_name)?;

        // 不使用转义，直接使用表名（已经验证过安全性）
        let query_sql = format!(
            "SELECT name FROM sqlite_master
             WHERE type='table'
             AND (name LIKE '{}_%' OR name = '{}_main')
             ORDER BY name",
            table_name, table_name
        );

        // 调试：打印查询SQL
        debug!("get_partitions query: {}", query_sql);

        let results = self
            .query_all::<String, _>(&query_sql, |row| {
                row.try_get::<String>("", "name")
                    .map_err(|e| CacheError::DatabaseError(e.to_string()))
            })
            .await?;

        // 调试：打印查询结果
        debug!("get_partitions found {} tables", results.len());
        for table_name in &results {
            debug!("  Found table: {}", table_name);
        }

        let mut partitions = Vec::new();
        for table_name in results {
            // 验证分区表名
            if let Some(start_date) = self.parse_partition_date(&table_name) {
                let end_date = self.get_next_month_first_day(&start_date);

                partitions.push(PartitionInfo {
                    name: table_name.clone(),
                    start_date,
                    end_date,
                    table_name,
                    created: true,
                });
            } else if table_name.ends_with("_main") {
                // 主表也包含在分区列表中
                let base_name = table_name.trim_end_matches("_main");
                partitions.push(PartitionInfo {
                    name: table_name.clone(),
                    start_date: Utc::now(),
                    end_date: Utc::now(),
                    table_name: base_name.to_string(),
                    created: true,
                });
            }
        }

        Ok(partitions)
    }

    async fn drop_partition(&self, _table_name: &str, partition_name: &str) -> Result<()> {
        // 验证分区名
        self.validate_identifier(partition_name)?;

        let escaped_partition = self.escape_identifier(partition_name);
        let drop_sql = format!("DROP TABLE IF EXISTS {}", escaped_partition);
        self.execute(&drop_sql).await?;
        Ok(())
    }

    async fn ensure_partition_exists(
        &self,
        date: DateTime<Utc>,
        table_name: &str,
    ) -> Result<String> {
        let partition_table = self.generate_partition_table_name(table_name, &date);

        let partitions = self.get_partitions(table_name).await?;
        let exists = partitions.iter().any(|p| p.table_name == partition_table);

        if !exists {
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
                date.with_year(date.year() + 1)
                    .expect("Year change should succeed")
                    .with_month(1)
                    .expect("January should exist")
                    .with_day(1)
                    .expect("Day 1 should exist")
                    .with_hour(0)
                    .expect("Hour 0 should exist")
                    .with_minute(0)
                    .expect("Minute 0 should exist")
                    .with_second(0)
                    .expect("Second 0 should exist")
            } else {
                date.with_month(date.month() + 1)
                    .expect("Month change should succeed")
                    .with_day(1)
                    .expect("Day 1 should exist")
                    .with_hour(0)
                    .expect("Hour 0 should exist")
                    .with_minute(0)
                    .expect("Minute 0 should exist")
                    .with_second(0)
                    .expect("Second 0 should exist")
            };

            let partition_info = PartitionInfo {
                name: partition_table.clone(),
                start_date,
                end_date,
                table_name: partition_table.clone(),
                created: false,
            };

            self.create_partition(&partition_info).await?;
        }

        Ok(partition_table)
    }

    async fn precreate_partitions(&self, table_name: &str, months_ahead: u32) -> Result<()> {
        PartitionManagerExt::precreate_partitions(self, table_name, months_ahead).await
    }

    async fn cleanup_old_partitions(
        &self,
        table_name: &str,
        retention_months: u32,
    ) -> Result<usize> {
        PartitionManagerExt::cleanup_old_partitions(self, table_name, retention_months).await
    }
}
