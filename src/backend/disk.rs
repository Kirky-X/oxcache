// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! redb 嵌入式磁盘持久化后端（absorb-hitbox-features，对标 hitbox-feoxdb 定位）
//!
//! - 纯安全 Rust 引擎（redb，ACID + WAL），与 crate 根 `#![deny(unsafe_code)]` 一致
//! - 无原生 TTL：value envelope 携带过期时间戳，读路径惰性判定 + 物理删除
//!   （与 DashMap 后端同口径）；时间戳为 epoch 毫秒，跨进程重启语义正确
//! - 写路径经 `spawn_blocking` 提交，避免 fsync 阻塞异步执行器；读为 mmap 直接返回
//! - `max_entries` 超限清扫：先删过期条目，仍超限按 seq 升序删最旧（清扫每
//!   `max(1, max_entries/10)` 次 set 至多触发一次，fire-and-forget 不阻塞写返回）
//! - `Database` 为 Clone（内含 Arc），克隆共享同一存储
//!
//! # Value envelope
//!
//! ```text
//! seq: u64 (LE) | flag: u8 | expires_at_ms: u64 (LE, flag=1 时存在) | payload
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};

use crate::backend::BackendKind;
use crate::backend::interface::{CacheConnector, CacheReader, CacheSetItem, CacheWriter};
use crate::backend::score::Scores;
use crate::error::{OxCacheError, OxCacheResult};

const CACHE_TABLE: TableDefinition<'static, &str, &[u8]> = TableDefinition::new("oxcache");

const ENV_HEADER_NO_EXPIRY: usize = 9; // seq(8) + flag(1)
const ENV_HEADER_EXPIRY: usize = 17; // seq(8) + flag(1) + expires_at_ms(8)

#[cfg(feature = "telemetry")]
#[inline]
fn telemetry_sweep_failed(err: &str) {
    tracing::warn!(target: "oxcache::disk", err, "disk cache sweep failed");
}

#[cfg(not(feature = "telemetry"))]
#[inline]
fn telemetry_sweep_failed(_err: &str) {}

fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 原始 envelope 解码产物：(seq, expires_at_ms, payload)
type RawEntry = (u64, Option<u64>, Vec<u8>);

/// envelope 编码：seq + 可选过期时间 + payload
fn encode_value(seq: u64, expires_at_ms: Option<u64>, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + ENV_HEADER_EXPIRY);
    out.extend_from_slice(&seq.to_le_bytes());
    match expires_at_ms {
        Some(expiry) => {
            out.push(1u8);
            out.extend_from_slice(&expiry.to_le_bytes());
        }
        None => out.push(0u8),
    }
    out.extend_from_slice(payload);
    out
}

/// envelope 解码：返回 (seq, expires_at_ms, payload)。短读视为损坏（None）。
fn decode_value(bytes: &[u8]) -> Option<(u64, Option<u64>, &[u8])> {
    if bytes.len() < ENV_HEADER_NO_EXPIRY {
        return None;
    }
    let seq = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
    match bytes[8] {
        0 => Some((seq, None, &bytes[ENV_HEADER_NO_EXPIRY..])),
        1 => {
            if bytes.len() < ENV_HEADER_EXPIRY {
                return None;
            }
            let expiry = u64::from_le_bytes(bytes[9..17].try_into().ok()?);
            Some((seq, Some(expiry), &bytes[ENV_HEADER_EXPIRY..]))
        }
        _ => None,
    }
}

fn is_expired(expires_at_ms: Option<u64>, now_ms: u64) -> bool {
    match expires_at_ms {
        Some(expiry) => now_ms >= expiry,
        None => false,
    }
}

/// redb 磁盘持久化后端。`open` 打开既有库文件，`create` 新建（已存在则打开）。
#[derive(Clone)]
pub struct RedbDiskBackend {
    db: Arc<Database>,
    path: Arc<std::path::PathBuf>,
    default_ttl: Option<Duration>,
    max_entries: Option<u64>,
    next_seq: Arc<AtomicU64>,
    set_count: Arc<AtomicU64>,
}

