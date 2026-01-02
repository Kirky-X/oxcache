//! MySQL分区管理器实现

//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了MySQL分区管理器的实现。

use crate::error::{CacheError, Result};
use chrono::{DateTime, Datelike, TimeZone, Utc};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::timeout;
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
    pub failed_attempts: u32,
}

/// MySQL分区管理器
pub struct MySQLPartitionManager {
    config: PartitionConfig,
    connection: Arc<DatabaseConnection>,
    pool_stats: Arc<Mutex<PoolStats>>,
}

impl MySQLPartitionManager {
    /// 创建新的MySQL分区管理器
    pub async fn new(connection_string: &str, config: PartitionConfig) -> Result<Self> {
        let mut opt = ConnectOptions::new(connection_string.to_string());
        opt.max_connections(10)
            .min_connections(2)
            .connect_timeout(Duration::from_secs(5))
            .idle_timeout(Duration::from_secs(8))
            .max_lifetime(Duration::from_secs(1800))
            .acquire_timeout(Duration::from_secs(10));

        let start = Instant::now();
        let connection = match timeout(Duration::from_secs(30), Database::connect(opt)).await {
            Ok(Ok(conn)) => conn,
            Ok(Err(e)) => {
                return Err(CacheError::DatabaseError(format!(
                    "Failed to connect to MySQL: {}. Please check your connection string and ensure the database server is running.",
                    e
                )));
            }
            Err(_) => {
                return Err(CacheError::DatabaseError(
                    "Connection timeout: MySQL server not responding within 30 seconds. Please check your connection string and ensure the database server is running.".to_string()
                ));
            }
        };

        let acquire_duration = start.elapsed();
        info!("MySQL connection established in {:?}", acquire_duration);

        if acquire_duration > Duration::from_secs(3) {
            warn!(
                "MySQL connection took longer than expected: {:?}",
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
                failed_attempts: 0,
            })),
        })
    }

    /// 验证连接健康状态
    pub async fn health_check(&self) -> bool {
        if let Err(e) = self.ping().await {
            warn!("MySQL health check failed: {}", e);
            return false;
        }
        true
    }

    /// 测试连接是否活跃
    pub async fn ping(&self) -> Result<()> {
        let conn = self.connection.as_ref();
        conn.execute(Statement::from_string(
            sea_orm::DatabaseBackend::MySql,
            "SELECT 1".to_string(),
        ))
        .await
        .map_err(|e| {
            CacheError::DatabaseError(format!(
                "Connection health check failed: {}. The connection may have been lost.",
                e
            ))
        })?;
        Ok(())
    }

    /// 获取连接池统计信息
    pub async fn pool_stats(&self) -> PoolStats {
        self.pool_stats.lock().await.clone()
    }

    /// 重新建立连接（用于恢复）
    pub async fn reconnect(&mut self, connection_string: &str) -> Result<()> {
        info!("Attempting to reconnect to MySQL...");

        let mut opt = ConnectOptions::new(connection_string.to_string());
        opt.max_connections(10)
            .min_connections(2)
            .connect_timeout(Duration::from_secs(10))
            .idle_timeout(Duration::from_secs(8));

        let start = Instant::now();
        let connection = match timeout(Duration::from_secs(30), Database::connect(opt)).await {
            Ok(Ok(conn)) => conn,
            Ok(Err(e)) => {
                return Err(CacheError::DatabaseError(format!(
                    "Failed to reconnect to MySQL: {}. Please check your database server.",
                    e
                )));
            }
            Err(_) => {
                return Err(CacheError::DatabaseError(
                    "Reconnection timeout: MySQL server not responding within 30 seconds."
                        .to_string(),
                ));
            }
        };

        let acquire_duration = start.elapsed();
        info!("MySQL reconnection established in {:?}", acquire_duration);

        self.connection = Arc::new(connection);

        let mut stats = self.pool_stats.lock().await;
        stats.connection_acquire_ms = acquire_duration.as_secs_f64() * 1000.0;
        stats.last_acquire = Some(Instant::now());
        stats.failed_attempts = 0;

        Ok(())
    }
}

