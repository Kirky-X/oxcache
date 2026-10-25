// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! redb 嵌入式磁盘持久化后端（absorb-hitbox-features，对标 hitbox-feoxdb 定位）
//!
//! - 纯安全 Rust 引擎（redb，ACID + WAL），与 crate 根 `#![deny(unsafe_code)]` 一致
//! - 无原生 TTL：value envelope 携带过期时间戳，读路径惰性判定 + 物理删除
//!   （与 DashMap 后端同口径）；时间戳为 epoch 毫秒，跨进程重启语义正确
//! - 写路径与全表扫描（`keys` / 模式计数 / 模式删除）经 `spawn_blocking`
//!   提交，避免阻塞异步执行器；点查为 mmap 直接返回
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
use std::ops::Bound;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};

use crate::backend::BackendKind;
use crate::backend::interface::{
    CacheConnector, CachePatternOps, CacheReader, CacheSetItem, CacheWriter, keys_limit_exceeded,
};
use crate::backend::score::Scores;
use crate::core::constants::MAX_KEYS_RETURN;
use crate::error::{OxCacheError, OxCacheResult};
#[cfg(feature = "telemetry")]
use crate::i18n::messages::MSG_LOG_DISK_SWEEP_FAILED;
use crate::i18n::messages::{
    MSG_DETAIL_DISK_CORRUPT_ENVELOPE, MSG_DETAIL_DISK_REDB_CREATE_FAILED,
    MSG_DETAIL_DISK_REDB_OPEN_FAILED, t,
};

const CACHE_TABLE: TableDefinition<'static, &str, &[u8]> = TableDefinition::new("oxcache");

/// 模式删除的单批键数：redb 的删除只能在写事务内进行，键需先收集后批量
/// 删除；分批收集把内存占用压到单批量级。
const PATTERN_DELETE_BATCH: usize = 512;

/// 模式删除的扫描轮数上限（与 Redis 的 `PATTERN_SCAN_MAX_PASSES` 同口径）：
/// 一轮 = 对全表的一次完整扫描，扫完再扫一轮以捡起并发写入的键；轮数上限
/// 保证并发写入下删除必然返回（不保证穷尽）。
const PATTERN_DELETE_MAX_PASSES: usize = 8;

const ENV_HEADER_NO_EXPIRY: usize = 9; // seq(8) + flag(1)
const ENV_HEADER_EXPIRY: usize = 17; // seq(8) + flag(1) + expires_at_ms(8)