impl RedbDiskBackend {
    /// 打开既有库文件（不存在时返回错误）。
    pub fn open(path: impl AsRef<std::path::Path>) -> OxCacheResult<Self> {
        let db = Database::open(path.as_ref())
            .map_err(|e| OxCacheError::DatabaseError(format!("redb open failed: {e}")))?;
        Ok(Self {
            db: Arc::new(db),
            path: Arc::new(path.as_ref().to_path_buf()),
            default_ttl: None,
            max_entries: None,
            next_seq: Arc::new(AtomicU64::new(1)),
            set_count: Arc::new(AtomicU64::new(0)),
        })
    }

    /// 新建库文件（父目录需存在；文件已存在则打开它）。
    pub fn create(path: impl AsRef<std::path::Path>) -> OxCacheResult<Self> {
        let db = Database::create(path.as_ref())
            .map_err(|e| OxCacheError::DatabaseError(format!("redb create failed: {e}")))?;
        Ok(Self {
            db: Arc::new(db),
            path: Arc::new(path.as_ref().to_path_buf()),
            default_ttl: None,
            max_entries: None,
            next_seq: Arc::new(AtomicU64::new(1)),
            set_count: Arc::new(AtomicU64::new(0)),
        })
    }

    /// 全局默认 TTL（`set(ttl=None)` 时兜底）。
    pub fn with_default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// 条目数上限（超限触发清扫）。清扫先删过期，仍超限按 seq 升序删最旧。
    pub fn with_max_entries(mut self, max_entries: u64) -> Self {
        self.max_entries = Some(max_entries);
        self
    }

    /// 库文件路径（诊断用）。
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn expiry_for(&self, ttl: Option<Duration>) -> Option<u64> {
        let ttl = ttl.or(self.default_ttl)?;
        Some(now_epoch_ms().saturating_add(ttl.as_millis() as u64))
    }

    /// 读单个 key（不判定过期，原始 envelope），供读路径共用。
    fn raw_get(&self, key: &str) -> OxCacheResult<Option<RawEntry>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| OxCacheError::DatabaseError(e.to_string()))?;
        let table = match read_txn.open_table(CACHE_TABLE) {
            Ok(table) => table,
            // 新库尚无任何写入：表不存在视为空
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(map_err(e)),
        };
        match table
            .get(key)
            .map_err(|e| OxCacheError::DatabaseError(e.to_string()))?
        {
            Some(v) => decode_value(v.value())
                .map(|(seq, expiry, payload)| (seq, expiry, payload.to_vec()))
                .ok_or_else(|| {
                    OxCacheError::DatabaseError("corrupt disk cache envelope".to_string())
                })
                .map(Some),
            None => Ok(None),
        }
    }

    /// 超限清扫（fire-and-forget）：先删过期，仍超限按 seq 升序删最旧。
    fn maybe_sweep(&self) {
        let Some(max) = self.max_entries else {
            return;
        };
        let interval = (max / 10).max(1);
        // +1：fetch_add 返回自增前的值，改为 1-based 计数，
        // 使最后一次 set（count = 总写入数）也落在触发点上
        let count = self.set_count.fetch_add(1, Ordering::Relaxed) + 1;
        if !count.is_multiple_of(interval) {
            return;
        }
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let sweep = || -> Result<(), Box<dyn std::error::Error>> {
                let write_txn = db.begin_write()?;
                {
                    let mut table = write_txn.open_table(CACHE_TABLE)?;
                    let now = now_epoch_ms();
                    // 1) 删除全部过期条目
                    let mut expired_keys: Vec<String> = Vec::new();
                    for item in table.iter()? {
                        let (k, v) = item?;
                        if let Some((_, expiry, _)) = decode_value(v.value())
                            && is_expired(expiry, now)
                        {
                            expired_keys.push(k.value().to_string());
                        }
                    }
                    for k in &expired_keys {
                        table.remove(k.as_str())?;
                    }
                    // 2) 仍超限：按 seq 升序删最旧，直至 len <= max
                    let mut over = table.len()? as i64 - max as i64;
                    if over > 0 {
                        let mut seq_keys: Vec<(u64, String)> = Vec::new();
                        for item in table.iter()? {
                            let (k, v) = item?;
                            if let Some((seq, _, _)) = decode_value(v.value()) {
                                seq_keys.push((seq, k.value().to_string()));
                            }
                        }
                        seq_keys.sort_by_key(|(seq, _)| *seq);
                        for (_, k) in seq_keys {
                            if over <= 0 {
                                break;
                            }
                            table.remove(k.as_str())?;
                            over -= 1;
                        }
                    }
                }
                write_txn.commit()?;
                Ok(())
            };
            if let Err(e) = sweep() {
                // 清扫失败不回传写路径（fire-and-forget 语义），记录告警
                telemetry_sweep_failed(&e.to_string());
            }
        });
    }
}

