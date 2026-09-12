// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 版本化 CAS 写
//!
//! 与 [`AtomicCacheWriter::compare_and_swap`](crate::backend::AtomicCacheWriter)
//! 的**按值比较**不同，这里基于**版本号**：
//!
//! - `compare_and_swap(key, expect_version, new_value)`：当前版本等于
//!   `expect_version` 才写入，成功返回新版本；不匹配返回 `None`——
//!   解决"相同值不同意图"场景与丢失更新（lost update）问题；
//! - L1：[`MemoryVersionedCache`]（Mutex 保护的版本表，单测覆盖）；
//! - L2：[`RedisVersionedCache`]（`WATCH` + `MULTI/EXEC` 事务 MVP，
//!   存储信封 `[8B 版本][payload]`；事务被并发改动打断返回 `None`）。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::features::versioning::MemoryVersionedCache;
//!
//! let store = MemoryVersionedCache::new();
//! let v1 = store.compare_and_swap("k", 0, b"first".to_vec(), None).await?.unwrap();
//! let v2 = store.compare_and_swap("k", v1, b"second".to_vec(), None).await?.unwrap();
//! assert!(store.compare_and_swap("k", v1, b"stale".to_vec(), None).await?.is_none());
//! ```

use crate::error::{OxCacheError, OxCacheResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 版本化值快照
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedValue {
    /// 当前版本（从 1 开始；0 = 未创建）
    pub version: u64,
    /// 值字节
    pub data: Vec<u8>,
}

/// 版本化存储端口
#[async_trait]
pub trait VersionedStore: Send + Sync {
    /// 当前版本（不存在为 None）
    async fn version(&self, key: &str) -> OxCacheResult<Option<u64>>;

    /// 版本化 CAS：`expect_version` 匹配才写入。
    ///
    /// 返回 `Ok(Some(new_version))` 写入成功；`Ok(None)` 版本不匹配（调用方
    /// 重读后重试）。
    async fn compare_and_swap(
        &self,
        key: &str,
        expect_version: u64,
        new_value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<Option<u64>>;

    /// 读取当前版本与值
    async fn get_versioned(&self, key: &str) -> OxCacheResult<Option<VersionedValue>>;

    /// 删除（重置版本）
    async fn delete(&self, key: &str) -> OxCacheResult<()>;
}

// ============================================================================
// L1 内存实现
// ============================================================================

/// L1 内存版本化缓存（Mutex 保护的版本表）
#[derive(Default)]
pub struct MemoryVersionedCache {
    entries: Mutex<HashMap<String, VersionedValue>>,
}

impl MemoryVersionedCache {
    pub fn new() -> Self {
        Self::default()
    }

    // 仅供本模块测试断言失败路径不残留条目；生产路径经 VersionedStore trait。
    #[cfg(test)]
    fn entry_count(&self) -> usize {
        self.entries.lock().map(|m| m.len()).unwrap_or(0)
    }
}

#[async_trait]
impl VersionedStore for MemoryVersionedCache {
    async fn version(&self, key: &str) -> OxCacheResult<Option<u64>> {
        Ok(self
            .entries
            .lock()
            .map_err(|_| OxCacheError::Operation("version store poisoned".to_string()))?
            .get(key)
            .map(|v| v.version))
    }

    async fn compare_and_swap(
        &self,
        key: &str,
        expect_version: u64,
        new_value: Vec<u8>,
        _ttl: Option<Duration>,
    ) -> OxCacheResult<Option<u64>> {
        let mut map = self
            .entries
            .lock()
            .map_err(|_| OxCacheError::Operation("version store poisoned".to_string()))?;
        match map.get(key) {
            Some(current) if current.version == expect_version => {
                let new_version = current.version + 1;
                map.insert(
                    key.to_string(),
                    VersionedValue {
                        version: new_version,
                        data: new_value,
                    },
                );
                Ok(Some(new_version))
            }
            Some(_) => Ok(None),
            None if expect_version == 0 => {
                // 版本 0 = 期望不存在（创建语义）
                map.insert(
                    key.to_string(),
                    VersionedValue {
                        version: 1,
                        data: new_value,
                    },
                );
                Ok(Some(1))
            }
            None => Ok(None),
        }
    }

    async fn get_versioned(&self, key: &str) -> OxCacheResult<Option<VersionedValue>> {
        Ok(self
            .entries
            .lock()
            .map_err(|_| OxCacheError::Operation("version store poisoned".to_string()))?
            .get(key)
            .cloned())
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.entries
            .lock()
            .map_err(|_| OxCacheError::Operation("version store poisoned".to_string()))?
            .remove(key);
        Ok(())
    }
}

// ============================================================================
// L2 Redis WATCH 实现（MVP）
// ============================================================================

/// L2 Redis 版本化缓存（WATCH + MULTI/EXEC 事务 MVP）
///
/// 存储信封：`[8B 大端版本][payload]`。WATCH 命中键后读当前版本，事务内
/// 写入新版本；EXEC 被打断（并发改动）返回 `None`（单次尝试，调用方重试）。
#[cfg(feature = "redis")]
pub struct RedisVersionedCache {
    backend: Arc<crate::backend::RedisBackend>,
}

#[cfg(feature = "redis")]
impl RedisVersionedCache {
    pub fn new(backend: Arc<crate::backend::RedisBackend>) -> Self {
        Self { backend }
    }

    fn encode(version: u64, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + data.len());
        out.extend_from_slice(&version.to_be_bytes());
        out.extend_from_slice(data);
        out
    }

    fn decode(raw: &[u8]) -> OxCacheResult<VersionedValue> {
        if raw.len() < 8 {
            return Err(OxCacheError::Operation(
                "versioned envelope too short".to_string(),
            ));
        }
        let mut version_bytes = [0u8; 8];
        version_bytes.copy_from_slice(&raw[..8]);
        Ok(VersionedValue {
            version: u64::from_be_bytes(version_bytes),
            data: raw[8..].to_vec(),
        })
    }
}

#[cfg(feature = "redis")]
#[async_trait]
impl VersionedStore for RedisVersionedCache {
    async fn version(&self, key: &str) -> OxCacheResult<Option<u64>> {
        Ok(self.get_versioned(key).await?.map(|v| v.version))
    }

    async fn compare_and_swap(
        &self,
        key: &str,
        expect_version: u64,
        new_value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<Option<u64>> {

        let mut conn = self.backend.conn();

        // 1. WATCH：事务化的乐观锁
        redis::cmd("WATCH")
            .arg(key)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("versioned WATCH failed: {e}")))?;

        // 2. 读当前信封，校验版本
        let current: Option<Vec<u8>> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("versioned GET failed: {e}")))?;
        let current_version = match current.as_deref().map(Self::decode).transpose()? {
            Some(v) => v.version,
            None => 0,
        };
        if current_version != expect_version {
            // 版本不匹配：UNWATCH 并放弃
            redis::cmd("UNWATCH")
                .query_async::<()>(&mut conn)
                .await
                .ok();
            return Ok(None);
        }

        let new_version = current_version + 1;
        let envelope = Self::encode(new_version, &new_value);

        // 3. MULTI/EXEC
        redis::cmd("MULTI")
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("versioned MULTI failed: {e}")))?;
        let mut set_cmd = redis::cmd("SET");
        set_cmd.arg(key).arg(envelope);
        if let Some(ttl) = ttl {
            set_cmd.arg("EX").arg(ttl.as_secs().max(1));
        }
        let _: Result<(), _> = set_cmd.query_async(&mut conn).await;
        let exec: Result<Option<Vec<redis::Value>>, _> =
            redis::cmd("EXEC").query_async(&mut conn).await;
        match exec {
            Ok(Some(_)) => Ok(Some(new_version)),
            // Nil = 事务被打断（键在 WATCH 后被他人修改）
            Ok(None) => Ok(None),
            Err(e) => Err(OxCacheError::Operation(format!(
                "versioned EXEC failed: {e}"
            ))),
        }
    }

    async fn get_versioned(&self, key: &str) -> OxCacheResult<Option<VersionedValue>> {
        let mut conn = self.backend.conn();
        let raw: Option<Vec<u8>> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("versioned GET failed: {e}")))?;
        match raw {
            Some(bytes) => Ok(Some(Self::decode(&bytes)?)),
            None => Ok(None),
        }
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        let mut conn = self.backend.conn();
        let _: i64 = redis::cmd("DEL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("versioned DEL failed: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc as StdArc;

    #[tokio::test]
    async fn memory_create_with_version_zero() {
        let store = MemoryVersionedCache::new();
        assert_eq!(store.version("k").await.unwrap(), None);

        // expect 0 = 创建
        let v1 = store
            .compare_and_swap("k", 0, b"first".to_vec(), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v1, 1);
        let value = store.get_versioned("k").await.unwrap().unwrap();
        assert_eq!(value.version, 1);
        assert_eq!(value.data, b"first".to_vec());
    }

    #[tokio::test]
    async fn memory_cas_bumps_version_and_detects_mismatch() {
        let store = MemoryVersionedCache::new();
        let v1 = store
            .compare_and_swap("k", 0, b"a".to_vec(), None)
            .await
            .unwrap()
            .unwrap();
        let v2 = store
            .compare_and_swap("k", v1, b"b".to_vec(), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v2, 2);

        // 过期版本写入失败
        assert_eq!(
            store
                .compare_and_swap("k", v1, b"stale".to_vec(), None)
                .await
                .unwrap(),
            None,
            "过期版本必须被拒绝"
        );
        // 值未被脏写
        assert_eq!(store.get_versioned("k").await.unwrap().unwrap().data, b"b".to_vec());
    }

    /// lost-update 场景：两方以同一期望版本并发写入，仅一方成功
    #[tokio::test]
    async fn concurrent_writers_no_lost_update() {
        let store = StdArc::new(MemoryVersionedCache::new());
        store
            .compare_and_swap("counter", 0, b"init".to_vec(), None)
            .await
            .unwrap()
            .unwrap();

        let a = store.clone();
        let b = store.clone();
        let (ra, rb) = tokio::join!(
            a.compare_and_swap("counter", 1, b"writer-a".to_vec(), None),
            b.compare_and_swap("counter", 1, b"writer-b".to_vec(), None),
        );

        let wins = [ra.unwrap(), rb.unwrap()].into_iter().flatten().count();
        assert_eq!(wins, 1, "同版本并发写只能有一方成功");
        assert_eq!(store.version("counter").await.unwrap(), Some(2));
    }

    #[tokio::test]
    async fn delete_resets_version() {
        let store = MemoryVersionedCache::new();
        store
            .compare_and_swap("k", 0, b"v".to_vec(), None)
            .await
            .unwrap()
            .unwrap();
        store.delete("k").await.unwrap();
        assert_eq!(store.version("k").await.unwrap(), None);
        // 删除后可重新以版本 0 创建
        assert_eq!(
            store
                .compare_and_swap("k", 0, b"new".to_vec(), None)
                .await
                .unwrap(),
            Some(1)
        );
    }

    #[tokio::test]
    async fn missing_key_rejects_nonzero_expect() {
        let store = MemoryVersionedCache::new();
        assert_eq!(
            store
                .compare_and_swap("ghost", 3, b"x".to_vec(), None)
                .await
                .unwrap(),
            None
        );
        assert_eq!(store.entry_count(), 0);
    }

    /// Redis WATCH 路径（需真实 Redis；CI 跳过）
    #[cfg(feature = "redis")]
    #[tokio::test]
    #[ignore = "needs live Redis at 127.0.0.1:6379"]
    async fn redis_watch_cas_roundtrip() {
        use crate::backend::RedisBackend;
        let backend = Arc::new(RedisBackend::new("redis://127.0.0.1:6379").await.unwrap());
        let store = RedisVersionedCache::new(backend);
        let key = format!("oxcache:versioned:{}", uuid::Uuid::new_v4());

        let v1 = store
            .compare_and_swap(&key, 0, b"a".to_vec(), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v1, 1);
        let v2 = store
            .compare_and_swap(&key, v1, b"b".to_vec(), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v2, 2);
        assert_eq!(
            store
                .compare_and_swap(&key, v1, b"stale".to_vec(), None)
                .await
                .unwrap(),
            None
        );
        store.delete(&key).await.unwrap();
    }
}
