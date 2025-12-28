use crate::database::{is_test_connection_string, normalize_connection_string};
use crate::error::Result;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement, Value};
use std::env;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[allow(async_fn_in_trait)]
pub trait WalReplayableBackend: Clone + Send + Sync + 'static {
    async fn pipeline_replay(&self, entries: Vec<WalEntry>) -> Result<()>;
}

impl<T: WalReplayableBackend> WalReplayableBackend for Arc<T> {
    async fn pipeline_replay(&self, entries: Vec<WalEntry>) -> Result<()> {
        T::pipeline_replay(self, entries).await
    }
}

#[derive(Debug)]
pub struct WalEntry {
    pub timestamp: SystemTime,
    pub operation: Operation,
    pub key: String,
    pub value: Option<Vec<u8>>,
    pub ttl: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Set,
    Delete,
}

pub struct WalManager {
    db: Arc<DatabaseConnection>,
    service_name: String,
}

impl WalManager {
    pub async fn new(service_name: &str) -> Result<Self> {
        let is_test =
            is_test_connection_string(service_name) || env::var("OXCACHE_TEST_USE_MEMORY").is_ok();

        let raw_connection_string = if is_test {
            "sqlite::memory:?cache=shared".to_string()
        } else {
            let wal_file = format!("{}_wal.db", service_name);
            let wal_path = if wal_file.starts_with("/") {
                Path::new(&wal_file).to_path_buf()
            } else {
                std::env::current_dir()?.join(&wal_file)
            };

            if let Some(parent) = wal_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        crate::error::CacheError::DatabaseError(format!(
                            "无法创建WAL目录 {}: {}",
                            parent.display(),
                            e
                        ))
                    })?;
                }
            }

            format!("sqlite:{}", wal_file)
        };

        let normalized = normalize_connection_string(&raw_connection_string);

        let mut opt = ConnectOptions::new(normalized.clone());
        opt.max_connections(1)
            .min_connections(1)
            .connect_timeout(std::time::Duration::from_secs(30));

        let db = Database::connect(opt)
            .await
            .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?;

        let create_sql = r#"
            CREATE TABLE IF NOT EXISTS wal_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                operation TEXT NOT NULL,
                key TEXT NOT NULL,
                value BLOB,
                ttl INTEGER,
                service_name TEXT NOT NULL
            )
        "#;

        db.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            create_sql.to_string(),
        ))
        .await
        .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?;

        Ok(Self {
            db: Arc::new(db),
            service_name: service_name.to_string(),
        })
    }

    pub async fn add_entry(&self, entry: &WalEntry) -> Result<()> {
        let timestamp = entry
            .timestamp
            .duration_since(UNIX_EPOCH)
            .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?
            .as_secs() as i64;

        let operation = match entry.operation {
            Operation::Set => "SET",
            Operation::Delete => "DELETE",
        };

        let insert_sql = r#"
            INSERT INTO wal_entries (timestamp, operation, key, value, ttl, service_name)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#;

        self.db
            .execute(Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                insert_sql.to_string(),
                vec![
                    Value::BigInt(Some(timestamp)),
                    Value::String(Some(Box::new(operation.to_string()))),
                    Value::String(Some(Box::new(entry.key.clone()))),
                    Value::Bytes(entry.value.as_ref().map(|v| Box::new(v.clone()))),
                    match entry.ttl {
                        Some(v) => Value::BigInt(Some(v)),
                        None => Value::BigInt(None),
                    },
                    Value::String(Some(Box::new(self.service_name.clone()))),
                ],
            ))
            .await
            .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn append(&self, entry: WalEntry) -> Result<()> {
        self.add_entry(&entry).await
    }

    pub async fn get_entries(&self) -> Result<Vec<WalEntry>> {
        let query_sql = r#"
            SELECT timestamp, operation, key, value, ttl FROM wal_entries
            WHERE service_name = ?1
            ORDER BY timestamp ASC
        "#;

        let results = self
            .db
            .query_all(Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                query_sql.to_string(),
                vec![Value::String(Some(Box::new(self.service_name.clone())))],
            ))
            .await
            .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?;

        let mut entries = Vec::new();
        for row in results {
            let timestamp_secs: i64 = row
                .try_get("", "timestamp")
                .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?;
            let timestamp = UNIX_EPOCH + std::time::Duration::from_secs(timestamp_secs as u64);

            let operation_str: String = row
                .try_get("", "operation")
                .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?;
            let operation = match operation_str.as_str() {
                "SET" => Operation::Set,
                _ => Operation::Delete,
            };

            let key: String = row
                .try_get("", "key")
                .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?;

            let value: Option<Vec<u8>> = row
                .try_get("", "value")
                .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?;

            let ttl: Option<i64> = row
                .try_get("", "ttl")
                .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?;

            entries.push(WalEntry {
                timestamp,
                operation,
                key,
                value,
                ttl,
            });
        }

        Ok(entries)
    }

    pub async fn clear_entries(&self) -> Result<()> {
        let delete_sql = format!(
            "DELETE FROM wal_entries WHERE service_name = '{}'",
            self.service_name
        );

        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                delete_sql,
            ))
            .await
            .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn clear(&self) -> Result<()> {
        self.clear_entries().await
    }

    pub async fn replay_all<B: WalReplayableBackend>(&self, backend: &B) -> Result<usize> {
        let entries = self.get_entries().await?;
        let count = entries.len();
        if entries.is_empty() {
            return Ok(0);
        }
        backend.pipeline_replay(entries).await?;
        self.clear_entries().await?;
        Ok(count)
    }
}