#[cfg(feature = "telemetry")]
#[inline]
fn telemetry_sweep_failed(err: &str) {
    tracing::warn!(
        target: "oxcache::disk",
        err,
        "{}",
        t(MSG_LOG_DISK_SWEEP_FAILED, &[])
    );
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
        let db = Database::open(path.as_ref()).map_err(|e| {
            OxCacheError::DatabaseError(t(
                MSG_DETAIL_DISK_REDB_OPEN_FAILED,
                &[("err", e.to_string())],
            ))
        })?;
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
        let db = Database::create(path.as_ref()).map_err(|e| {
            OxCacheError::DatabaseError(t(
                MSG_DETAIL_DISK_REDB_CREATE_FAILED,
                &[("err", e.to_string())],
            ))
        })?;
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
                    OxCacheError::DatabaseError(t(MSG_DETAIL_DISK_CORRUPT_ENVELOPE, &[]))
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

/// 本轮扫描的边界：表中的最大键（扫描上界）与键数（访问预算）。
///
/// 两者取自本轮开始时的快照：并发写入的键既不计入访问预算，也可能落在
/// 键序上界之下（扫描仍会经过它们，但不因此无限延长）。
fn table_scan_bound(db: &Database) -> OxCacheResult<(Option<String>, u64)> {
    let read_txn = db.begin_read().map_err(map_err)?;
    let table = match read_txn.open_table(CACHE_TABLE) {
        Ok(table) => table,
        // 新库尚无任何写入：表不存在视为空
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok((None, 0)),
        Err(e) => return Err(map_err(e)),
    };
    let bounds: (Bound<&str>, Bound<&str>) = (Bound::Unbounded, Bound::Unbounded);
    let mut range = table.range::<&str>(bounds).map_err(map_err)?;
    let max_key = match range.next_back() {
        Some(Ok((k, _))) => Some(k.value().to_string()),
        Some(Err(e)) => return Err(map_err(e)),
        None => None,
    };
    let len = table.len().map_err(map_err)?;
    Ok((max_key, len))
}

/// 收集一批匹配键（至多 `PATTERN_DELETE_BATCH` 个）、续扫游标与已访问键数。
///
/// 游标取本次扫描访问到的最后一个键（键序单调递增），下一批从它之后继续，
/// 全表只需扫描一遍而非每批从头重扫。`from` 为 `None` 时从表头开始；
/// `upper` 为本轮扫描的键序上界（到达即止）；`max_visits` 为本轮剩余的
/// 访问预算，用尽即止（并发写入持续补键时靠它保证扫描收敛）。
fn collect_pattern_batch(
    db: &Database,
    pattern: &str,
    from: Option<&str>,
    upper: Option<&str>,
    max_visits: usize,
) -> OxCacheResult<(Vec<String>, Option<String>, usize)> {
    let read_txn = db.begin_read().map_err(map_err)?;
    let table = match read_txn.open_table(CACHE_TABLE) {
        Ok(table) => table,
        // 新库尚无任何写入：表不存在视为空
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok((Vec::new(), None, 0)),
        Err(e) => return Err(map_err(e)),
    };
    let now = now_epoch_ms();
    let mut batch: Vec<String> = Vec::new();
    let mut last_key: Option<String> = None;
    let mut visited = 0usize;
    let lower: Bound<&str> = match from {
        Some(last) => Bound::Excluded(last),
        None => Bound::Unbounded,
    };
    for item in table
        .range::<&str>((lower, Bound::Unbounded))
        .map_err(map_err)?
    {
        if visited >= max_visits {
            break;
        }
        let (k, v) = item.map_err(map_err)?;
        let key = k.value();
        visited += 1;
        last_key = Some(key.to_string());
        if let Some((_, expiry, _)) = decode_value(v.value())
            && !is_expired(expiry, now)
            && crate::backend::interface::glob_match(pattern, key)
        {
            batch.push(key.to_string());
            if batch.len() >= PATTERN_DELETE_BATCH {
                break;
            }
        }
        // 到达本轮键序上界：本轮扫描到此为止，之后写入的键留给下一轮
        if let Some(max) = upper
            && key >= max
        {
            break;
        }
    }
    Ok((batch, last_key, visited))
}

/// 批量删除匹配键（单写事务），返回实际删除条数。
///
/// 键可能已被并发删除（懒过期物理删除 / 并发 delete），故以 `remove` 的
/// 返回值为准，不按批长度计数。
fn remove_pattern_keys(db: &Database, keys: &[String]) -> OxCacheResult<u64> {
    let write_txn = db.begin_write().map_err(map_err)?;
    let mut removed = 0u64;
    {
        let mut table = write_txn.open_table(CACHE_TABLE).map_err(map_err)?;
        for k in keys {
            if table.remove(k.as_str()).map_err(map_err)?.is_some() {
                removed += 1;
            }
        }
    }
    write_txn.commit().map_err(map_err)?;
    Ok(removed)
}

/// 模式删除（阻塞实现）：游标续扫 + 分批写事务删除。
///
/// 一轮 = 对全表的一次完整扫描（键序游标续扫，表只扫一遍）：本轮的访问
/// 预算取自轮开始时的键数，扫描最多访问该数量的键，故并发写入持续补键
/// 也拉不长本轮；轮内无删除即结束，轮数达 `PATTERN_DELETE_MAX_PASSES`
/// 时提前返回，故并发写入下必然返回（不保证穷尽）。
fn delete_by_pattern_blocking(db: &Database, pattern: &str) -> OxCacheResult<u64> {
    let mut deleted = 0u64;
    for _ in 0..PATTERN_DELETE_MAX_PASSES {
        let (upper, key_count) = table_scan_bound(db)?;
        let budget = key_count as usize;
        let mut cursor: Option<String> = None;
        let mut visited = 0usize;
        let mut pass_deleted = 0u64;
        loop {
            if visited >= budget {
                break;
            }
            let (batch, next_cursor, visits) = collect_pattern_batch(
                db,
                pattern,
                cursor.as_deref(),
                upper.as_deref(),
                budget - visited,
            )?;
            visited += visits;
            if batch.is_empty() {
                break;
            }
            let removed = remove_pattern_keys(db, &batch)?;
            pass_deleted += removed;
            deleted += removed;
            cursor = next_cursor;
        }
        // 本轮无删除：并发写入方未再补匹配键，提前结束
        if pass_deleted == 0 {
            break;
        }
    }
    Ok(deleted)
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
        // 只暴露文件名：绝对路径会随 stats() 进入 health/metrics 响应体，
        // 泄露部署目录布局。需要完整路径时用诊断接口 path()。
        let file_name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "disk".to_string());
        stats.insert("path".to_string(), file_name);
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
        // 模式长度上限：全表扫描逐键匹配，超长模式会把单次操作的开销放大到
        // 与模式长度成正比（本地 glob 匹配逐键比较 + 逐键展开模式）
        crate::security::validate_glob_pattern(pattern)?;
        let db = self.db.clone();
        let pattern = pattern.to_string();
        // 全表扫描是 O(表大小) 的 CPU + 页错误开销，放进 spawn_blocking
        // 以免占住异步执行器的 worker 线程
        tokio::task::spawn_blocking(move || -> OxCacheResult<Vec<String>> {
            let read_txn = db.begin_read().map_err(map_err)?;
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
                    && crate::backend::interface::glob_match(&pattern, k.value())
                {
                    // 键空间可达百万级：一次性物化全部匹配键会打满调用方内存
                    // 上限只计匹配键；恰好等于上限允许返回，超过才拒绝
                    if out.len() >= MAX_KEYS_RETURN {
                        return Err(keys_limit_exceeded(MAX_KEYS_RETURN));
                    }
                    out.push(k.value().to_string());
                }
            }
            Ok(out)
        })
        .await
        .map_err(|e| OxCacheError::DatabaseError(e.to_string()))?
    }
}

