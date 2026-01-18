//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了WAL（Write-Ahead Log）日志管理机制。
//! 通过 `wal-recovery` feature 控制启用/禁用

#[cfg(feature = "wal-recovery")]
use crate::database::{is_test_connection_string, normalize_connection_string};
#[cfg(feature = "wal-recovery")]
use crate::error::Result;
#[cfg(feature = "wal-recovery")]
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement, TransactionTrait,
    Value,
};
#[cfg(feature = "wal-recovery")]
use std::env;
#[cfg(feature = "wal-recovery")]
use std::path::Path;
#[cfg(feature = "wal-recovery")]
use std::sync::Arc;
#[cfg(feature = "wal-recovery")]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(feature = "wal-recovery")]
use tokio::sync::{Mutex, Notify};

#[cfg(feature = "wal-recovery")]
#[allow(async_fn_in_trait)]
pub trait WalReplayableBackend: Clone + Send + Sync + 'static {
    async fn pipeline_replay(&self, entries: Vec<WalEntry>) -> Result<()>;
}

#[cfg(feature = "wal-recovery")]
impl<T: WalReplayableBackend> WalReplayableBackend for Arc<T> {
    async fn pipeline_replay(&self, entries: Vec<WalEntry>) -> Result<()> {
        T::pipeline_replay(self, entries).await
    }
}

#[cfg(feature = "wal-recovery")]
#[derive(Debug, Clone)]
pub struct WalEntry {
    pub timestamp: SystemTime,
    pub operation: Operation,
    pub key: String,
    pub value: Option<Vec<u8>>,
    pub ttl: Option<i64>,
}

#[cfg(feature = "wal-recovery")]
#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Set,
    Delete,
}

#[cfg(feature = "wal-recovery")]
pub struct WalManager {
    db: Arc<DatabaseConnection>,
    service_name: String,
    pending_entries: Arc<Mutex<Vec<WalEntry>>>,
    flush_trigger: Arc<Notify>,
    batch_size: usize,
}

#[cfg(feature = "wal-recovery")]
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

            format!("sqlite:{}?mode=rwc", wal_file)
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
                service_name TEXT NOT NULL)
        "#;

        db.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            create_sql.to_string(),
        ))
        .await
        .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?;

        let db_arc = Arc::new(db);
        let pending_entries = Arc::new(Mutex::new(Vec::new()));
        let flush_trigger = Arc::new(Notify::new());
        let batch_size = 100; // 批量写入大小

        // 启动后台批量写入任务
        let db_clone = Arc::clone(&db_arc);
        let service_name_clone = service_name.to_string();
        let pending_entries_clone = Arc::clone(&pending_entries);
        let flush_trigger_clone = Arc::clone(&flush_trigger);
        let batch_size_clone = batch_size;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // 定期刷新
                        Self::flush_batch_internal(
                            &db_clone,
                            &service_name_clone,
                            &pending_entries_clone,
                            batch_size_clone
                        ).await;
                    }
                    _ = flush_trigger_clone.notified() => {
                        // 手动触发刷新
                        Self::flush_batch_internal(
                            &db_clone,
                            &service_name_clone,
                            &pending_entries_clone,
                            batch_size_clone
                        ).await;
                    }
                }
            }
        });

        Ok(Self {
            db: db_arc,
            service_name: service_name.to_string(),
            pending_entries,
            flush_trigger,
            batch_size,
        })
    }

    pub async fn add_entry(&self, entry: &WalEntry) -> Result<()> {
        // 添加到缓冲区
        {
            let mut pending = self.pending_entries.lock().await;
            pending.push(entry.clone());

            // 如果达到批量大小，触发刷新
            if pending.len() >= self.batch_size {
                drop(pending); // 释放锁
                self.flush_trigger.notify_one();
            }
        }
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
        // 使用参数化查询防止SQL注入
        use sea_orm::Value;

        self.db
            .execute(Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                "DELETE FROM wal_entries WHERE service_name = ?",
                [Value::String(Some(Box::new(self.service_name.clone())))],
            ))
            .await
            .map_err(|e| crate::error::CacheError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// 刷新缓冲区中的所有条目到数据库（使用事务批量提交）
    pub async fn flush(&self) -> Result<()> {
        Self::flush_batch_internal(
            &self.db,
            &self.service_name,
            &self.pending_entries,
            self.batch_size,
        )
        .await;
        Ok(())
    }

    /// 内部批量刷新方法
    async fn flush_batch_internal(
        db: &Arc<DatabaseConnection>,
        service_name: &str,
        pending_entries: &Arc<Mutex<Vec<WalEntry>>>,
        batch_size: usize,
    ) {
        let entries_to_flush = {
            let mut pending = pending_entries.lock().await;
            if pending.is_empty() {
                return;
            }
            let count = pending.len().min(batch_size);
            let entries: Vec<WalEntry> = pending.drain(..count).collect();
            entries
        };

        if entries_to_flush.is_empty() {
            return;
        }

        // 使用事务批量插入
        let txn = match db.begin().await {
            Ok(txn) => txn,
            Err(e) => {
                tracing::error!("Failed to begin transaction for WAL batch write: {}", e);
                return;
            }
        };

        let insert_sql = r#"
            INSERT INTO wal_entries (timestamp, operation, key, value, ttl, service_name)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#;

        let mut success = true;
        for entry in &entries_to_flush {
            let timestamp = match entry.timestamp.duration_since(UNIX_EPOCH) {
                Ok(d) => d.as_secs() as i64,
                Err(e) => {
                    tracing::error!("Failed to convert timestamp: {}", e);
                    success = false;
                    break;
                }
            };

            let operation = match entry.operation {
                Operation::Set => "SET",
                Operation::Delete => "DELETE",
            };

            let result = txn
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
                        Value::String(Some(Box::new(service_name.to_string()))),
                    ],
                ))
                .await;

            if let Err(e) = result {
                tracing::error!("Failed to insert WAL entry: {}", e);
                success = false;
                break;
            }
        }

        if success {
            if let Err(e) = txn.commit().await {
                tracing::error!("Failed to commit WAL batch transaction: {}", e);
                // 回滚：将条目放回缓冲区
                let mut pending = pending_entries.lock().await;
                for entry in entries_to_flush {
                    pending.push(entry);
                }
            }
        } else {
            if let Err(e) = txn.rollback().await {
                tracing::error!("Failed to rollback WAL batch transaction: {}", e);
            }
            // 回滚：将条目放回缓冲区
            let mut pending = pending_entries.lock().await;
            for entry in entries_to_flush {
                pending.push(entry);
            }
        }
    }

    pub async fn clear(&self) -> Result<()> {
        self.clear_entries().await
    }

    /// 重放所有 WAL 条目到后端
    ///
    /// # 参数
    ///
    /// * `backend` - 可重放的后端实现
    ///
    /// # 返回值
    ///
    /// 返回成功重放的条目数量
    ///
    /// # 注意
    ///
    /// 实现事务性重放：只在确认所有条目都成功后才清空 WAL
    /// 如果重放失败，WAL 条目将保留以便下次重试
    pub async fn replay_all<B: WalReplayableBackend>(&self, backend: &B) -> Result<usize> {
        let entries = self.get_entries().await?;
        let count = entries.len();

        if entries.is_empty() {
            return Ok(0);
        }

        // 记录开始重放
        tracing::info!(
            "Starting WAL replay for service '{}': {} entries",
            self.service_name,
            count
        );

        // 尝试重放所有条目
        match backend.pipeline_replay(entries.clone()).await {
            Ok(_) => {
                // 只有在所有条目都成功重放后才清空 WAL
                tracing::info!(
                    "WAL replay successful for service '{}': clearing {} entries",
                    self.service_name,
                    count
                );
                self.clear_entries().await?;
                Ok(count)
            }
            Err(e) => {
                // 重放失败，保留 WAL 条目以便下次重试
                tracing::error!(
                    "WAL replay failed for service '{}': {}. WAL entries preserved for retry.",
                    self.service_name,
                    e
                );
                Err(e)
            }
        }
    }
}