#[async_trait::async_trait]
impl PartitionManager for MySQLPartitionManager {
    async fn initialize_table(&self, table_name: &str, schema: &str) -> Result<()> {
        if self.config.enabled {
            // 创建分区表
            let partition_schema = self.add_partition_clause_to_schema(schema, table_name)?;
            self.connection
                .execute(Statement::from_string(
                    sea_orm::DatabaseBackend::MySql,
                    partition_schema,
                ))
                .await?;
        } else {
            // 创建普通表
            self.connection
                .execute(Statement::from_string(
                    sea_orm::DatabaseBackend::MySql,
                    schema.to_string(),
                ))
                .await?;
        }

        Ok(())
    }

    async fn create_partition(&self, partition: &PartitionInfo) -> Result<()> {
        // 提取基础表名（使用公共方法）
        let base_table = self.extract_base_table(&partition.table_name);

        // MySQL分区命名约定（使用公共方法）
        let partition_name = self.generate_partition_name(&partition.start_date, "p");

        let _start_days = self.date_to_days(&partition.start_date);
        let end_days = self.date_to_days(&partition.end_date);

        // 验证表名和分区名，防止 SQL 注入
        self.validate_identifier(&base_table)?;
        self.validate_identifier(&partition_name)?;

        // 检查分区是否已存在 - 使用参数化查询
        let check_sql = "SELECT COUNT(*) FROM INFORMATION_SCHEMA.PARTITIONS
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND PARTITION_NAME = ?";

        let statement =
            Statement::from_string(sea_orm::DatabaseBackend::MySql, check_sql.to_string());

        let result = self.connection.query_one(statement).await?;
        if let Some(row) = result {
            let count: i64 = row.try_get("", "COUNT(*)")?;
            if count > 0 {
                // 分区已存在
                return Ok(());
            }
        }

        // 获取现有分区
        let existing_partitions = self.get_partitions(&base_table).await?;
        debug!(
            "Creating partition {} for table {}, existing partitions: {}",
            partition_name,
            base_table,
            existing_partitions.len()
        );

        // 按 end_date 排序现有分区
        let mut sorted_partitions = existing_partitions.clone();
        sorted_partitions.sort_by_key(|p| p.end_date);

        // 找到第一个 end_date > new_partition.end_date 的分区
        let target_partition = sorted_partitions
            .iter()
            .find(|p| p.end_date > partition.end_date);

        // 验证所有标识符，防止 SQL 注入
        self.validate_identifier(&base_table)?;
        self.validate_identifier(&partition_name)?;
        if let Some(target) = &target_partition {
            self.validate_identifier(&target.name)?;
        }

        let sql = if let Some(target) = target_partition {
            // 需要重组 target 分区
            debug!(
                "Reorganizing partition {} to insert {}",
                target.name, partition_name
            );

            let target_end_days_str = if target.name == "p_future" {
                "MAXVALUE".to_string()
            } else {
                self.date_to_days(&target.end_date).to_string()
            };

            format!(
                "ALTER TABLE {} REORGANIZE PARTITION {} INTO (PARTITION {} VALUES LESS THAN ({}), PARTITION {} VALUES LESS THAN ({}))",
                self.escape_identifier(&base_table),
                self.escape_identifier(&target.name),
                self.escape_identifier(&partition_name), end_days,
                self.escape_identifier(&target.name), target_end_days_str
            )
        } else {
            // 没有更大的分区，直接添加
            debug!(
                "DEBUG: Appending new partition {} at the end",
                partition_name
            );
            format!(
                "ALTER TABLE {} ADD PARTITION (PARTITION {} VALUES LESS THAN ({}))",
                self.escape_identifier(&base_table),
                self.escape_identifier(&partition_name),
                end_days
            )
        };

        debug!("Generated SQL: {}", sql);

        self.connection
            .execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
            .await?;

        Ok(())
    }