#[async_trait]
impl CachePatternOps for RedbDiskBackend {
    async fn pattern_count(&self, pattern: &str) -> OxCacheResult<u64> {
        crate::security::validate_glob_pattern(pattern)?;
        let db = self.db.clone();
        let pattern = pattern.to_string();
        // 全表扫描是 O(表大小) 的 CPU + 页错误开销，放进 spawn_blocking
        // 以免占住异步执行器的 worker 线程
        tokio::task::spawn_blocking(move || -> OxCacheResult<u64> {
            let read_txn = db.begin_read().map_err(map_err)?;
            let table = match read_txn.open_table(CACHE_TABLE) {
                Ok(table) => table,
                Err(redb::TableError::TableDoesNotExist(_)) => return Ok(0),
                Err(e) => return Err(map_err(e)),
            };
            let now = now_epoch_ms();
            let mut count = 0u64;
            for item in table.iter().map_err(map_err)? {
                let (k, v) = item.map_err(map_err)?;
                if let Some((_, expiry, _)) = decode_value(v.value())
                    && !is_expired(expiry, now)
                    && crate::backend::interface::glob_match(&pattern, k.value())
                {
                    count += 1;
                }
            }
            Ok(count)
        })
        .await
        .map_err(|e| OxCacheError::DatabaseError(e.to_string()))?
    }