// ============================================================================
// 当 wal-recovery 功能禁用时的空实现
// ============================================================================

#[cfg(not(feature = "wal-recovery"))]
use crate::error::Result;

/// WAL条目（空实现）
#[cfg(not(feature = "wal-recovery"))]
#[derive(Debug, Clone, Default)]
pub struct WalEntry {
    pub timestamp: std::time::SystemTime,
    pub operation: Operation,
    pub key: String,
    pub value: Option<Vec<u8>>,
    pub ttl: Option<i64>,
}

/// WAL操作类型（空实现）
#[cfg(not(feature = "wal-recovery"))]
#[derive(Debug, Clone, Copy, Default)]
pub enum Operation {
    #[default]
    Set,
    Delete,
}

/// 可重放后端Trait（空实现）
#[cfg(not(feature = "wal-recovery"))]
#[allow(async_fn_in_trait)]
pub trait WalReplayableBackend: Clone + Send + Sync + 'static {
    async fn pipeline_replay(&self, _entries: Vec<WalEntry>) -> Result<()>;
}

#[cfg(not(feature = "wal-recovery"))]
impl<T: WalReplayableBackend> WalReplayableBackend for Arc<T> {
    async fn pipeline_replay(&self, _entries: Vec<WalEntry>) -> Result<()> {
        Ok(())
    }
}

/// WAL管理器（空实现）
#[cfg(not(feature = "wal-recovery"))]
#[derive(Debug, Clone, Default)]
pub struct WalManager;

#[cfg(not(feature = "wal-recovery"))]
impl WalManager {
    pub async fn new(_service_name: &str) -> Result<Self> {
        Ok(Self)
    }

    pub async fn add_entry(&self, _entry: &WalEntry) -> Result<()> {
        Ok(())
    }

    pub async fn append(&self, _entry: WalEntry) -> Result<()> {
        Ok(())
    }

    pub async fn get_entries(&self) -> Result<Vec<WalEntry>> {
        Ok(Vec::new())
    }

    pub async fn clear_entries(&self) -> Result<()> {
        Ok(())
    }

    pub async fn flush(&self) -> Result<()> {
        Ok(())
    }

    pub async fn clear(&self) -> Result<()> {
        Ok(())
    }

    pub async fn replay_all<B: WalReplayableBackend>(&self, _backend: &B) -> Result<usize> {
        Ok(0)
    }
}