    async fn get_partitions(&self, table_name: &str) -> Result<Vec<PartitionInfo>> {
        // 验证表名，防止 SQL 注入
        self.validate_identifier(table_name)?;

        let sql = "SELECT
                PARTITION_NAME,
                PARTITION_DESCRIPTION,
                PARTITION_ORDINAL_POSITION,
                PARTITION_METHOD,
                PARTITION_EXPRESSION
             FROM INFORMATION_SCHEMA.PARTITIONS
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND PARTITION_NAME IS NOT NULL";

        debug!("get_partitions SQL: {} with table_name={}", sql, table_name);

        let statement = Statement::from_string(sea_orm::DatabaseBackend::MySql, sql.to_string());

        let result = self.connection.query_all(statement).await?;
        debug!("get_partitions found {} rows", result.len());

        let mut partitions = Vec::new();
        for row in result {
            let partition_name: String = row.try_get("", "PARTITION_NAME")?;
            let partition_description: Option<String> = row.try_get("", "PARTITION_DESCRIPTION")?;

            debug!(
                "Found partition: name={}, description={:?}",
                partition_name, partition_description
            );

            // 解析分区信息
            if let Some(info) = self.parse_mysql_partition(
                table_name,
                &partition_name,
                partition_description.as_deref(),
            ) {
                partitions.push(info);
            }
        }

        debug!("get_partitions returning {} partitions", partitions.len());

        Ok(partitions)
    }

    async fn drop_partition(&self, table_name: &str, partition_name: &str) -> Result<()> {
        // 验证表名和分区名，防止 SQL 注入
        self.validate_identifier(table_name)?;
        self.validate_identifier(partition_name)?;

        let sql = format!(
            "ALTER TABLE {} DROP PARTITION {}",
            self.escape_identifier(table_name),
            self.escape_identifier(partition_name)
        );

        self.connection
            .execute(Statement::from_string(sea_orm::DatabaseBackend::MySql, sql))
            .await?;

        Ok(())
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

    // 使用通用实现
    async fn precreate_partitions(&self, table_name: &str, months_ahead: u32) -> Result<()> {
        PartitionManagerExt::precreate_partitions(self, table_name, months_ahead).await
    }
}

#[async_trait::async_trait]
impl PartitionManagerExt for MySQLPartitionManager {
    // 使用PartitionManager trait中定义的ensure_partition_exists实现
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

impl MySQLPartitionManager {
    /// 验证 SQL 标识符是否安全（防止 SQL 注入）
    fn validate_identifier(&self, identifier: &str) -> Result<()> {
        // 标识符只能包含字母、数字、下划线，且必须以字母或下划线开头
        if identifier.is_empty() {
            return Err(CacheError::DatabaseError(
                "Identifier cannot be empty".to_string(),
            ));
        }

        // 检查长度限制
        if identifier.len() > 64 {
            return Err(CacheError::DatabaseError(
                "Identifier exceeds maximum length of 64 characters".to_string(),
            ));
        }

        // 检查字符集
        if !identifier.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(CacheError::DatabaseError(format!(
                "Invalid identifier '{}': only alphanumeric characters and underscores are allowed",
                identifier
            )));
        }

        // 检查第一个字符
        let first_char = identifier
            .chars()
            .next()
            .ok_or_else(|| CacheError::DatabaseError("Invalid identifier: empty".to_string()))?;
        if !first_char.is_alphabetic() && first_char != '_' {
            return Err(CacheError::DatabaseError(format!(
                "Invalid identifier '{}': must start with a letter or underscore",
                identifier
            )));
        }