fn map_err<E: std::fmt::Display>(e: E) -> OxCacheError {
    OxCacheError::DatabaseError(e.to_string())
}

#[async_trait]
impl CacheReader for RedbDiskBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        let Some((_, expiry, payload)) = self.raw_get(key)? else {
            return Ok(None);
        };
        if is_expired(expiry, now_epoch_ms()) {
            // 懒过期：物理删除（与 DashMap 后端同口径）
            let db = self.db.clone();
            let k = key.to_string();
            tokio::task::spawn_blocking(move || -> OxCacheResult<()> {
                let write_txn = db.begin_write().map_err(map_err)?;
                {
                    let mut table = write_txn.open_table(CACHE_TABLE).map_err(map_err)?;
                    table.remove(k.as_str()).map_err(map_err)?;
                }
                write_txn.commit().map_err(map_err)?;
                Ok(())
            })
            .await
            .map_err(|e| OxCacheError::DatabaseError(e.to_string()))??;
            return Ok(None);
        }
        Ok(Some(payload))
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        Ok(match self.raw_get(key)? {
            Some((_, expiry, _)) => !is_expired(expiry, now_epoch_ms()),
            None => false,
        })
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        let now = now_epoch_ms();
        Ok(match self.raw_get(key)? {
            Some((_, Some(expiry), _)) if expiry > now => Some(Duration::from_millis(expiry - now)),
            _ => None,
        })
    }

    async fn len(&self) -> OxCacheResult<u64> {
        let read_txn = self.db.begin_read().map_err(map_err)?;
        let table = match read_txn.open_table(CACHE_TABLE) {
            Ok(table) => table,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(0),
            Err(e) => return Err(map_err(e)),
        };
        Ok(table.len().map_err(map_err)?)
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        Ok(self.max_entries.unwrap_or(u64::MAX))
    }

    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("backend".to_string(), "disk".to_string());
        stats.insert("path".to_string(), self.path.display().to_string());
        stats.insert("len".to_string(), self.len().await?.to_string());
        stats.insert(
            "max_entries".to_string(),
            self.max_entries
                .map(|m| m.to_string())
                .unwrap_or_else(|| "unbounded".to_string()),
        );
        Ok(stats)
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        let read_txn = self.db.begin_read().map_err(map_err)?;
        let table = match read_txn.open_table(CACHE_TABLE) {
            Ok(table) => table,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(map_err(e)),
        };
        let now = now_epoch_ms();
        let mut out = Vec::new();
        for item in table.iter().map_err(map_err)? {
            let (k, v) = item.map_err(map_err)?;
            if let Some((_, expiry, _)) = decode_value(v.value())
                && !is_expired(expiry, now)
                && crate::backend::interface::glob_match(pattern, k.value())
            {
                out.push(k.value().to_string());
            }
        }
        Ok(out)
    }
}