    /// 按模式删除：键在 redb 中只能经写事务删除，故先收集匹配键再批量删除。
    /// 收集与删除分批进行，内存占用与单批键数同阶而非全量键数。
    ///
    /// 扫描以键序游标续扫（每批从上一批末尾之后继续），一轮内全表只扫
    /// 一遍；删除不保证覆盖并发新增键（同 Redis SCAN 的既有限次语义）：
    /// 扫描轮数达 `PATTERN_DELETE_MAX_PASSES` 时提前返回已删除条数。
    async fn pattern_delete(&self, pattern: &str) -> OxCacheResult<u64> {
        crate::security::validate_glob_pattern(pattern)?;
        let db = self.db.clone();
        let pattern = pattern.to_string();
        tokio::task::spawn_blocking(move || delete_by_pattern_blocking(&db, &pattern))
            .await
            .map_err(|e| OxCacheError::DatabaseError(e.to_string()))?
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

    fn as_pattern_ops(&self) -> Option<&dyn CachePatternOps> {
        Some(self)
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

    /// 墙钟阶跃容差：过期判定与 remaining 读数都基于墙钟（见 wait_expired），
    /// 阶跃落在写入→读取间隙会让 remaining 偏离请求值，断言放行该量级偏差。
    const CLOCK_STEP_TOLERANCE: Duration = Duration::from_secs(2);

    /// 轮询等待条目过期（惰性删除以读触发）。过期时间戳是墙钟（epoch ms，
    /// 为跨进程重启语义刻意选择），而测试等待是单调钟——系统时钟阶跃
    /// （WSL2 宿主休眠恢复后常见，实测秒级回拨）会让固定 sleep 后条目尚未
    /// 到期，须以单调钟为截止的轮询收敛判定过期。
    async fn wait_expired(backend: &RedbDiskBackend, key: &str, ttl: Duration) -> bool {
        let deadline = Instant::now() + ttl + Duration::from_secs(5);
        loop {
            if backend.get(key).await.unwrap().is_none() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    // 基础 CRUD 与持久化（R-disk-001）

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
        assert!(remaining <= Duration::from_millis(300) + CLOCK_STEP_TOLERANCE);
        assert!(
            wait_expired(&backend, "ttl", Duration::from_millis(300)).await,
            "entry must expire after per-entry ttl"
        );
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
        assert!(
            wait_expired(&backend, "d", Duration::from_millis(500)).await,
            "entry must expire under the default ttl"
        );
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
        let reopened = RedbDiskBackend::open(&path).unwrap();
        assert_eq!(
            reopened.get("keep").await.unwrap(),
            Some(vec![7, 7]),
            "fresh entry must survive reopen"
        );
        assert!(
            wait_expired(&reopened, "gone", Duration::from_millis(30)).await,
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
        assert!(
            backend
                .ttl("e")
                .await
                .unwrap()
                .is_some_and(|d| d + CLOCK_STEP_TOLERANCE > Duration::from_secs(5)),
            "expire must apply the requested ttl"
        );
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

    // 有界性清扫（R-disk-003）

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

    // 分层接线与 BackendScore（R-disk-004）

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

    // 模式级操作（计数 / 删除）

    #[tokio::test]
    async fn count_by_pattern_matches_subset() {
        use crate::backend::CacheBackend;

        let (_dir, backend) = temp_db();
        let items: Vec<CacheSetItem> = vec![
            (k("user:1"), Arc::new(vec![1]), None),
            (k("user:2"), Arc::new(vec![2]), None),
            (k("order:1"), Arc::new(vec![3]), None),
        ];
        backend.set_many(&items).await.unwrap();
        assert_eq!(backend.count_by_pattern("user:*").await.unwrap(), 2);
        assert_eq!(backend.count_by_pattern("order:*").await.unwrap(), 1);
        assert_eq!(backend.count_by_pattern("*").await.unwrap(), 3);
        assert_eq!(backend.count_by_pattern("none:*").await.unwrap(), 0);
    }

    /// 模式长度超限的三个入口（枚举 / 计数 / 删除）一律拒绝：本地 glob
    /// 匹配逐键比较且逐键展开模式，超长模式会把单次全表扫描的开销放大到
    /// 与模式长度成正比。
    #[tokio::test]
    async fn pattern_ops_reject_overlong_pattern() {
        use crate::backend::CacheBackend;

        let (_dir, backend) = temp_db();
        backend
            .set(k("user:1"), Arc::new(vec![1]), None)
            .await
            .unwrap();
        let overlong = "x".repeat(crate::security::MAX_GLOB_PATTERN_LENGTH + 1);

        assert!(backend.keys(&overlong).await.is_err());
        assert!(backend.count_by_pattern(&overlong).await.is_err());
        assert!(backend.delete_by_pattern(&overlong).await.is_err());
        // 拒绝发生在扫描前：数据未被触碰
        assert_eq!(backend.count_by_pattern("user:*").await.unwrap(), 1);
    }

    /// 模式长度恰好等于上限时正常放行（边界口径：超限才拒）
    #[tokio::test]
    async fn pattern_at_limit_is_accepted() {
        use crate::backend::CacheBackend;

        let (_dir, backend) = temp_db();
        backend
            .set(k("user:1"), Arc::new(vec![1]), None)
            .await
            .unwrap();
        let at_limit = "x".repeat(crate::security::MAX_GLOB_PATTERN_LENGTH);

        assert_eq!(backend.count_by_pattern(&at_limit).await.unwrap(), 0);
        assert_eq!(backend.keys(&at_limit).await.unwrap().len(), 0);
    }

    /// 过期键既不计入模式计数，也不被模式删除物理删除（惰性过期口径：
    /// 读到才删）；删除返回条数只计实际删掉的非过期键。
    #[tokio::test]
    async fn count_and_delete_skip_expired() {
        use crate::backend::CacheBackend;

        let (_dir, backend) = temp_db();
        backend
            .set(
                k("user:exp"),
                Arc::new(vec![1]),
                Some(Duration::from_millis(50)),
            )
            .await
            .unwrap();
        backend
            .set(k("user:live"), Arc::new(vec![2]), None)
            .await
            .unwrap();
        // 等待到期但不读值：读会触发懒过期物理删除，改变被测语义
        let deadline = Instant::now() + Duration::from_secs(5);
        while backend.ttl("user:exp").await.unwrap().is_some() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            backend.ttl("user:exp").await.unwrap().is_none(),
            "entry must be expired"
        );
        assert_eq!(backend.count_by_pattern("user:*").await.unwrap(), 1);
        assert_eq!(backend.delete_by_pattern("user:*").await.unwrap(), 1);
        // 过期键仍在表内（未被模式删除物理删除），非过期键已删
        assert_eq!(backend.len().await.unwrap(), 1);
        assert!(backend.get("user:live").await.unwrap().is_none());
    }

    /// 删除条数 = 实际被删的键数：匹配键全消失、非匹配键保留，重复删除
    /// 返回 0（键已不存在不重复计入）。
    #[tokio::test]
    async fn delete_by_pattern_returns_actual_count() {
        use crate::backend::CacheBackend;

        let (_dir, backend) = temp_db();
        let items: Vec<CacheSetItem> = vec![
            (k("user:1"), Arc::new(vec![1]), None),
            (k("user:2"), Arc::new(vec![2]), None),
            (k("order:1"), Arc::new(vec![3]), None),
        ];
        backend.set_many(&items).await.unwrap();
        assert_eq!(backend.delete_by_pattern("user:*").await.unwrap(), 2);
        assert_eq!(
            backend.len().await.unwrap(),
            1,
            "only matched keys must be removed"
        );
        assert_eq!(backend.get("order:1").await.unwrap(), Some(vec![3]));
        assert_eq!(
            backend.delete_by_pattern("user:*").await.unwrap(),
            0,
            "already deleted keys must not be counted again"
        );
    }

    /// 匹配键超过单批上限时，游标续扫须跨批删净全部命中键（游标跳过或
    /// 重扫都会让返回条数对不上）。
    #[tokio::test]
    async fn delete_by_pattern_deletes_across_batches() {
        use crate::backend::CacheBackend;

        let (_dir, backend) = temp_db();
        let total = PATTERN_DELETE_BATCH + 88;
        let items: Vec<CacheSetItem> = (0..total)
            .map(|i| (k(&format!("user:{i:06}")), Arc::new(vec![1]), None))
            .collect();
        backend.set_many(&items).await.unwrap();
        assert_eq!(
            backend.delete_by_pattern("user:*").await.unwrap(),
            total as u64,
            "every matched key must be deleted across batches"
        );
        assert_eq!(backend.len().await.unwrap(), 0);
    }

    /// 删除计数以 `remove` 的返回值为准：键不存在时实际删除数为 0（按批
    /// 长度盲计会把已被并发删除的键计入返回值）。
    #[tokio::test]
    async fn remove_pattern_keys_counts_actual_removals() {
        let (_dir, backend) = temp_db();
        assert_eq!(
            remove_pattern_keys(&backend.db, &["ghost".to_string()]).unwrap(),
            0,
            "a missing key must not be counted as deleted"
        );
        backend
            .set(k("live"), Arc::new(vec![1]), None)
            .await
            .unwrap();
        assert_eq!(
            remove_pattern_keys(&backend.db, &["live".to_string()]).unwrap(),
            1,
            "an existing key must be counted once"
        );
        assert_eq!(
            remove_pattern_keys(&backend.db, &["live".to_string()]).unwrap(),
            0,
            "a key removed earlier must not be counted again"
        );
    }

    /// 键枚举上限：恰好等于上限允许返回，超过才报错。
    #[tokio::test]
    async fn keys_over_limit_errors() {
        let (_dir, over) = temp_db();
        let items: Vec<CacheSetItem> = (0..=MAX_KEYS_RETURN)
            .map(|i| (k(&format!("k:{i:06}")), Arc::new(vec![1]), None))
            .collect();
        over.set_many(&items).await.unwrap();
        let err = over.keys("*").await.unwrap_err();
        assert!(
            err.to_string().contains(&MAX_KEYS_RETURN.to_string()),
            "error must mention the limit, got {err}"
        );

        let (_dir, exact) = temp_db();
        let items: Vec<CacheSetItem> = (0..MAX_KEYS_RETURN)
            .map(|i| (k(&format!("k:{i:06}")), Arc::new(vec![1]), None))
            .collect();
        exact.set_many(&items).await.unwrap();
        assert_eq!(exact.keys("*").await.unwrap().len(), MAX_KEYS_RETURN);
    }

    /// 并发写入下模式删除必须收敛：删除不保证穷尽，但必须在有限时间内返回。
    ///
    /// 旧实现每轮从头重扫并以「本轮无命中」为退出条件；redb 单写者串行化，
    /// 多个写入方的提交落在删除方的读快照之后，下一轮必然还有命中，循环
    /// 永不退出（占用 blocking 线程，该 future 永不 resolve）。
    #[tokio::test]
    async fn delete_by_pattern_terminates_under_concurrent_writes() {
        use crate::backend::CacheBackend;
        use std::sync::atomic::AtomicBool;

        let (_dir, backend) = temp_db();
        // 种子键：保证删除方无论并发写入时序如何都有命中
        backend
            .set(k("user:seed"), Arc::new(vec![0]), None)
            .await
            .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let writers: Vec<_> = (0..4)
            .map(|w| {
                let backend = backend.clone();
                let stop = stop.clone();
                tokio::spawn(async move {
                    let mut n = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        let _ = backend
                            .set(
                                Arc::from(format!("user:{w}-{n:08}").as_str()),
                                Arc::new(vec![1]),
                                None,
                            )
                            .await;
                        n += 1;
                    }
                })
            })
            .collect();
        let deleted =
            tokio::time::timeout(Duration::from_secs(10), backend.delete_by_pattern("user:*"))
                .await
                .expect("pattern delete must return under concurrent writes")
                .unwrap();
        stop.store(true, Ordering::Relaxed);
        for w in writers {
            let _ = w.await;
        }
        assert!(deleted > 0, "concurrent writer's keys must be deleted");
    }
}
