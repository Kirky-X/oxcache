// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! [`BloomFilterBackend`] — a `CacheBackend` decorator that wraps an inner
//! backend with a [`BloomFilter`] for negative query filtering.
//!
//! On `get`, the Bloom filter is consulted first: if it says the key is
//! absent, `Ok(None)` is returned without touching the inner backend. If the
//! filter says the key may be present, the inner backend's `get` is called.
//! When the inner backend returns `None` (e.g. TTL expiry) the Bloom filter is
//! left untouched — Bloom filters do not support deletion.
//!
//! `set` inserts the key into the Bloom filter before delegating to the inner
//! backend. `delete` only delegates to the inner backend (the Bloom filter is
//! not modified). `clear` clears both. All TTL operations (`set` ttl, `ttl`,
//! `expire`) pass through to the inner backend unchanged.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::backend::{
    BackendKind, BackendScore, CacheBackend, CacheConnector, CacheReader, CacheWriter,
    SyncCacheBackend, SyncCacheConnector, SyncCacheReader, SyncCacheWriter,
};
use crate::error::{OxCacheError, OxCacheResult};

use super::BloomFilter;

/// `CacheBackend` decorator wrapping an inner backend `B` with a Bloom filter
/// for negative query filtering.
///
/// The Bloom filter is shared via `Arc<RwLock<>>` inside [`BloomFilter`], so
/// cloning the backend (or the filter) shares state.
pub struct BloomFilterBackend<B: CacheBackend> {
    inner: B,
    bloom: BloomFilter,
}

impl<B: CacheBackend> BloomFilterBackend<B> {
    /// Create a decorator over `inner` with the default Bloom filter
    /// configuration (capacity `100_000`, false positive rate `0.01`).
    pub fn new(inner: B) -> Self {
        Self {
            inner,
            bloom: BloomFilter::new(100_000, 0.01),
        }
    }

    /// Create a decorator over `inner` with an explicit Bloom filter
    /// configuration.
    pub fn with_capacity_and_rate(inner: B, capacity: usize, false_positive_rate: f64) -> Self {
        Self {
            inner,
            bloom: BloomFilter::new(capacity, false_positive_rate),
        }
    }

    /// Start a builder for configurable construction.
    pub fn builder() -> BloomFilterBackendBuilder<B> {
        BloomFilterBackendBuilder {
            capacity: 100_000,
            false_positive_rate: 0.01,
            inner: None,
        }
    }

    /// Borrow the inner backend.
    pub fn inner(&self) -> &B {
        &self.inner
    }

    /// Borrow the Bloom filter.
    pub fn bloom(&self) -> &BloomFilter {
        &self.bloom
    }

    /// 直接灌入已知存在的 key（进程重启预热 / 首次部署对齐）。
    ///
    /// 过滤器状态为**进程内存**：重启清零、多实例各自独立，对共享持久后端
    /// （如 Redis）中已存在但不在本过滤器插入集合内的 key 会产生假阴性——
    /// `get` 判"不存在"直接短路返回 `None`，连后端都不查询。部署/重启后
    /// 必须预热对齐，或使用 [`Self::prefill_from_backend`]。
    pub fn prefill_from_keys<I, K>(&self, keys: I)
    where
        I: IntoIterator<Item = K>,
        K: AsRef<str>,
    {
        for key in keys {
            self.bloom.insert(key.as_ref());
        }
    }

    /// 以 `inner.keys(pattern)` 回灌过滤器，返回回灌的 key 数。
    ///
    /// 适用于重启后过滤器为空而后端仍有数据的场景：回灌后 get 才能查到
    /// 后端已有条目。注意多实例部署下各进程仍需各自回灌（过滤器不共享）。
    pub async fn prefill_from_backend(&self, pattern: &str) -> OxCacheResult<usize> {
        let keys = self.inner.keys(pattern).await?;
        let count = keys.len();
        for key in &keys {
            self.bloom.insert(key);
        }
        Ok(count)
    }
}

