use super::{common::*, PartitionConfig, PartitionInfo, PartitionManager};
use crate::error::{CacheError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Timelike, Utc};
use rusqlite::{Connection, Row};
use std::sync::{Arc, Mutex, MutexGuard};

pub struct SQLitePartitionManager {
    config: PartitionConfig,
    connection: Arc<Mutex<Connection>>,
}

impl SQLitePartitionManager {
    pub async fn new(connection_string: &str, config: PartitionConfig) -> Result<Self> {
        Self::new_sync(connection_string, config)
    }

    pub fn new_sync(connection_string: &str, config: PartitionConfig) -> Result<Self> {
        let db_path = if connection_string.starts_with("sqlite:") {
            connection_string
                .strip_prefix("sqlite:")
                .unwrap_or(connection_string)
        } else {
            connection_string
        };

        if !db_path.contains(":memory:") {
            if let Some(parent) = std::path::Path::new(db_path).parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        CacheError::DatabaseError(format!("Failed to create directory: {}", e))
                    })?;
                }
            }
        }

        let connection = Connection::open(db_path)
            .map_err(|e| CacheError::DatabaseError(format!("Failed to open database: {}", e)))?;

        Ok(Self {
            config,
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    fn connection_lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|e| CacheError::DatabaseError(format!("Mutex lock failed: {}", e)))
    }

    fn execute(&self, sql: &str) -> Result<()> {
        let conn = self.connection_lock()?;
        conn.execute(sql, [])
            .map_err(|e| CacheError::DatabaseError(format!("SQL execution failed: {}", e)))?;
        Ok(())
    }

    fn query_one<T, F>(&self, sql: &str, mapper: F) -> Result<Option<T>>
    where
        F: Fn(&Row) -> Result<T>,
    {
        let conn = self.connection_lock()?;
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| CacheError::DatabaseError(format!("SQL prepare failed: {}", e)))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| CacheError::DatabaseError(format!("SQL query failed: {}", e)))?;
        match rows
            .next()
            .map_err(|e| CacheError::DatabaseError(format!("SQL next row failed: {}", e)))?
        {
            Some(row) => mapper(row).map(Some),
            None => Ok(None),
        }
    }

    fn query_all<T, F>(&self, sql: &str, mapper: F) -> Result<Vec<T>>
    where
        F: Fn(&Row) -> Result<T>,
    {
        let conn = self.connection_lock()?;
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| CacheError::DatabaseError(format!("SQL prepare failed: {}", e)))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| CacheError::DatabaseError(format!("SQL query failed: {}", e)))?;
        let mut results = Vec::new();
        while let Some(row) = rows
            .next()
            .map_err(|e| CacheError::DatabaseError(format!("SQL next row failed: {}", e)))?
        {
            results.push(mapper(row)?);
        }
        Ok(results)
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
        let main_table_sql = schema.replace(
            &format!("CREATE TABLE IF NOT EXISTS {}", table_name),
            &format!("CREATE TABLE IF NOT EXISTS {}_main", table_name),
        );

        self.execute(&main_table_sql)?;

        let now = Utc::now();
        let partition_table = self.generate_partition_table_name(table_name, &now);
        let partition_schema = schema.replace(
            &format!("CREATE TABLE IF NOT EXISTS {}", table_name),
            &format!("CREATE TABLE IF NOT EXISTS {}", partition_table),
        );

        self.execute(&partition_schema)?;

        let view_check = format!(
            "SELECT name FROM sqlite_master WHERE type='view' AND name='{}'",
            table_name
        );
        let view_exists = self
            .query_one::<String, _>(&view_check, |row| {
                row.get::<_, String>(0)
                    .map_err(|e| CacheError::DatabaseError(e.to_string()))
            })?
            .is_some();

        if !view_exists {
            let view_sql = format!(
                "CREATE VIEW IF NOT EXISTS {} AS SELECT * FROM {}_main UNION ALL SELECT * FROM {}",
                table_name, table_name, partition_table
            );

            self.execute(&view_sql)?;
        }

        Ok(())
    }

    async fn create_partition(&self, partition: &PartitionInfo) -> Result<()> {
        let base_table = self.extract_base_table(&partition.table_name);

        let main_table_check = format!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='{}_main'",
            base_table
        );

        let result = self.query_one::<String, _>(&main_table_check, |row| {
            row.get::<_, String>(0)
                .map_err(|e| CacheError::DatabaseError(e.to_string()))
        })?;

        if result.is_none() {
            let create_main_sql = format!(
                "CREATE TABLE IF NOT EXISTS {}_main (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    key TEXT NOT NULL,
                    value TEXT,
                    timestamp TEXT DEFAULT CURRENT_TIMESTAMP
                )",
                base_table
            );
            self.execute(&create_main_sql)?;
        }

        let create_sql = format!(
            "CREATE TABLE IF NOT EXISTS {} AS SELECT * FROM {}_main WHERE 0",
            partition.table_name, base_table
        );

        self.execute(&create_sql)?;

        let partition_tables_query = format!(
            "SELECT name FROM sqlite_master 
             WHERE type='table' 
             AND name LIKE '{}_%' 
             AND name != '{}_main'
             ORDER BY name",
            base_table, base_table
        );

        let partition_tables: Vec<String> =
            self.query_all::<String, _>(&partition_tables_query, |row| {
                row.get::<_, String>(0)
                    .map_err(|e| CacheError::DatabaseError(e.to_string()))
            })?;

        if !partition_tables.is_empty() {
            let union_parts: Vec<String> = partition_tables
                .iter()
                .map(|t| format!("SELECT * FROM {}", t))
                .collect();
            let union_sql = union_parts.join(" UNION ALL ");

            let drop_view_sql = format!("DROP VIEW IF EXISTS {}", base_table);
            self.execute(&drop_view_sql)?;

            let create_view_sql = format!(
                "CREATE VIEW IF NOT EXISTS {} AS SELECT * FROM {}_main UNION ALL {}",
                base_table, base_table, union_sql
            );

            self.execute(&create_view_sql)?;
        } else {
            let drop_view_sql = format!("DROP VIEW IF EXISTS {}", base_table);
            self.execute(&drop_view_sql)?;

            let create_view_sql = format!(
                "CREATE VIEW IF NOT EXISTS {} AS SELECT * FROM {}_main",
                base_table, base_table
            );

            self.execute(&create_view_sql)?;
        }

        Ok(())
    }

    async fn get_partitions(&self, table_name: &str) -> Result<Vec<PartitionInfo>> {
        let query_sql = format!(
            "SELECT name FROM sqlite_master 
             WHERE type='table' 
             AND name LIKE '{}_%' 
             AND name != '{}_main'
             ORDER BY name",
            table_name, table_name
        );

        let results = self.query_all::<String, _>(&query_sql, |row| {
            row.get::<_, String>(0)
                .map_err(|e| CacheError::DatabaseError(e.to_string()))
        })?;

        let mut partitions = Vec::new();
        for table_name in results {
            if let Some(start_date) = self.parse_partition_date(&table_name) {
                let end_date = self.get_next_month_first_day(&start_date);

                partitions.push(PartitionInfo {
                    name: table_name.clone(),
                    start_date,
                    end_date,
                    table_name,
                    created: true,
                });
            }
        }

        Ok(partitions)
    }

    async fn drop_partition(&self, _table_name: &str, partition_name: &str) -> Result<()> {
        let drop_sql = format!("DROP TABLE IF EXISTS {}", partition_name);
        self.execute(&drop_sql)?;
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
                .unwrap()
                .with_hour(0)
                .unwrap()
                .with_minute(0)
                .unwrap()
                .with_second(0)
                .unwrap();
            let end_date = if date.month() == 12 {
                date.with_year(date.year() + 1)
                    .unwrap()
                    .with_month(1)
                    .unwrap()
                    .with_day(1)
                    .unwrap()
                    .with_hour(0)
                    .unwrap()
                    .with_minute(0)
                    .unwrap()
                    .with_second(0)
                    .unwrap()
            } else {
                date.with_month(date.month() + 1)
                    .unwrap()
                    .with_day(1)
                    .unwrap()
                    .with_hour(0)
                    .unwrap()
                    .with_minute(0)
                    .unwrap()
                    .with_second(0)
                    .unwrap()
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
