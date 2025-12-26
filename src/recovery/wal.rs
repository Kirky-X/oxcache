//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的WAL（预写日志）机制。

use crate::error::Result;
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// 定义WAL重放所需的trait
#[allow(async_fn_in_trait)]
pub trait WalReplayableBackend: Clone + Send + Sync + 'static {
    async fn pipeline_replay(&self, entries: Vec<WalEntry>) -> Result<()>;
}

/// 为Arc<T>实现WalReplayableBackend（当T实现该trait时）
impl<T: WalReplayableBackend> WalReplayableBackend for Arc<T> {
    async fn pipeline_replay(&self, entries: Vec<WalEntry>) -> Result<()> {
        T::pipeline_replay(self, entries).await
    }
}

/// WAL条目
///
/// 表示一个预写日志条目
#[derive(Debug)]
pub struct WalEntry {
    /// 时间戳
    pub timestamp: SystemTime,
    /// 操作类型
    pub operation: Operation,
    /// 缓存键
    pub key: String,
    /// 缓存值（字节数组）
    pub value: Option<Vec<u8>>,
    /// 过期时间（秒）
    pub ttl: Option<i64>,
}

/// 操作类型枚举
///
/// 定义WAL支持的操作类型
#[derive(Debug, Clone, Copy)]
pub enum Operation {
    /// 设置操作
    Set,
    /// 删除操作
    Delete,
}

/// WAL管理器
///
/// 负责管理预写日志（Write-Ahead Log）
pub struct WalManager {
    /// SQLite数据库连接
    db: Arc<Mutex<Connection>>,
    /// 服务名称
    service_name: String,
}

impl WalManager {
    /// 创建新的WAL管理器
    ///
    /// # 参数
    ///
    /// * `service_name` - 服务名称
    ///
    /// # 返回值
    ///
    /// 返回新的WAL管理器实例或错误
    pub fn new(service_name: &str) -> Result<Self> {
        let conn = Connection::open(format!("{}_wal.db", service_name))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS wal_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                operation TEXT NOT NULL,
                key TEXT NOT NULL,
                value BLOB,
                ttl INTEGER,
                service_name TEXT NOT NULL
            )",
            [],
        )?;

        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            service_name: service_name.to_string(),
        })
    }

    /// 添加WAL条目
    ///
    /// # 参数
    ///
    /// * `entry` - 要添加的WAL条目
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    pub async fn append(&self, entry: WalEntry) -> Result<()> {
        let db = self.db.clone();
        let service = self.service_name.clone();

        tokio::task::spawn_blocking(move || {
            let conn = db.lock().unwrap();
            conn.execute(
                "INSERT INTO wal_entries (timestamp, operation, key, value, ttl, service_name) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    entry
                        .timestamp
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                    match entry.operation {
                        Operation::Set => "set",
                        Operation::Delete => "delete",
                    },
                    entry.key,
                    entry.value,
                    entry.ttl,
                    service,
                ],
            )
            .map_err(crate::error::CacheError::from)
        })
        .await
        .map_err(|e| crate::error::CacheError::IoError(std::io::Error::other(e)))??;

        Ok(())
    }

    /// 重放所有WAL条目
    ///
    /// # 参数
    ///
    /// * `l2` - L2缓存后端
    ///
    /// # 返回值
    ///
    /// 返回重放的条目数量或错误
    pub async fn replay_all<T: WalReplayableBackend>(&self, l2: &T) -> Result<usize> {
        let db = self.db.clone();
        let service = self.service_name.clone();

        let entries = tokio::task::spawn_blocking(move || {
            let conn = db.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT operation, key, value, ttl FROM wal_entries 
                 WHERE service_name = ?1 ORDER BY timestamp",
            )?;

            let rows = stmt.query_map([&service], |row| {
                let op_str: String = row.get(0)?;
                Ok(WalEntry {
                    timestamp: SystemTime::now(), // Not strictly needed for replay
                    operation: if op_str == "set" {
                        Operation::Set
                    } else {
                        Operation::Delete
                    },
                    key: row.get(1)?,
                    value: row.get(2)?,
                    ttl: row.get(3)?,
                })
            })?;

            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok::<Vec<WalEntry>, crate::error::CacheError>(result)
        })
        .await
        .map_err(|e| crate::error::CacheError::IoError(std::io::Error::other(e)))??;

        let count = entries.len();
        if count > 0 {
            l2.pipeline_replay(entries).await?;
            self.clear().await?;
        }

        Ok(count)
    }

    /// 清空WAL
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    pub async fn clear(&self) -> Result<()> {
        let db = self.db.clone();
        let service = self.service_name.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.lock().unwrap();
            conn.execute(
                "DELETE FROM wal_entries WHERE service_name = ?1",
                params![service],
            )
        })
        .await
        .map_err(|e| crate::error::CacheError::IoError(std::io::Error::other(e)))??;
        Ok(())
    }
}