/// Builder for [`BloomFilterBackend`].
pub struct BloomFilterBackendBuilder<B: CacheBackend> {
    capacity: usize,
    false_positive_rate: f64,
    inner: Option<B>,
}

impl<B: CacheBackend> BloomFilterBackendBuilder<B> {
    /// Set the Bloom filter capacity (estimated max items).
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Set the target false positive rate (must be in `(0.0, 1.0)`).
    pub fn false_positive_rate(mut self, rate: f64) -> Self {
        self.false_positive_rate = rate;
        self
    }

    /// Set the inner backend to wrap.
    pub fn inner(mut self, inner: B) -> Self {
        self.inner = Some(inner);
        self
    }

    /// Build the decorator. Returns `Err` if no inner backend was set.
    pub fn build(self) -> OxCacheResult<BloomFilterBackend<B>> {
        let inner = self.inner.ok_or_else(|| {
            OxCacheError::InvalidInput(
                "inner backend is required for BloomFilterBackend".to_string(),
            )
        })?;
        Ok(BloomFilterBackend {
            inner,
            bloom: BloomFilter::new(self.capacity, self.false_positive_rate),
        })
    }
}

#[async_trait]
impl<B: CacheBackend> CacheReader for BloomFilterBackend<B> {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        // BF first: if the filter says the key is absent, skip the inner
        // backend entirely. （"无假阴性"仅对本过滤器的 insert 集合成立：
        // 进程内存态，重启清零 / 多实例独立 / 对共享持久后端存在假阴性，
        // 部署后需经 prefill_from_keys / prefill_from_backend 预热对齐。）
        if !self.bloom.contains(key) {
            return Ok(None);
        }
        // BF says maybe present — delegate to inner. If inner returns None
        // (e.g. TTL expiry) the BF is left untouched: Bloom filters do not
        // support deletion, and the spec forbids mutating BF on a miss.
        self.inner.get(key).await
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        // BF miss（对本进程 insert 集合）→ 视为不存在跳过后端；
        // 进程边界假阴性风险见 prefill_from_keys 文档。
        if !self.bloom.contains(key) {
            return Ok(false);
        }
        self.inner.exists(key).await
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        // BF miss → key cannot exist → TTL is definitively None.
        if !self.bloom.contains(key) {
            return Ok(None);
        }
        self.inner.ttl(key).await
    }

    async fn len(&self) -> OxCacheResult<u64> {
        self.inner.len().await
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        self.inner.capacity().await
    }

    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        let mut stats = self.inner.stats().await?;
        stats.insert(
            "bloom_capacity".to_string(),
            self.bloom.capacity().to_string(),
        );
        stats.insert(
            "bloom_load_factor".to_string(),
            self.bloom.load_factor().to_string(),
        );
        stats.insert(
            "bloom_false_positive_rate".to_string(),
            self.bloom.false_positive_rate().to_string(),
        );
        stats.insert(
            "bloom_estimated_count".to_string(),
            self.bloom.len().to_string(),
        );
        Ok(stats)
    }
}

#[async_trait]
impl<B: CacheBackend> CacheWriter for BloomFilterBackend<B> {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        // Delegate to inner first; only update BF on success to avoid
        // permanent false positives if the inner set fails.
        self.inner.set(key.clone(), value, ttl).await?;
        self.bloom.insert(&key);
        Ok(())
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        // Only delegate to inner; BF does not support removal.
        self.inner.delete(key).await
    }

    async fn clear(&self) -> OxCacheResult<()> {
        self.inner.clear().await?;
        self.bloom.clear();
        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        self.inner.expire(key, ttl).await
    }
}

#[async_trait]
impl<B: CacheBackend> CacheConnector for BloomFilterBackend<B> {
    async fn health_check(&self) -> OxCacheResult<()> {
        self.inner.health_check().await
    }

    async fn shutdown(&self) {
        self.inner.shutdown().await
    }

    fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }
}

impl<B: CacheBackend + BackendScore> BackendScore for BloomFilterBackend<B> {
    fn score(&self) -> u8 {
        self.inner.score()
    }

    fn is_persistent(&self) -> bool {
        self.inner.is_persistent()
    }

    fn backend_name(&self) -> &'static str {
        self.inner.backend_name()
    }
}

// ============================================================================
// Synchronous trait hierarchy (任务组 14)
// ============================================================================
//
// Mirror of the async `CacheBackend` impl. Only available when the inner
// backend `B` also supports sync access (`B: SyncCacheBackend`). UFCS is used
// throughout the bodies to disambiguate from the async trait methods that `B`
// also implements (both hierarchies define `get`/`set`/etc.).

impl<B: CacheBackend + SyncCacheBackend> SyncCacheReader for BloomFilterBackend<B> {
    fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        // BF first: if the filter says the key is absent, skip the inner
        // backend entirely (no false negatives). Mirrors the async impl.
        if !self.bloom.contains(key) {
            return Ok(None);
        }
        // BF says maybe present — delegate to inner's sync get. If inner
        // returns None (e.g. TTL expiry) the BF is left untouched.
        SyncCacheReader::get(&self.inner, key)
    }

    fn exists(&self, key: &str) -> OxCacheResult<bool> {
        // BF has no false negatives: a miss means the key definitely does not
        // exist, so we can skip the inner backend entirely.
        if !self.bloom.contains(key) {
            return Ok(false);
        }
        SyncCacheReader::exists(&self.inner, key)
    }

    fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        // BF miss → key cannot exist → TTL is definitively None.
        if !self.bloom.contains(key) {
            return Ok(None);
        }
        SyncCacheReader::ttl(&self.inner, key)
    }

    fn len(&self) -> OxCacheResult<u64> {
        SyncCacheReader::len(&self.inner)
    }

    fn capacity(&self) -> OxCacheResult<u64> {
        SyncCacheReader::capacity(&self.inner)
    }

    fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        let mut stats = SyncCacheReader::stats(&self.inner)?;
        stats.insert(
            "bloom_capacity".to_string(),
            self.bloom.capacity().to_string(),
        );
        stats.insert(
            "bloom_load_factor".to_string(),
            self.bloom.load_factor().to_string(),
        );
        stats.insert(
            "bloom_false_positive_rate".to_string(),
            self.bloom.false_positive_rate().to_string(),
        );
        stats.insert(
            "bloom_estimated_count".to_string(),
            self.bloom.len().to_string(),
        );
        Ok(stats)
    }
}

impl<B: CacheBackend + SyncCacheBackend> SyncCacheWriter for BloomFilterBackend<B> {
    fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()> {
        // Delegate to inner first; only update BF on success to avoid
        // permanent false positives if the inner set fails.
        SyncCacheWriter::set(&self.inner, key.clone(), value, ttl)?;
        self.bloom.insert(&key);
        Ok(())
    }

    fn delete(&self, key: &str) -> OxCacheResult<()> {
        // Only delegate to inner; BF does not support removal.
        SyncCacheWriter::delete(&self.inner, key)
    }

    fn clear(&self) -> OxCacheResult<()> {
        SyncCacheWriter::clear(&self.inner)?;
        self.bloom.clear();
        Ok(())
    }

    fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        SyncCacheWriter::expire(&self.inner, key, ttl)
    }
}

impl<B: CacheBackend + SyncCacheBackend> SyncCacheConnector for BloomFilterBackend<B> {
    fn health_check(&self) -> OxCacheResult<()> {
        SyncCacheConnector::health_check(&self.inner)
    }

    fn shutdown(&self) {
        SyncCacheConnector::shutdown(&self.inner)
    }

    fn backend_kind(&self) -> BackendKind {
        SyncCacheConnector::backend_kind(&self.inner)
    }
}

#[cfg(test)]
mod tests;