#[async_trait]
impl CacheWriter for RedbDiskBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let expires_at = self.expiry_for(ttl);
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let encoded = Arc::new(encode_value(seq, expires_at, &value));
        let db = self.db.clone();
        let k = key.clone();
        tokio::task::spawn_blocking(move || -> OxCacheResult<()> {
            let write_txn = db.begin_write().map_err(map_err)?;
            {
                let mut table = write_txn.open_table(CACHE_TABLE).map_err(map_err)?;
                table
                    .insert(k.as_ref(), encoded.as_slice())
                    .map_err(map_err)?;
            }
            write_txn.commit().map_err(map_err)?;
            Ok(())
        })
        .await
        .map_err(|e| OxCacheError::DatabaseError(e.to_string()))??;
        self.maybe_sweep();
        Ok(())
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        let db = self.db.clone();
        let k = key.to_string();
        tokio::task::spawn_blocking(move || -> OxCacheResult<()> {
            let write_txn = db.begin_write().map_err(map_err)?;
            {
                let mut table = write_txn.open_table(CACHE_TABLE).map_err(map_err)?;
                table.remove(k.as_str()).map_err(map_err)?;
            }
            write_txn.commit().map_err(map_err)?;
            Ok(())
        })
        .await
        .map_err(|e| OxCacheError::DatabaseError(e.to_string()))??;
        Ok(())
    }

    async fn clear(&self) -> OxCacheResult<()> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || -> OxCacheResult<()> {
            let write_txn = db.begin_write().map_err(map_err)?;
            {
                let mut table = write_txn.open_table(CACHE_TABLE).map_err(map_err)?;
                let mut keys: Vec<String> = Vec::new();
                for item in table.iter().map_err(map_err)? {
                    let (k, _) = item.map_err(map_err)?;
                    keys.push(k.value().to_string());
                }
                for k in keys {
                    table.remove(k.as_str()).map_err(map_err)?;
                }
            }
            write_txn.commit().map_err(map_err)?;
            Ok(())
        })
        .await
        .map_err(|e| OxCacheError::DatabaseError(e.to_string()))??;
        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        let Some((seq, existing_expiry, payload)) = self.raw_get(key)? else {
            return Ok(false);
        };
        if is_expired(existing_expiry, now_epoch_ms()) {
            return Ok(false);
        }
        let expires_at = now_epoch_ms().saturating_add(ttl.as_millis() as u64);
        let encoded = Arc::new(encode_value(seq, Some(expires_at), &payload));
        let db = self.db.clone();
        let k = key.to_string();
        tokio::task::spawn_blocking(move || -> OxCacheResult<()> {
            let write_txn = db.begin_write().map_err(map_err)?;
            {
                let mut table = write_txn.open_table(CACHE_TABLE).map_err(map_err)?;
                table
                    .insert(k.as_str(), encoded.as_slice())
                    .map_err(map_err)?;
            }
            write_txn.commit().map_err(map_err)?;
            Ok(())
        })
        .await
        .map_err(|e| OxCacheError::DatabaseError(e.to_string()))??;
        Ok(true)
    }

    async fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        let mut encoded: Vec<(String, Vec<u8>)> = Vec::with_capacity(items.len());
        for (key, value, ttl) in items {
            let expires_at = self.expiry_for(*ttl);
            let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
            encoded.push((key.to_string(), encode_value(seq, expires_at, value)));
        }
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || -> OxCacheResult<()> {
            let write_txn = db.begin_write().map_err(map_err)?;
            {
                let mut table = write_txn.open_table(CACHE_TABLE).map_err(map_err)?;
                for (k, v) in &encoded {
                    table.insert(k.as_str(), v.as_slice()).map_err(map_err)?;
                }
            }
            write_txn.commit().map_err(map_err)?;
            Ok(())
        })
        .await
        .map_err(|e| OxCacheError::DatabaseError(e.to_string()))??;
        self.maybe_sweep();
        Ok(())
    }
}

#[async_trait]
impl CacheConnector for RedbDiskBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        // 打开一个读事务即可证明库文件可用；新库无表视为健康
        let read_txn = self.db.begin_read().map_err(map_err)?;
        match read_txn.open_table(CACHE_TABLE) {
            Ok(_) | Err(redb::TableError::TableDoesNotExist(_)) => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }

    async fn shutdown(&self) {
        // redb 无显式关闭 API；每次提交即落盘
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Disk
    }
}

impl crate::backend::score::BackendScore for RedbDiskBackend {
    fn score(&self) -> u8 {
        Scores::REDB
    }

    fn is_persistent(&self) -> bool {
        true
    }

    fn backend_name(&self) -> &'static str {
        "disk"
    }
}

#[cfg(all(test, feature = "disk"))]
mod tests {
    use super::*;
    use std::time::Instant;

    fn temp_db() -> (tempfile::TempDir, RedbDiskBackend) {
        let dir = tempfile::tempdir().unwrap();
        let backend = RedbDiskBackend::create(dir.path().join("cache.redb")).unwrap();
        (dir, backend)
    }

    fn k(s: &str) -> Arc<str> {
        Arc::from(s)
    }

    // ========================================================================
    // 基础 CRUD 与持久化（R-disk-001）
    // ========================================================================