        // 检查是否是保留关键字
        let reserved_keywords = [
            "SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE", "TABLE", "INDEX",
            "WHERE", "FROM", "JOIN", "UNION", "OR", "AND", "NOT", "NULL", "TRUE", "FALSE", "IS",
            "IN", "LIKE", "BETWEEN", "ORDER", "BY", "GROUP", "HAVING", "LIMIT", "OFFSET",
            "DISTINCT", "COUNT", "SUM", "AVG", "MAX", "MIN",
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

    /// 转义 SQL 标识符（使用反引号）
    fn escape_identifier(&self, identifier: &str) -> String {
        format!("`{}`", identifier)
    }

    /// 将分区子句添加到表结构
    fn add_partition_clause_to_schema(
        &self,
        original_schema: &str,
        _table_name: &str,
    ) -> Result<String> {
        // 获取当前日期和下一个日期（使用UTC时间）
        let now = Utc::now();
        let current_year = now.year();
        let current_month = now.month();

        // 计算下个月的第一天
        let (next_year, next_month) = if current_month == 12 {
            (current_year + 1, 1)
        } else {
            (current_year, current_month + 1)
        };

        let start_of_next_month = Utc
            .with_ymd_and_hms(next_year, next_month, 1, 0, 0, 0)
            .single()
            .ok_or_else(|| {
                CacheError::DatabaseError(format!(
                    "Invalid date: {}-{}-01 is not a valid date",
                    next_year, next_month
                ))
            })?;
        let next_month_days = self.date_to_days(&start_of_next_month);

        // 查找表结构中的分区列 - 优先使用DATE类型，避免使用TIMESTAMP（时区相关）
        let partition_column = if original_schema.contains("created_at DATE") {
            "created_at"
        } else if original_schema.contains("date_column") {
            "date_column"
        } else if original_schema.contains("created_at")
            && !original_schema.contains("created_at TIMESTAMP")
        {
            "created_at"
        } else {
            // 如果没有找到合适的时间列，默认使用created_at
            "created_at"
        };

        // 添加分区子句 - 使用 TO_DAYS 函数，但确保列是 DATE 类型
        let partition_clause = format!(
            " PARTITION BY RANGE (TO_DAYS({})) (PARTITION p{}_{} VALUES LESS THAN ({}), PARTITION p_future VALUES LESS THAN MAXVALUE)",
            partition_column,
            current_year, current_month,
            next_month_days
        );

        // 将分区子句添加到表结构的末尾
        let modified_schema = if original_schema.trim().ends_with(';') {
            let trimmed = original_schema.trim().trim_end_matches(';');
            format!("{}{};", trimmed, partition_clause)
        } else {
            format!("{}{}", original_schema, partition_clause)
        };

        debug!("Modified schema: {}", modified_schema);
        Ok(modified_schema)
    }

    /// 将日期转换为MySQL的TO_DAYS函数值
    fn date_to_days(&self, date: &DateTime<Utc>) -> i32 {
        // MySQL的TO_DAYS函数计算从0年到指定日期的天数
        let epoch = Utc
            .with_ymd_and_hms(0, 1, 1, 0, 0, 0)
            .single()
            .expect("Year 0-01-01 00:00:00 should be a valid UTC date");
        let duration = date.signed_duration_since(epoch);
        duration.num_days() as i32
    }

    fn parse_mysql_partition(
        &self,
        table_name: &str,
        partition_name: &str,
        _description: Option<&str>,
    ) -> Option<PartitionInfo> {
        debug!("parse_mysql_partition called with name: {}", partition_name);

        // 处理特殊的p_future分区（MAXVALUE分区）
        if partition_name == "p_future" {
            debug!("Found p_future partition (MAXVALUE)");
            // 使用一个遥远的未来日期作为结束日期
            let max_date = Utc
                .with_ymd_and_hms(9999, 12, 31, 23, 59, 59)
                .single()
                .expect("Year 9999-12-31 23:59:59 should be a valid UTC date");
            let mut info = PartitionInfo::new(max_date, table_name);
            info.name = partition_name.to_string();
            info.start_date = max_date;
            info.end_date = max_date;
            info.created = true;
            return Some(info);
        }

        // MySQL分区名格式: p2024_1, p2024_2等
        if let Some(stripped) = partition_name.strip_prefix('p') {
            let parts: Vec<&str> = stripped.split('_').collect();
            debug!("Parsed parts: {:?}", parts);
            if parts.len() == 2 {
                if let (Ok(year), Ok(month)) = (parts[0].parse::<i32>(), parts[1].parse::<u32>()) {
                    debug!("Parsed year={}, month={}", year, month);
                    if let Some(date) = Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0).single() {
                        let mut info = PartitionInfo::new(date, table_name);
                        info.name = partition_name.to_string();
                        info.created = true;
                        debug!("Successfully parsed partition: {:?}", info.name);
                        return Some(info);
                    }
                }
            }
        }

        debug!("Failed to parse partition: {}", partition_name);
        None
    }
}