    #[tokio::test]
    async fn set_get_delete_roundtrip() {
        let (_dir, backend) = temp_db();
        backend
            .set(k("a"), Arc::new(vec![1, 2, 3]), None)
            .await
            .unwrap();
        assert_eq!(backend.get("a").await.unwrap(), Some(vec![1, 2, 3]));
        assert!(backend.exists("a").await.unwrap());
        backend.delete("a").await.unwrap();
        assert_eq!(backend.get("a").await.unwrap(), None);
        assert!(!backend.exists("a").await.unwrap());
        assert_eq!(backend.len().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn per_entry_ttl_lazy_expiry_physically_deletes() {
        let (_dir, backend) = temp_db();
        // TTL 取较大窗口：插桩/慢环境下 set→ttl 查询间隙也可能超过数十毫秒
        backend
            .set(
                k("ttl"),
                Arc::new(vec![9]),
                Some(Duration::from_millis(300)),
            )
            .await
            .unwrap();
        let remaining = backend.ttl("ttl").await.unwrap().unwrap();
        assert!(remaining <= Duration::from_millis(300));
        tokio::time::sleep(Duration::from_millis(350)).await;
        // 懒过期：读取触发物理删除
        assert_eq!(backend.get("ttl").await.unwrap(), None);
        assert!(!backend.exists("ttl").await.unwrap());
        assert_eq!(backend.ttl("ttl").await.unwrap(), None);
        assert_eq!(
            backend.len().await.unwrap(),
            0,
            "expired entry must be removed"
        );
    }

    #[tokio::test]
    async fn default_ttl_applies_when_set_ttl_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let backend = RedbDiskBackend::create(dir.path().join("c.redb"))
            .unwrap()
            .with_default_ttl(Duration::from_millis(500));
        backend.set(k("d"), Arc::new(vec![1]), None).await.unwrap();
        assert!(backend.ttl("d").await.unwrap().is_some());
        tokio::time::sleep(Duration::from_millis(550)).await;
        assert_eq!(backend.get("d").await.unwrap(), None);
    }

    #[tokio::test]
    async fn persistence_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("persist.redb");
        {
            let backend = RedbDiskBackend::create(&path).unwrap();
            backend
                .set(k("keep"), Arc::new(vec![7, 7]), None)
                .await
                .unwrap();
            backend
                .set(
                    k("gone"),
                    Arc::new(vec![1]),
                    Some(Duration::from_millis(30)),
                )
                .await
                .unwrap();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        let reopened = RedbDiskBackend::open(&path).unwrap();
        assert_eq!(
            reopened.get("keep").await.unwrap(),
            Some(vec![7, 7]),
            "fresh entry must survive reopen"
        );
        assert_eq!(
            reopened.get("gone").await.unwrap(),
            None,
            "expired entry must be treated as absent after reopen"
        );
    }

    #[tokio::test]
    async fn expire_updates_remaining_ttl() {
        let (_dir, backend) = temp_db();
        backend.set(k("e"), Arc::new(vec![1]), None).await.unwrap();
        assert!(backend.ttl("e").await.unwrap().is_none());
        assert!(
            !backend
                .expire("missing", Duration::from_secs(5))
                .await
                .unwrap(),
            "expire on absent key returns false"
        );
        assert!(backend.expire("e", Duration::from_secs(5)).await.unwrap());
        assert!(backend.ttl("e").await.unwrap().unwrap() > Duration::from_secs(4));
    }

    #[tokio::test]
    async fn clear_and_set_many_and_keys_glob() {
        let (_dir, backend) = temp_db();
        let items: Vec<CacheSetItem> = vec![
            (k("user:1"), Arc::new(vec![1]), None),
            (k("user:2"), Arc::new(vec![2]), None),
            (k("order:1"), Arc::new(vec![3]), None),
        ];
        backend.set_many(&items).await.unwrap();
        let keys = backend.keys("user:*").await.unwrap();
        assert_eq!(keys.len(), 2, "glob filter on disk keys, got {keys:?}");
        backend.clear().await.unwrap();
        assert_eq!(backend.len().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn health_check_and_backend_kind() {
        let (_dir, backend) = temp_db();
        backend.health_check().await.unwrap();
        assert!(matches!(backend.backend_kind(), BackendKind::Disk));
    }

    // ========================================================================
    // 有界性清扫（R-disk-003）
    // ========================================================================

    #[tokio::test]
    async fn max_entries_sweep_keeps_newest() {
        let dir = tempfile::tempdir().unwrap();
        let backend = RedbDiskBackend::create(dir.path().join("c.redb"))
            .unwrap()
            .with_max_entries(100);
        for i in 0..120u64 {
            backend
                .set(
                    Arc::from(format!("key-{i:03}").as_str()),
                    Arc::new(vec![i as u8]),
                    None,
                )
                .await
                .unwrap();
        }
        // 清扫为 fire-and-forget：轮询等待收敛
        let deadline = Instant::now() + Duration::from_secs(5);
        while backend.len().await.unwrap() > 100 && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            backend.len().await.unwrap() <= 100,
            "sweep must cap entries at max_entries"
        );
        // 最旧（seq 最小）被删，最新保留
        assert_eq!(
            backend.get("key-119").await.unwrap(),
            Some(vec![119]),
            "newest entries must survive"
        );
        assert_eq!(
            backend.get("key-000").await.unwrap(),
            None,
            "oldest entries must be swept first"
        );
    }

    #[tokio::test]
    async fn sweep_prefers_deleting_expired_entries() {
        let dir = tempfile::tempdir().unwrap();
        let backend = RedbDiskBackend::create(dir.path().join("c.redb"))
            .unwrap()
            .with_max_entries(100);
        // 50 条立即过期 + 100 条新鲜；触发阈值时过期条目应先消失
        for i in 0..50u64 {
            backend
                .set(
                    Arc::from(format!("exp-{i}").as_str()),
                    Arc::new(vec![0]),
                    Some(Duration::from_millis(20)),
                )
                .await
                .unwrap();
        }
        for i in 0..100u64 {
            backend
                .set(
                    Arc::from(format!("fresh-{i:03}").as_str()),
                    Arc::new(vec![1]),
                    None,
                )
                .await
                .unwrap();
        }
        tokio::time::sleep(Duration::from_millis(60)).await; // 令 exp-* 过期
        let deadline = Instant::now() + Duration::from_secs(5);
        while backend.len().await.unwrap() > 100 && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(backend.len().await.unwrap() <= 100);
        assert_eq!(
            backend.get("exp-0").await.unwrap(),
            None,
            "expired entries must be swept before fresh ones"
        );
        assert_eq!(backend.get("fresh-099").await.unwrap(), Some(vec![1]));
    }

    // ========================================================================
    // 分层接线与 BackendScore（R-disk-004）
    // ========================================================================

    #[test]
    fn backend_score_is_l3_persistent() {
        use crate::backend::score::BackendScore;
        let dir = tempfile::tempdir().unwrap();
        let backend = RedbDiskBackend::create(dir.path().join("c.redb")).unwrap();
        assert_eq!(BackendScore::score(&backend), Scores::REDB);
        assert_eq!(Scores::REDB, 85);
        assert!(BackendScore::is_persistent(&backend));
        assert_eq!(BackendScore::backend_name(&backend), "disk");
    }

    /// 磁盘层以 L3 成员挂链：L1 未命中时读穿透命中磁盘层，且回填启用时
    /// 回填 L1（enable_backfill）。
    #[tokio::test]
    async fn disk_backend_joins_chain_as_l3_with_backfill() {
        use crate::backend::{CacheBackend, MokaMemoryBackend};
        use crate::cache::chain::ChainLink;

        let dir = tempfile::tempdir().unwrap();
        let disk = Arc::new(RedbDiskBackend::create(dir.path().join("c.redb")).unwrap());
        let l1 = Arc::new(MokaMemoryBackend::new());

        let l1_link = ChainLink::from_arc(l1.clone(), Scores::MOKA, false, "moka");
        let disk_link = ChainLink::from_arc(
            disk.clone() as Arc<dyn CacheBackend>,
            Scores::REDB,
            true,
            "disk",
        );
        let chain = crate::cache::chain::ChainCache::builder()
            .link(l1_link)
            .link(disk_link)
            .enable_backfill()
            .build();

        // 直写磁盘层（绕过链写路径，模拟 L1 失效后的存量数据）
        disk.set(k("hot"), Arc::new(vec![42]), None).await.unwrap();
        assert_eq!(l1.get("hot").await.unwrap(), None, "L1 must start cold");

        // 链读：L1 miss → 磁盘层命中
        let v = chain.get("hot").await.unwrap();
        assert_eq!(v, Some(vec![42]), "chain read must reach the disk layer");

        // 回填 L1（fire-and-forget，轮询等待）
        let deadline = Instant::now() + Duration::from_secs(5);
        while l1.get("hot").await.unwrap().is_none() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            l1.get("hot").await.unwrap(),
            Some(vec![42]),
            "disk hit must backfill into L1"
        );
    }
}
