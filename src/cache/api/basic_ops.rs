// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Cache 基础操作方法

use super::Cache;
use crate::core::constants::NULL_SENTINEL;
use crate::error::{OxCacheError, OxCacheResult};
use crate::macro_support::{AsyncSfGuard, shard_index as global_shard_index};
use crate::traits::CacheKey;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tokio::sync::watch;

/// 分片数量（2 的幂，通过掩码路由）
const GET_OR_LOCK_SHARDS: usize = crate::macro_support::SF_SHARDS;

/// 单个 get_or 分片的存储类型：key → 该 key 的 flight 完成信号
type GetOrShard = Mutex<HashMap<String, std::sync::Arc<watch::Sender<()>>>>;

/// 全局 get_or 去重锁，防止缓存击穿（thundering herd）。
/// 当多个并发请求同时调用 `get_or` 且缓存未命中时，
/// 只让第一个请求执行 fallback，其余请求等待结果。
///
/// 使用 64 路分片（按 key hash 路由），避免所有 key 竞争同一把 Mutex，
/// 消除全局锁热点（问题 3.1）。
///
/// # 进程边界
///
/// 本注册表为**进程级**：多实例部署时各进程各自去重，跨实例合并需配合
/// `crate::features::dist_lock` 在 fallback 外层加分布式锁，例如：
///
/// ```text
/// let _lock = dist_lock.acquire(key).await?;   // 跨实例 leader 选举
/// cache.get_or(&key, || load_from_db(key)).await?  // 进程内合并
/// ```
static GET_OR_LOCKS: Lazy<[GetOrShard; GET_OR_LOCK_SHARDS]> =
    Lazy::new(|| std::array::from_fn(|_| Mutex::new(HashMap::new())));

/// 计算 key 对应的分片索引（复用 macro_support 单一实现）
fn get_or_shard_index(key: &str) -> usize {
    global_shard_index(key)
}

// SWR telemetry 双版本 inline 埋点（stale feature）
#[cfg(all(feature = "stale", feature = "telemetry"))]
#[inline]
fn telemetry_stale_downgrade(key: &str) {
    tracing::debug!(target: "oxcache::stale", key, "stale hit served via get_or (Return downgrade; use get_or_refresh for background revalidation)");
}

#[cfg(not(all(feature = "stale", feature = "telemetry")))]
#[inline]
fn telemetry_stale_downgrade(_key: &str) {}

#[cfg(all(feature = "stale", feature = "telemetry"))]
#[inline]
fn telemetry_stale_refresh(key: &str, spawned: bool) {
    tracing::debug!(target: "oxcache::stale", key, spawned, "stale hit; background revalidation scheduled");
}

#[cfg(not(all(feature = "stale", feature = "telemetry")))]
#[inline]
fn telemetry_stale_refresh(_key: &str, _spawned: bool) {}

// 生产路径的序列化/反序列化统一走 `UnifiedSerializer`（格式可插拔），
// 原 `deserialize_value` 辅助函数已被其取代。

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    pub async fn get(&self, key: &K) -> OxCacheResult<Option<V>> {
        // 纯 L1 路径指标埋点（默认 NoOp 零开销）
        #[cfg(feature = "metrics")]
        let __start = std::time::Instant::now();
        let key_str = key.to_key_string();
        let bytes = self.backend.get(&key_str).await?;
        #[cfg(feature = "metrics")]
        {
            let latency = __start.elapsed();
            let layer = self.metrics_layer();
            if bytes.is_some() {
                self.metrics.record_hit(layer, latency);
            } else {
                self.metrics.record_miss(layer, latency);
            }
            self.record_backend_op();
        }
        // 审计事件（hit/miss）
        #[cfg(feature = "audit")]
        if let Some(publisher) = self.audit.as_ref() {
            let action = if bytes.is_some() {
                crate::features::audit::AuditAction::Hit
            } else {
                crate::features::audit::AuditAction::Miss
            };
            publisher.publish(
                crate::features::audit::AuditEvent::new(action)
                    .with_key(crate::features::audit::redact_key_for_audit(&key_str)),
            );
        }
        match bytes {
            Some(data) if data.as_slice() == NULL_SENTINEL => Ok(None),
            // 经 UnifiedSerializer 反序列化（JSON 默认；可切二进制格式）
            Some(data) => self.unified_serializer.deserialize(&data).map(Some),
            None => Ok(None),
        }
    }

    // ========================================================================
    // 热路径借用查询（零分配）
    // ========================================================================

    /// 借用键查询：跳过 `K::to_key_string()` 的 String 分配，直接以 `&str`
    /// 查询后端（get 热路径零堆分配）。
    ///
    /// # 语义注意
    ///
    /// `key` 原样进入后端（不做任何键变换）：当 `K` 的 `to_key_string()`
    /// 恰为原值（如 `K = String`）时与 [`Self::get`](Self::get) 等价；
    /// 存在键前缀策略时调用方需自带完整键。
    pub async fn get_by_str(&self, key: &str) -> OxCacheResult<Option<V>> {
        #[cfg(feature = "metrics")]
        let __start = std::time::Instant::now();
        let bytes = self.backend.get(key).await?;
        #[cfg(feature = "metrics")]
        {
            let latency = __start.elapsed();
            let layer = self.metrics_layer();
            if bytes.is_some() {
                self.metrics.record_hit(layer, latency);
            } else {
                self.metrics.record_miss(layer, latency);
            }
            self.record_backend_op();
        }
        #[cfg(feature = "audit")]
        if let Some(publisher) = self.audit.as_ref() {
            let action = if bytes.is_some() {
                crate::features::audit::AuditAction::Hit
            } else {
                crate::features::audit::AuditAction::Miss
            };
            publisher.publish(
                crate::features::audit::AuditEvent::new(action)
                    .with_key(crate::features::audit::redact_key_for_audit(key)),
            );
        }
        match bytes {
            Some(data) if data.as_slice() == NULL_SENTINEL => Ok(None),
            Some(data) => self.unified_serializer.deserialize(&data).map(Some),
            None => Ok(None),
        }
    }

    /// 借用键写入：键路径仅产生一次 `Arc<str>` 分配
    ///（`set` 路径为 `String` + `Arc<str>` 两次）。
    pub async fn set_by_str(
        &self,
        key: &str,
        value: &V,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let bytes = self.unified_serializer.serialize(value)?;
        #[cfg(feature = "metrics")]
        let __start = std::time::Instant::now();
        let result = self.backend.set(Arc::from(key), Arc::new(bytes), ttl).await;
        #[cfg(feature = "metrics")]
        {
            self.metrics
                .record_set(self.metrics_layer(), __start.elapsed());
            self.record_backend_op();
        }
        result
    }

    // ========================================================================
    // Lifecycle and stats methods (delegating to backend)
    // ========================================================================

    /// Clear all entries in the cache.
    pub async fn clear(&self) -> OxCacheResult<()> {
        self.backend.clear().await
    }

    /// List keys matching a glob pattern.
    /// Delegates to the backend's `CacheReader::keys()` implementation.
    pub async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        self.backend.keys(pattern).await
    }

    /// Shutdown the cache and release resources.
    pub async fn shutdown(&self) {
        self.backend.shutdown().await
    }

    /// Health check for the cache backend.
    pub async fn health_check(&self) -> OxCacheResult<()> {
        self.backend.health_check().await
    }

    /// Get cache statistics.
    pub async fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
        self.backend.stats().await
    }

    /// Get the number of entries in the cache.
    pub async fn len(&self) -> OxCacheResult<u64> {
        self.backend.len().await
    }

    /// Check if the cache is empty.
    pub async fn is_empty(&self) -> OxCacheResult<bool> {
        self.backend.is_empty().await
    }

    /// Get the capacity of the cache.
    pub async fn capacity(&self) -> OxCacheResult<u64> {
        self.backend.capacity().await
    }

    pub async fn set(&self, key: &K, value: &V) -> OxCacheResult<()> {
        self.set_with_ttl(key, value, None).await
    }

    pub async fn set_with_ttl(
        &self,
        key: &K,
        value: &V,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let key_str = key.to_key_string();
        let ttl = ttl.map(|t| self.apply_jitter(t));
        // 脱敏键需在 key_str 被 move 前计算
        #[cfg(feature = "audit")]
        let __redacted_key = crate::features::audit::redact_key_for_audit(&key_str);

        // cache 模块门控 any(memory,redis,disk) 且三者均隐含 serialization，
        // 序列化双分支的负分支不可达，本文件无条件处于序列化面
        // 经 UnifiedSerializer 序列化（JSON 默认；可切二进制格式）
        let bytes = self.unified_serializer.serialize(value)?;
        // 写路径指标埋点
        #[cfg(feature = "metrics")]
        let __start = std::time::Instant::now();
        let result = self
            .backend
            .set(Arc::from(key_str), Arc::new(bytes), ttl)
            .await;
        #[cfg(feature = "metrics")]
        {
            self.metrics
                .record_set(self.metrics_layer(), __start.elapsed());
            self.record_backend_op();
        }
        // 审计事件（set）
        #[cfg(feature = "audit")]
        if result.is_ok()
            && let Some(publisher) = self.audit.as_ref()
        {
            publisher.publish(
                crate::features::audit::AuditEvent::new(crate::features::audit::AuditAction::Set)
                    .with_key(__redacted_key),
            );
        }
        result
    }

    pub async fn delete(&self, key: &K) -> OxCacheResult<()> {
        let key_str = key.to_key_string();
        // 删除路径指标埋点
        #[cfg(feature = "metrics")]
        let __start = std::time::Instant::now();
        let result = self.backend.delete(&key_str).await;
        #[cfg(feature = "metrics")]
        {
            self.metrics
                .record_delete(self.metrics_layer(), __start.elapsed());
            self.record_backend_op();
        }
        // 审计事件（delete）
        #[cfg(feature = "audit")]
        if result.is_ok()
            && let Some(publisher) = self.audit.as_ref()
        {
            publisher.publish(
                crate::features::audit::AuditEvent::new(
                    crate::features::audit::AuditAction::Delete,
                )
                .with_key(crate::features::audit::redact_key_for_audit(&key_str)),
            );
        }
        result
    }

    pub async fn exists(&self, key: &K) -> OxCacheResult<bool> {
        let key_str = key.to_key_string();
        self.backend.exists(&key_str).await
    }

    /// Get the remaining time-to-live for a key.
    ///
    /// Returns `Ok(None)` if the key has no per-entry TTL (either no TTL
    /// set, or the backend uses global TTL only). Returns `Ok(None)` if
    /// the key does not exist.
    ///
    /// This method is essential for update-with-preserving-TTL workflows:
    /// ```rust,ignore
    /// let original_ttl = cache.ttl(&key).await?;
    /// cache.set_with_ttl(&key, &new_value, original_ttl).await?;
    /// ```
    pub async fn ttl(&self, key: &K) -> OxCacheResult<Option<Duration>> {
        let key_str = key.to_key_string();
        self.backend.ttl(&key_str).await
    }

    /// Update the time-to-live for an existing key.
    ///
    /// Returns `Ok(true)` if the TTL was updated, `Ok(false)` if the key
    /// does not exist. This does NOT touch the value — only the TTL.
    pub async fn expire(&self, key: &K, ttl: Duration) -> OxCacheResult<bool> {
        let key_str = key.to_key_string();
        self.backend.expire(&key_str, ttl).await
    }

    pub async fn get_or<F, Fut>(&self, key: &K, fallback: F) -> OxCacheResult<V>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = OxCacheResult<V>>,
    {
        self.get_or_core(key, None, fallback).await
    }

    /// get-or-compute with a per-entry TTL for the cached value.
    ///
    /// 与 [`Self::get_or`] 语义一致，区别仅在于 fallback 成功后经
    /// [`Self::set_with_ttl`] 写入（TTL 经 `apply_jitter` 抖动）。避免
    /// `get_or` 主值永不过期（审计 F09：Redis 后端将产生永久键）。
    pub async fn get_or_with_ttl<F, Fut>(
        &self,
        key: &K,
        ttl: Option<Duration>,
        fallback: F,
    ) -> OxCacheResult<V>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = OxCacheResult<V>>,
    {
        self.get_or_core(key, ttl, fallback).await
    }

    /// SWR stale 探测（get_or 路径，无 offload 能力）：
    /// Stale 命中按策略处理——Return（及 OffloadRevalidate 的降级）返回旧值；
    /// Revalidate 返回 None 落入 miss 路径同步回源；Fresh/Opaque/Expired
    /// 交由标准路径。
    #[cfg(feature = "stale")]
    async fn stale_step_basic(&self, key: &K) -> OxCacheResult<Option<V>> {
        let Some(stale) = self.stale_backend.as_ref() else {
            return Ok(None);
        };
        let key_str = key.to_key_string();
        let (bytes, state) = stale.get_with_state(&key_str).await?;
        if state != crate::features::stale::StaleState::Stale {
            return Ok(None);
        }
        // 哨兵载荷按 miss 处理：fallback 重执行自然刷新哨兵
        if bytes.as_deref() == Some(crate::core::constants::NULL_SENTINEL) {
            return Ok(None);
        }
        let Some(raw) = bytes else {
            return Ok(None);
        };
        match self.stale_policy {
            crate::features::stale::StalePolicy::Return
            | crate::features::stale::StalePolicy::OffloadRevalidate => {
                // OffloadRevalidate 经 get_or（非 offloadable）按 Return 降级
                telemetry_stale_downgrade(&key_str);
                let old = self.unified_serializer.deserialize(&raw)?;
                Ok(Some(old))
            }
            crate::features::stale::StalePolicy::Revalidate => {
                // 视同 miss（hitbox Revalidate 语义）：先删除 stale 条目，
                // 使标准 miss/single-flight 路径的 hit 检查不再把 stale 视为
                // 命中；leader 回源写新值，follower 重查得 fresh 值。代价：
                // fallback 失败时旧值不再保留（同步刷新语义的固有取舍）。
                let _ = crate::backend::CacheWriter::delete(stale.as_ref(), &key_str).await;
                Ok(None)
            }
        }
    }

    /// `get_or_refresh`：`get_or` 的后台刷新变体（`stale` feature）。
    ///
    /// `OffloadRevalidate` 策略下 stale 命中立即返回旧值，并将 fallback 交由
    /// [`OffloadManager`](crate::features::offload::OffloadManager) 后台执行
    /// （同 key 去重；这正是 fallback 需要 `Send + 'static` 约束的原因）。
    /// Return / Revalidate 语义与 [`Self::get_or_with_ttl`] 一致。
    #[cfg(feature = "stale")]
    pub async fn get_or_refresh<F, Fut>(
        &self,
        key: &K,
        ttl: Option<Duration>,
        fallback: F,
    ) -> OxCacheResult<V>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = OxCacheResult<V>> + Send + 'static,
        V: Send + 'static,
    {
        if let Some(stale) = self.stale_backend.as_ref() {
            let key_str = key.to_key_string();
            let (bytes, state) = stale.get_with_state(&key_str).await?;
            let stale_hit = state == crate::features::stale::StaleState::Stale
                && bytes.as_deref() != Some(crate::core::constants::NULL_SENTINEL);
            if stale_hit {
                let raw = bytes.expect("Stale state must carry payload");
                match self.stale_policy {
                    crate::features::stale::StalePolicy::Return => {
                        return self.unified_serializer.deserialize(&raw);
                    }
                    crate::features::stale::StalePolicy::OffloadRevalidate => {
                        let old = self.unified_serializer.deserialize(&raw)?;
                        if let Some(offload) = self.offload.as_ref() {
                            let backend = self.backend.clone();
                            let serializer = self.unified_serializer.clone();
                            let write_ttl = ttl.map(|t| self.apply_jitter(t));
                            let refresh_key: std::sync::Arc<str> = Arc::from(key_str.as_str());
                            let spawned = offload.spawn(refresh_key.clone(), async move {
                                if let Ok(value) = fallback().await
                                    && let Ok(bytes) = serializer.serialize(&value)
                                {
                                    let _ =
                                        backend.set(refresh_key, Arc::new(bytes), write_ttl).await;
                                }
                            });
                            telemetry_stale_refresh(&key_str, spawned);
                            return Ok(old);
                        }
                        // 无管理器（不应发生）→ Return 降级
                        return Ok(old);
                    }
                    crate::features::stale::StalePolicy::Revalidate => {
                        // 同 stale_step_basic：删除 stale 条目后走 miss 路径
                        let _ = crate::backend::CacheWriter::delete(stale.as_ref(), &key_str).await;
                    }
                }
            }
        }
        self.get_or_core(key, ttl, fallback).await
    }

    /// `get_or` / `get_or_with_ttl` 的共享实现：`value_ttl=None` 保持旧路径
    /// 行为（无 TTL），`Some` 经 `apply_jitter` 抖动后写入。
    async fn get_or_core<F, Fut>(
        &self,
        key: &K,
        value_ttl: Option<Duration>,
        fallback: F,
    ) -> OxCacheResult<V>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = OxCacheResult<V>>,
    {
        // SWR 三态探测（stale feature）：Stale 命中按策略处理，其余走标准路径
        #[cfg(feature = "stale")]
        if let Some(v) = self.stale_step_basic(key).await? {
            return Ok(v);
        }
        // 快速路径：缓存命中
        if let Some(value) = self.get(key).await? {
            return Ok(value);
        }

        let key_str = key.to_key_string();
        let shard_index = get_or_shard_index(&key_str);

        // 注册为 leader 或成为 follower。锁在 match 结束即释放，不跨 await。
        //
        // follower 侧 `subscribe()` 返回 owned Receiver，watch 的版本比对
        // 语义保证"leader 先完成、follower 后 changed()"仍立即返回——不存在
        // `Notify::notify_waiters` 在注册与首次 poll 之间丢失唤醒的窗口（审计 F03）。
        enum FlightReg {
            Leader(Arc<watch::Sender<()>>),
            Follower(watch::Receiver<()>),
        }
        let reg = {
            let shard = &GET_OR_LOCKS[shard_index];
            let mut map = shard
                .lock()
                .expect("GET_OR_LOCKS poisoned - concurrent operation panic detected");
            match map.entry(key_str.clone()) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    // 已有其他请求在执行 fallback，订阅其完成信号
                    FlightReg::Follower(entry.get().subscribe())
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let (tx, _rx) = watch::channel(());
                    let tx = Arc::new(tx);
                    entry.insert(tx.clone());
                    FlightReg::Leader(tx)
                }
            }
        };

        match reg {
            FlightReg::Follower(mut rx) => {
                // 等待 leader 完成（信号发送或通道关闭均放行），随后读缓存
                let _ = rx.changed().await;
                // leader 应将结果写入缓存
                self.get(key).await?.ok_or_else(|| {
                    OxCacheError::L1Error(
                        "get_or: concurrent fetch leader failed to cache result".to_string(),
                    )
                })
            }
            FlightReg::Leader(signal) => {
                // panic 安全守卫：panic / 早退时 Drop 兜底移除条目并放行等待者
                let mut guard =
                    AsyncSfGuard::new(&GET_OR_LOCKS, shard_index, key_str.clone(), signal);

                // leader：二次检查缓存（避免与另一个刚刚完成的 leader 竞争）
                if let Some(value) = self.get(key).await? {
                    guard.finish();
                    return Ok(value);
                }

                let result = fallback().await;
                match result {
                    Ok(value) => {
                        match value_ttl {
                            Some(ttl) => self.set_with_ttl(key, &value, Some(ttl)).await?,
                            None => self.set(key, &value).await?,
                        }
                        guard.finish();
                        Ok(value)
                    }
                    Err(e) => {
                        guard.finish();
                        Err(e)
                    }
                }
            }
        }
    }

    /// Apply TTL jitter based on the configured jitter factor.
    ///
    /// When `ttl_jitter_factor` is 0.0, returns the original TTL unchanged.
    /// Otherwise, returns `base_ttl * (1.0 + uniform(-factor, factor))`.
    ///
    /// 随机源为静态原子状态 + `Instant` 单调纳秒混合的 xorshift（审计 F07）：
    /// 弃用 SystemTime——NTP 回拨时其 seed 恒为 0，所有 key 同时取得最小 TTL，
    /// 反而制造相关性雪崩。单调时钟不可回拨，且无锁开销。
    pub(super) fn apply_jitter(&self, ttl: Duration) -> Duration {
        if self.ttl_jitter_factor <= 0.0 {
            return ttl;
        }
        static JITTER_STATE: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);
        let mut s = JITTER_STATE
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(std::time::Instant::now().elapsed().subsec_nanos() as u64 | 1);
        // xorshift64 混合（Marsaglia）；仅用于抖动分布，无需密码学强度
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        let uniform = (s % 20_001) as f64 / 10_000.0 - 1.0;
        let jittered = ttl.as_millis() as f64 * (1.0 + self.ttl_jitter_factor * uniform);
        Duration::from_millis(jittered.max(1.0) as u64)
    }

    /// Get-or-compute with optional result and null caching for penetration guard.
    ///
    /// When the fallback returns `Ok(None)` and `null_cache_ttl` is configured,
    /// a null sentinel is written to the cache to prevent repeated lookups
    /// (cache penetration). The sentinel expires after the configured TTL.
    ///
    /// Existing `get_or` remains unchanged for backward compatibility.
    pub async fn get_or_option<F, Fut>(&self, key: &K, fallback: F) -> OxCacheResult<Option<V>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = OxCacheResult<Option<V>>>,
    {
        self.get_or_option_core(key, None, false, fallback).await
    }

    /// get-or-compute with per-entry TTL for the cached value AND jittered null
    /// sentinel TTL.
    ///
    /// 与 [`Self::get_or_option`] 语义一致，区别在于：真实值经
    /// [`Self::set_with_ttl`] 写入（抖动），空值哨兵 TTL 同样过 `apply_jitter`
    /// ——攻击突发产生的同批哨兵不会同时过期（审计 F09）。
    pub async fn get_or_option_with_ttl<F, Fut>(
        &self,
        key: &K,
        ttl: Option<Duration>,
        fallback: F,
    ) -> OxCacheResult<Option<V>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = OxCacheResult<Option<V>>>,
    {
        self.get_or_option_core(key, ttl, true, fallback).await
    }

    /// `get_or_option` / `get_or_option_with_ttl` 的共享实现。
    async fn get_or_option_core<F, Fut>(
        &self,
        key: &K,
        value_ttl: Option<Duration>,
        jitter_sentinel: bool,
        fallback: F,
    ) -> OxCacheResult<Option<V>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = OxCacheResult<Option<V>>>,
    {
        // Fast path: cache hit (returns None for null sentinel too)
        if let Some(value) = self.get(key).await? {
            return Ok(Some(value));
        }

        // Check if this is a null sentinel hit (key exists but value is sentinel).
        // 哨兵判定用 get + 字节比对而非 exists（审计 F02）：exists 无法区分
        // 哨兵与真实值——get miss 后并发写入真实值会被 exists 误判为"哨兵有效"
        // 而返回 Ok(None)；get + 比对在竞态命中时直接返回真实值。
        let key_str = key.to_key_string();
        if self.null_cache_ttl.is_some()
            && let Some(bytes) = self.backend.get(&key_str).await?
        {
            if bytes.as_slice() == NULL_SENTINEL {
                return Ok(None);
            }
            return self.unified_serializer.deserialize(&bytes).map(Some);
        }

        let shard_index = get_or_shard_index(&key_str);

        // Single-flight: register as leader or become follower.
        // watch 订阅语义保证 follower 不丢失唤醒（审计 F03），锁不跨 await。
        enum FlightReg {
            Leader(Arc<watch::Sender<()>>),
            Follower(watch::Receiver<()>),
        }
        let reg = {
            let shard = &GET_OR_LOCKS[shard_index];
            let mut map = shard
                .lock()
                .expect("GET_OR_LOCKS poisoned - concurrent operation panic detected");
            match map.entry(key_str.clone()) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    FlightReg::Follower(entry.get().subscribe())
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let (tx, _rx) = watch::channel(());
                    let tx = Arc::new(tx);
                    entry.insert(tx.clone());
                    FlightReg::Leader(tx)
                }
            }
        };

        match reg {
            FlightReg::Follower(mut rx) => {
                // Re-check cache after leader completes
                let _ = rx.changed().await;
                if let Some(value) = self.get(key).await? {
                    return Ok(Some(value));
                }
                // Leader cached a null sentinel or fallback failed（get + 字节比对，审计 F02）
                if self.null_cache_ttl.is_some()
                    && let Some(bytes) = self.backend.get(&key_str).await?
                    && bytes.as_slice() == NULL_SENTINEL
                {
                    return Ok(None);
                }
                Err(OxCacheError::L1Error(
                    "get_or_option: concurrent fetch leader failed to cache result".to_string(),
                ))
            }
            FlightReg::Leader(signal) => {
                // panic 安全守卫：panic / 早退时 Drop 兜底移除条目并放行等待者
                let mut guard =
                    AsyncSfGuard::new(&GET_OR_LOCKS, shard_index, key_str.clone(), signal);

                // Double-check
                if let Some(value) = self.get(key).await? {
                    guard.finish();
                    return Ok(Some(value));
                }

                let result = fallback().await;
                match result {
                    Ok(Some(value)) => {
                        match value_ttl {
                            Some(ttl) => self.set_with_ttl(key, &value, Some(ttl)).await?,
                            None => self.set(key, &value).await?,
                        }
                        guard.finish();
                        Ok(Some(value))
                    }
                    Ok(None) => {
                        // Cache null sentinel if null_cache_ttl is configured
                        //（jitter_sentinel 时哨兵 TTL 同样过抖动，防同批同时过期）
                        if let Some(null_ttl) = self.null_cache_ttl {
                            let effective = if jitter_sentinel {
                                self.apply_jitter(null_ttl)
                            } else {
                                null_ttl
                            };
                            self.backend
                                .set(
                                    Arc::from(key_str.as_str()),
                                    Arc::new(NULL_SENTINEL.to_vec()),
                                    Some(effective),
                                )
                                .await?;
                        }
                        guard.finish();
                        Ok(None)
                    }
                    Err(e) => {
                        guard.finish();
                        Err(e)
                    }
                }
            }
        }
    }
}

// ============================================================================
// Synchronous API — mirrors the async API but dispatches through
// `backend_sync: Option<Arc<dyn SyncCacheBackend>>`.
//
// Returns `Err(OxCacheError::NotSupported)` when the cache was not built with
// `sync_mode` enabled (i.e., `backend_sync` is `None`).
//
// Single-flight for `get_or_sync` uses `std::sync::Condvar` (no async runtime
// required), mirroring the async `get_or` which uses `tokio::sync::Notify`.
// ============================================================================

/// Single-flight state for `get_or_sync`. The `Mutex<bool>` flag is `false`
/// while the leader is executing fallback, `true` once the leader has finished
/// (success or failure). Followers `wait()` on the `Condvar` until `true`.
type SyncFlight = Arc<(Mutex<bool>, Condvar)>;

/// 单个 get_or_sync 分片的存储类型：key → 该 key 的 leader flight
type GetOrSyncShard = Mutex<HashMap<String, SyncFlight>>;

/// Global registry of in-flight `get_or_sync` leaders, keyed by cache key.
/// Followers find their leader's `SyncFlight` here and block on its `Condvar`.
///
/// 使用 64 路分片（按 key hash 路由），避免全局锁热点（问题 3.1）。
static GET_OR_SYNC_LOCKS: Lazy<[GetOrSyncShard; GET_OR_LOCK_SHARDS]> =
    Lazy::new(|| std::array::from_fn(|_| Mutex::new(HashMap::new())));

/// Panic-safe guard for `get_or_sync` leaders. If the leader panics before
/// marking its flight `done`, this `Drop` impl flips the flag to `true` and
/// `notify_all`s followers so they don't block forever, then removes the
/// stale entry from the registry.
struct GetOrSyncGuard {
    shard_index: usize,
    map_key: String,
    flight: SyncFlight,
    removed: bool,
}

/// leader 执行上下文（打包 run_sync_fallback 的定位参数，避免超长参数列表）
struct SyncLeaderCtx<'a> {
    shard_index: usize,
    key_str: &'a str,
    flight: &'a SyncFlight,
}

impl Drop for GetOrSyncGuard {
    fn drop(&mut self) {
        if !self.removed {
            {
                let mut done = self.flight.0.lock().expect(
                    "GetOrSyncGuard: flight mutex poisoned - leader panicked during fallback",
                );
                *done = true;
            }
            self.flight.1.notify_all();
            GET_OR_SYNC_LOCKS[self.shard_index]
                .lock()
                .expect("GET_OR_SYNC_LOCKS poisoned - concurrent operation panic detected")
                .remove(&self.map_key);
        }
    }
}

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    /// Resolve the sync backend or return `Err(NotSupported)` when the cache
    /// was not built with `sync_mode(true)`.
    pub(super) fn sync_backend(&self) -> OxCacheResult<&Arc<dyn crate::backend::SyncCacheBackend>> {
        self.backend_sync.as_ref().ok_or_else(|| {
            OxCacheError::NotSupported(
                "sync API requires CacheBuilder::sync_mode(true); backend_sync is None".to_string(),
            )
        })
    }

    /// Synchronously get a value from the cache.
    pub fn get_sync(&self, key: &K) -> OxCacheResult<Option<V>> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        // Method-call syntax (not UFCS) — `dyn SyncCacheBackend` exposes
        // super-trait methods via its vtable; UFCS would require
        // `Arc<dyn SyncCacheBackend>: SyncCacheReader` which needs the
        // unstable `trait_upcasting` feature.
        let bytes = backend.get(&key_str)?;
        // 审计事件（hit/miss）——与 async get 同语义（sentinel 命中计为 Hit）
        #[cfg(feature = "audit")]
        if let Some(publisher) = self.audit.as_ref() {
            let action = if bytes.is_some() {
                crate::features::audit::AuditAction::Hit
            } else {
                crate::features::audit::AuditAction::Miss
            };
            publisher.publish(
                crate::features::audit::AuditEvent::new(action)
                    .with_key(crate::features::audit::redact_key_for_audit(&key_str)),
            );
        }
        match bytes {
            Some(data) if data.as_slice() == NULL_SENTINEL => Ok(None),
            // 经 UnifiedSerializer 反序列化（格式可插拔）
            Some(data) => self.unified_serializer.deserialize(&data).map(Some),
            None => Ok(None),
        }
    }

    /// Synchronously set a value in the cache (no TTL).
    pub fn set_sync(&self, key: &K, value: &V) -> OxCacheResult<()> {
        self.set_with_ttl_sync(key, value, None)
    }

    /// Synchronously set a value with an optional per-entry TTL.
    pub fn set_with_ttl_sync(
        &self,
        key: &K,
        value: &V,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let key_str = key.to_key_string();
        let ttl = ttl.map(|t| self.apply_jitter(t));
        let backend = self.sync_backend()?;

        #[cfg(feature = "audit")]
        let __redacted_key = crate::features::audit::redact_key_for_audit(&key_str);

        // 经 UnifiedSerializer 序列化（格式可插拔）
        let bytes = self.unified_serializer.serialize(value)?;
        let result = backend.set(Arc::from(key_str), Arc::new(bytes), ttl);
        // 审计事件（set）——与 async set_with_ttl 同语义（仅成功发布）
        #[cfg(feature = "audit")]
        if result.is_ok()
            && let Some(publisher) = self.audit.as_ref()
        {
            publisher.publish(
                crate::features::audit::AuditEvent::new(crate::features::audit::AuditAction::Set)
                    .with_key(__redacted_key),
            );
        }
        result
    }

    /// Synchronously delete a key.
    pub fn delete_sync(&self, key: &K) -> OxCacheResult<()> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        let result = backend.delete(&key_str);
        // 审计事件（delete）——与 async delete 同语义（仅成功发布）
        #[cfg(feature = "audit")]
        if result.is_ok()
            && let Some(publisher) = self.audit.as_ref()
        {
            publisher.publish(
                crate::features::audit::AuditEvent::new(
                    crate::features::audit::AuditAction::Delete,
                )
                .with_key(crate::features::audit::redact_key_for_audit(&key_str)),
            );
        }
        result
    }

    /// Synchronously check if a key exists.
    pub fn exists_sync(&self, key: &K) -> OxCacheResult<bool> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        backend.exists(&key_str)
    }

    /// Synchronously get the remaining time-to-live for a key.
    ///
    /// Returns `Ok(None)` if the key has no per-entry TTL or does not exist.
    /// Mirrors the async [`Self::ttl`].
    pub fn ttl_sync(&self, key: &K) -> OxCacheResult<Option<Duration>> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        backend.ttl(&key_str)
    }

    /// Synchronously update the time-to-live for an existing key.
    ///
    /// Returns `Ok(true)` if the TTL was updated, `Ok(false)` if the key
    /// does not exist. Mirrors the async [`Self::expire`].
    pub fn expire_sync(&self, key: &K, ttl: Duration) -> OxCacheResult<bool> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        backend.expire(&key_str, ttl)
    }

    /// Synchronously get-or-compute: returns cached value if present, otherwise
    /// invokes `fallback` and caches the result. Uses `Condvar`-based
    /// single-flight to prevent thundering-herd duplicate fallback calls.
    pub fn get_or_sync<F>(&self, key: &K, fallback: F) -> OxCacheResult<V>
    where
        F: FnOnce() -> OxCacheResult<V>,
    {
        self.get_or_sync_core(key, None, fallback)
    }

    /// Synchronously get-or-compute with a per-entry TTL for the cached value.
    ///
    /// 与 [`Self::get_or_sync`] 语义一致，缓存写入经 `apply_jitter` 抖动
    /// （审计 F09：避免主值永不过期）。
    pub fn get_or_with_ttl_sync<F>(
        &self,
        key: &K,
        ttl: Option<Duration>,
        fallback: F,
    ) -> OxCacheResult<V>
    where
        F: FnOnce() -> OxCacheResult<V>,
    {
        self.get_or_sync_core(key, ttl, fallback)
    }

    /// `get_or_sync` / `get_or_with_ttl_sync` 的共享实现。
    fn get_or_sync_core<F>(
        &self,
        key: &K,
        value_ttl: Option<Duration>,
        fallback: F,
    ) -> OxCacheResult<V>
    where
        F: FnOnce() -> OxCacheResult<V>,
    {
        // Fast path: cache hit
        if let Some(value) = self.get_sync(key)? {
            return Ok(value);
        }

        let key_str = key.to_key_string();
        let shard_index = get_or_shard_index(&key_str);

        // Register as leader or become follower. Lock is released before any
        // blocking work to avoid holding it while running fallback.
        let (is_follower, flight) = {
            let shard = &GET_OR_SYNC_LOCKS[shard_index];
            let mut map = shard
                .lock()
                .expect("GET_OR_SYNC_LOCKS poisoned - concurrent operation panic detected");
            match map.entry(key_str.clone()) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    // Another leader is in flight — become follower
                    (true, entry.get().clone())
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let f = Arc::new((Mutex::new(false), Condvar::new()));
                    entry.insert(f.clone());
                    (false, f)
                }
            }
        };

        if is_follower {
            // Wait for leader to finish (flag flips to true)
            let mut done = flight
                .0
                .lock()
                .expect("GET_OR_SYNC_LOCKS: follower flight mutex poisoned");
            while !*done {
                done = flight
                    .1
                    .wait(done)
                    .expect("GET_OR_SYNC_LOCKS: follower Condvar wait poisoned");
            }
            // Leader has finished — re-check cache. If leader succeeded the
            // value is now cached; if leader failed, return an error.
            return self.get_sync(key)?.ok_or_else(|| {
                OxCacheError::L1Error(
                    "get_or_sync: concurrent fetch leader failed to cache result".to_string(),
                )
            });
        }

        // Leader path
        let mut guard = GetOrSyncGuard {
            shard_index,
            map_key: key_str.clone(),
            flight: flight.clone(),
            removed: false,
        };

        let leader = SyncLeaderCtx {
            shard_index,
            key_str: &key_str,
            flight: &flight,
        };
        self.run_sync_fallback(key, leader, value_ttl, fallback, &mut guard)
    }

    /// Execute the fallback as the single-flight leader and notify followers.
    ///
    /// Re-checks the cache after acquiring leadership (another leader may have
    /// just finished), runs the fallback, caches the result, and always wakes
    /// followers via `finish_sync_flight` before propagating success or error.
    fn run_sync_fallback<F>(
        &self,
        key: &K,
        leader: SyncLeaderCtx<'_>,
        value_ttl: Option<Duration>,
        fallback: F,
        guard: &mut GetOrSyncGuard,
    ) -> OxCacheResult<V>
    where
        F: FnOnce() -> OxCacheResult<V>,
    {
        // Double-check cache after acquiring leadership (another leader may
        // have just finished and cached the value)
        if let Some(value) = self.get_sync(key)? {
            Self::finish_sync_flight(leader.shard_index, leader.key_str, leader.flight, guard);
            return Ok(value);
        }

        // Run fallback
        match fallback() {
            Ok(value) => {
                let cache_result = match value_ttl {
                    Some(ttl) => self.set_with_ttl_sync(key, &value, Some(ttl)),
                    None => self.set_sync(key, &value),
                };
                if let Err(e) = cache_result {
                    // Caching failed — still wake followers before propagating
                    Self::finish_sync_flight(
                        leader.shard_index,
                        leader.key_str,
                        leader.flight,
                        guard,
                    );
                    return Err(e);
                }
                Self::finish_sync_flight(leader.shard_index, leader.key_str, leader.flight, guard);
                Ok(value)
            }
            Err(e) => {
                Self::finish_sync_flight(leader.shard_index, leader.key_str, leader.flight, guard);
                Err(e)
            }
        }
    }

    /// Mark the flight as done, notify all followers, and remove the entry
    /// from the registry. Idempotent via the `guard.removed` flag.
    fn finish_sync_flight(
        shard_index: usize,
        key_str: &str,
        flight: &SyncFlight,
        guard: &mut GetOrSyncGuard,
    ) {
        {
            let mut done = flight
                .0
                .lock()
                .expect("GET_OR_SYNC_LOCKS: leader flight mutex poisoned");
            *done = true;
        }
        flight.1.notify_all();
        GET_OR_SYNC_LOCKS[shard_index]
            .lock()
            .expect("GET_OR_SYNC_LOCKS poisoned - concurrent operation panic detected")
            .remove(key_str);
        guard.removed = true;
    }

    /// Synchronously get-or-compute with optional result and null caching.
    ///
    /// Sync variant of [`Self::get_or_option`]. Uses `Condvar`-based single-flight.
    pub fn get_or_option_sync<F>(&self, key: &K, fallback: F) -> OxCacheResult<Option<V>>
    where
        F: FnOnce() -> OxCacheResult<Option<V>>,
    {
        self.get_or_option_sync_core(key, None, false, fallback)
    }

    /// Synchronously get-or-compute with per-entry TTL AND jittered null
    /// sentinel TTL（sync 变体，语义同 [`Self::get_or_option_with_ttl`]）。
    pub fn get_or_option_with_ttl_sync<F>(
        &self,
        key: &K,
        ttl: Option<Duration>,
        fallback: F,
    ) -> OxCacheResult<Option<V>>
    where
        F: FnOnce() -> OxCacheResult<Option<V>>,
    {
        self.get_or_option_sync_core(key, ttl, true, fallback)
    }

    /// `get_or_option_sync` / `get_or_option_with_ttl_sync` 的共享实现。
    fn get_or_option_sync_core<F>(
        &self,
        key: &K,
        value_ttl: Option<Duration>,
        jitter_sentinel: bool,
        fallback: F,
    ) -> OxCacheResult<Option<V>>
    where
        F: FnOnce() -> OxCacheResult<Option<V>>,
    {
        // Fast path: cache hit
        if let Some(value) = self.get_sync(key)? {
            return Ok(Some(value));
        }

        let key_str = key.to_key_string();

        // Check null sentinel（get + 字节比对，审计 F02——exists 无法区分哨兵与真实值）
        if self.null_cache_ttl.is_some() {
            let backend = self.sync_backend()?;
            if let Some(bytes) = backend.get(&key_str)? {
                if bytes.as_slice() == NULL_SENTINEL {
                    return Ok(None);
                }
                return self.unified_serializer.deserialize(&bytes).map(Some);
            }
        }

        let shard_index = get_or_shard_index(&key_str);

        let (is_follower, flight) = {
            let shard = &GET_OR_SYNC_LOCKS[shard_index];
            let mut map = shard
                .lock()
                .expect("GET_OR_SYNC_LOCKS poisoned - concurrent operation panic detected");
            match map.entry(key_str.clone()) {
                std::collections::hash_map::Entry::Occupied(entry) => (true, entry.get().clone()),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let f = Arc::new((Mutex::new(false), Condvar::new()));
                    entry.insert(f.clone());
                    (false, f)
                }
            }
        };

        if is_follower {
            let mut done = flight
                .0
                .lock()
                .expect("GET_OR_SYNC_LOCKS: follower flight mutex poisoned");
            while !*done {
                done = flight
                    .1
                    .wait(done)
                    .expect("GET_OR_SYNC_LOCKS: follower Condvar wait poisoned");
            }
            if let Some(value) = self.get_sync(key)? {
                return Ok(Some(value));
            }
            // Leader cached a null sentinel or fallback failed（get + 字节比对，审计 F02）
            if self.null_cache_ttl.is_some() {
                let backend = self.sync_backend()?;
                if let Some(bytes) = backend.get(&key_str)?
                    && bytes.as_slice() == NULL_SENTINEL
                {
                    return Ok(None);
                }
            }
            return Err(OxCacheError::L1Error(
                "get_or_option_sync: concurrent fetch leader failed to cache result".to_string(),
            ));
        }

        let mut guard = GetOrSyncGuard {
            shard_index,
            map_key: key_str.clone(),
            flight: flight.clone(),
            removed: false,
        };

        // Double-check
        if let Some(value) = self.get_sync(key)? {
            Self::finish_sync_flight(shard_index, &key_str, &flight, &mut guard);
            return Ok(Some(value));
        }

        match fallback() {
            Ok(Some(value)) => {
                let cache_result = match value_ttl {
                    Some(ttl) => self.set_with_ttl_sync(key, &value, Some(ttl)),
                    None => self.set_sync(key, &value),
                };
                if let Err(e) = cache_result {
                    // Caching failed — still wake followers before propagating
                    Self::finish_sync_flight(shard_index, &key_str, &flight, &mut guard);
                    return Err(e);
                }
                Self::finish_sync_flight(shard_index, &key_str, &flight, &mut guard);
                Ok(Some(value))
            }
            Ok(None) => {
                if let Some(null_ttl) = self.null_cache_ttl {
                    // jitter_sentinel 时哨兵 TTL 同样过抖动，防同批同时过期
                    let effective = if jitter_sentinel {
                        self.apply_jitter(null_ttl)
                    } else {
                        null_ttl
                    };
                    let backend = self.sync_backend()?;
                    if let Err(e) = backend.set(
                        Arc::from(key_str.as_str()),
                        Arc::new(NULL_SENTINEL.to_vec()),
                        Some(effective),
                    ) {
                        Self::finish_sync_flight(shard_index, &key_str, &flight, &mut guard);
                        return Err(e);
                    }
                }
                Self::finish_sync_flight(shard_index, &key_str, &flight, &mut guard);
                Ok(None)
            }
            Err(e) => {
                Self::finish_sync_flight(shard_index, &key_str, &flight, &mut guard);
                Err(e)
            }
        }
    }

    /// Synchronously clear all entries.
    pub fn clear_sync(&self) -> OxCacheResult<()> {
        let backend = self.sync_backend()?;
        backend.clear()
    }

    /// Synchronously run a health check against the backend.
    pub fn health_check_sync(&self) -> OxCacheResult<()> {
        let backend = self.sync_backend()?;
        backend.health_check()
    }

    /// Synchronously shut down the backend and release resources.
    /// No-op when `backend_sync` is `None` (no sync backend to shut down).
    pub fn shutdown_sync(&self) {
        if let Some(backend) = &self.backend_sync {
            backend.shutdown();
        }
    }

    /// Synchronously get backend statistics.
    pub fn stats_sync(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
        let backend = self.sync_backend()?;
        backend.stats()
    }

    /// Synchronously get the number of entries.
    pub fn len_sync(&self) -> OxCacheResult<u64> {
        let backend = self.sync_backend()?;
        backend.len()
    }

    /// Synchronously get the capacity.
    pub fn capacity_sync(&self) -> OxCacheResult<u64> {
        let backend = self.sync_backend()?;
        backend.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::constants::MAX_JSON_DEPTH;

    #[tokio::test]
    async fn test_cache_clear() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set(&"key".to_string(), &"value".to_string())
            .await
            .unwrap();
        cache.clear().await.unwrap();
        assert!(cache.get(&"key".to_string()).await.unwrap().is_none());
    }

    #[test]
    fn test_get_or_shard_index_in_range() {
        // 任意 key 都映射到 [0, SHARDS) 内的分片
        for key in [
            "",
            "a",
            "key1",
            "user:123",
            "很长很长的中文key🎯",
            "x".repeat(1024).as_str(),
        ] {
            let idx = get_or_shard_index(key);
            assert!(
                idx < GET_OR_LOCK_SHARDS,
                "key={key} shard={idx} out of range"
            );
        }
    }

    #[test]
    fn test_get_or_shards_distribute() {
        // 不同 key 应分散到多个分片（而非全部挤在同一分片）
        let mut seen = std::collections::HashSet::new();
        for i in 0..256 {
            seen.insert(get_or_shard_index(&format!("key{i}")));
        }
        assert!(
            seen.len() > 1,
            "256 keys should spread across shards, only {} distinct shards",
            seen.len()
        );
    }

    #[test]
    fn test_get_or_shard_index_same_key_same_shard() {
        // 相同 key 稳定映射到同一分片（single-flight 正确性的前提）
        let a = get_or_shard_index("stable-key");
        let b = get_or_shard_index("stable-key");
        assert_eq!(a, b);
        // 不同 key 可能不同分片
        let c = get_or_shard_index("other-key");
        let d = get_or_shard_index("another-key");
        assert_ne!(c, d, "不同 key 应路由到不同分片（本例期望）");
    }

    #[tokio::test]
    async fn test_get_or_concurrent_different_keys_no_contention_error() {
        // 高并发不同 key 的 get_or：分片锁下不应出错、不应丢值
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let cache = Arc::new(cache);

        let mut handles = Vec::new();
        for i in 0..64u64 {
            let cache = cache.clone();
            handles.push(tokio::spawn(async move {
                let key = format!("concurrent-key-{i}");
                let value = cache
                    .get_or(&key, || async move { Ok(format!("value-{i}")) })
                    .await
                    .unwrap();
                assert_eq!(value, format!("value-{i}"));
                cache.get(&key).await.unwrap().unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_cache_len() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set(&"key1".to_string(), &"v1".to_string())
            .await
            .unwrap();
        // Moka's entry_count() is approximate; verify it returns a reasonable value
        let len = cache.len().await.unwrap();
        assert!(len <= 100, "len should be reasonable after single insert");
    }

    #[tokio::test]
    async fn test_cache_is_empty() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set(&"key".to_string(), &"value".to_string())
            .await
            .unwrap();
        // Moka's is_empty is based on approximate entry_count; just verify no error
        let _ = cache.is_empty().await.unwrap();
    }

    #[tokio::test]
    async fn test_cache_exists() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        assert!(!cache.exists(&"key".to_string()).await.unwrap());
        cache
            .set(&"key".to_string(), &"value".to_string())
            .await
            .unwrap();
        assert!(cache.exists(&"key".to_string()).await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_delete() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set(&"key".to_string(), &"value".to_string())
            .await
            .unwrap();
        cache.delete(&"key".to_string()).await.unwrap();
        assert!(cache.get(&"key".to_string()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cache_get_or() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let value = cache
            .get_or(&"key".to_string(), || async { Ok("computed".to_string()) })
            .await
            .unwrap();
        assert_eq!(value, "computed");
        let cached = cache.get(&"key".to_string()).await.unwrap().unwrap();
        assert_eq!(cached, "computed");
    }

    #[tokio::test]
    async fn test_cache_health_check() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        assert!(cache.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let stats = cache.stats().await.unwrap();
        assert!(stats.contains_key("type"));
    }

    // ========================================================================
    // get / set / delete scenarios
    // ========================================================================

    #[tokio::test]
    async fn test_cache_get_miss_returns_none() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let result = cache.get(&"missing".to_string()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_set_overwrite() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        cache
            .set(&"k".to_string(), &"v1".to_string())
            .await
            .unwrap();
        assert_eq!(
            cache.get(&"k".to_string()).await.unwrap().unwrap(),
            "v1".to_string()
        );

        // Overwrite with a new value
        cache
            .set(&"k".to_string(), &"v2".to_string())
            .await
            .unwrap();
        assert_eq!(
            cache.get(&"k".to_string()).await.unwrap().unwrap(),
            "v2".to_string()
        );
    }

    #[tokio::test]
    async fn test_cache_delete_missing_key_no_error() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        // Deleting a key that was never set should not error
        assert!(cache.delete(&"never".to_string()).await.is_ok());
    }

    #[tokio::test]
    async fn test_cache_exists_after_delete() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        cache.set(&"k".to_string(), &"v".to_string()).await.unwrap();
        assert!(cache.exists(&"k".to_string()).await.unwrap());

        cache.delete(&"k".to_string()).await.unwrap();
        assert!(!cache.exists(&"k".to_string()).await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_set_with_ttl() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        cache
            .set_with_ttl(
                &"k".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        assert_eq!(
            cache.get(&"k".to_string()).await.unwrap().unwrap(),
            "v".to_string()
        );
    }

    #[tokio::test]
    async fn test_cache_set_with_ttl_none() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();

        cache
            .set_with_ttl(&"k".to_string(), &42, None)
            .await
            .unwrap();
        assert_eq!(cache.get(&"k".to_string()).await.unwrap().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_cache_get_set_integer_type() {
        let cache: Cache<String, i64> = Cache::builder().build().await.unwrap();

        cache.set(&"count".to_string(), &12345).await.unwrap();
        assert_eq!(
            cache.get(&"count".to_string()).await.unwrap().unwrap(),
            12345
        );
    }

    #[tokio::test]
    async fn test_cache_get_set_struct_type() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct User {
            id: u64,
            name: String,
        }

        let cache: Cache<String, User> = Cache::builder().build().await.unwrap();
        let user = User {
            id: 1,
            name: "alice".to_string(),
        };

        cache.set(&"user:1".to_string(), &user).await.unwrap();
        let result = cache.get(&"user:1".to_string()).await.unwrap().unwrap();
        assert_eq!(result, user);
    }

    // ========================================================================
    // get_or scenarios
    // ========================================================================

    #[tokio::test]
    async fn test_cache_get_or_cache_hit_fast_path() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        // Pre-populate cache
        cache
            .set(&"k".to_string(), &"cached".to_string())
            .await
            .unwrap();

        // get_or should return cached value without calling fallback
        let value = cache
            .get_or(&"k".to_string(), || async {
                Err(OxCacheError::Operation(
                    "fallback should not be called".to_string(),
                ))
            })
            .await
            .unwrap();
        assert_eq!(value, "cached");
    }

    #[tokio::test]
    async fn test_cache_get_or_fallback_error_propagates() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        let result: OxCacheResult<String> = cache
            .get_or(&"missing".to_string(), || async {
                Err(OxCacheError::Operation("db down".to_string()))
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(OxCacheError::Operation(msg)) => assert_eq!(msg, "db down"),
            _ => panic!("expected OxCacheError::Operation"),
        }
    }

    #[tokio::test]
    async fn test_cache_get_or_writes_to_cache() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();

        // First call: miss, fallback computes and caches
        let v1 = cache
            .get_or(&"k".to_string(), || async { Ok(99) })
            .await
            .unwrap();
        assert_eq!(v1, 99);

        // Verify it was cached: a direct get should return the value
        let cached = cache.get(&"k".to_string()).await.unwrap().unwrap();
        assert_eq!(cached, 99);
    }

    // ========================================================================
    // capacity / shutdown
    // ========================================================================

    #[tokio::test]
    async fn test_cache_capacity() {
        let cache: Cache<String, String> = Cache::builder().capacity(500).build().await.unwrap();

        let capacity = cache.capacity().await.unwrap();
        assert_eq!(capacity, 500);
    }

    #[tokio::test]
    async fn test_cache_shutdown() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set(&"k".to_string(), &"v".to_string()).await.unwrap();

        // Should not panic
        cache.shutdown().await;
    }

    // ========================================================================
    // deserialize_value internal functions
    // ========================================================================

    /// 热路径借用查询语义与吞吐对比（本机 debug 口径记录 docs/PERFORMANCE.md）
    #[tokio::test(flavor = "multi_thread")]
    async fn get_by_str_semantics_and_throughput() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set(&"hit".to_string(), &"v".to_string())
            .await
            .unwrap();

        // 语义：K=String 时 get_by_str 与 get 等价
        assert_eq!(
            cache.get_by_str("hit").await.unwrap(),
            Some("v".to_string())
        );
        assert_eq!(cache.get_by_str("miss").await.unwrap(), None);
        cache
            .set_by_str("via-str", &"w".to_string(), None)
            .await
            .unwrap();
        assert_eq!(
            cache.get(&"via-str".to_string()).await.unwrap(),
            Some("w".to_string())
        );

        // 吞吐对比（相对值；绝对值随环境波动）
        // 公平口径：两个独立循环、各自 2 轮取优，均查询同样 100 个命中键
        const ITER: u32 = 50_000;
        let measure_owned = || async {
            let mut total = Duration::ZERO;
            for round in 0..2 {
                let t = std::time::Instant::now();
                for i in 0..ITER {
                    let key = format!("hit{}", (i + round) % 100);
                    let _: Option<String> = cache.get(&key).await.unwrap();
                }
                if round == 1 {
                    total = t.elapsed();
                }
            }
            total
        };
        let measure_borrowed = || async {
            let mut total = Duration::ZERO;
            for round in 0..2 {
                let t = std::time::Instant::now();
                for i in 0..ITER {
                    let key = format!("hit{}", (i + round) % 100);
                    let _: Option<String> = cache.get_by_str(&key).await.unwrap();
                }
                if round == 1 {
                    total = t.elapsed();
                }
            }
            total
        };
        let owned_total = measure_owned().await;
        let borrowed_total = measure_borrowed().await;
        let owned_us = owned_total.as_micros();
        let borrowed_us = borrowed_total.as_micros();
        println!(
            "hot path ({} iters, debug profile): get(owned)={}us get_by_str(borrowed)={}us",
            ITER, owned_us, borrowed_us
        );
        // 借用查询不应显著劣化（允许测量抖动）
        assert!(
            borrowed_total <= owned_total.saturating_mul(3),
            "borrowed 路径不应慢于 owned 3 倍以上: {owned_us}us vs {borrowed_us}us"
        );
    }

    #[tokio::test]
    async fn test_deserialize_value_valid() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
        cache.set(&"k".to_string(), &42).await.unwrap();

        // get() internally calls deserialize_value
        let v = cache.get(&"k".to_string()).await.unwrap().unwrap();
        assert_eq!(v, 42);
    }

    #[tokio::test]
    async fn test_deserialize_value_invalid_json() {
        // Store invalid JSON bytes directly via backend
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
        cache
            .backend
            .set(Arc::from("bad"), Arc::new(b"not json".to_vec()), None)
            .await
            .unwrap();

        // get() should return a serialization error
        let result = cache.get(&"bad".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_deserialize_value_depth_exceeded() {
        // Build a deeply nested JSON that exceeds MAX_JSON_DEPTH (64)
        let mut json_str = String::new();
        for _ in 0..(MAX_JSON_DEPTH + 5) {
            json_str.push('[');
        }
        for _ in 0..(MAX_JSON_DEPTH + 5) {
            json_str.push(']');
        }

        let cache: Cache<String, serde_json::Value> = Cache::builder().build().await.unwrap();
        cache
            .backend
            .set(Arc::from("deep"), Arc::new(json_str.into_bytes()), None)
            .await
            .unwrap();

        let result = cache.get(&"deep".to_string()).await;
        assert!(result.is_err());
        match result {
            Err(OxCacheError::Serialization(msg)) => {
                assert!(msg.contains("深度") || msg.contains("depth"));
            }
            _ => panic!("expected OxCacheError::Serialization"),
        }
    }

    // ========================================================================
    // Async keys(), ttl(), expire() coverage
    // ========================================================================

    #[tokio::test]
    async fn test_cache_keys_returns_matching() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set(&"user:1".to_string(), &"a".to_string())
            .await
            .unwrap();
        cache
            .set(&"user:2".to_string(), &"b".to_string())
            .await
            .unwrap();
        cache
            .set(&"session:1".to_string(), &"c".to_string())
            .await
            .unwrap();

        let all = cache.keys("*").await.unwrap();
        assert_eq!(all.len(), 3);

        let users = cache.keys("user:*").await.unwrap();
        assert_eq!(users.len(), 2);

        let none = cache.keys("nope:*").await.unwrap();
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn test_cache_ttl_returns_remaining() {
        // 断言精确 TTL 窗口：显式关闭默认抖动（审计 F06）
        let cache: Cache<String, String> = Cache::builder().ttl_jitter(0.0).build().await.unwrap();
        cache
            .set_with_ttl(
                &"k".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        let ttl = cache
            .ttl(&"k".to_string())
            .await
            .unwrap()
            .expect("ttl should be Some");
        assert!(ttl > Duration::from_secs(58));
        assert!(ttl <= Duration::from_secs(60));
        // Missing key
        assert_eq!(cache.ttl(&"missing".to_string()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_cache_expire_extends_ttl() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set_with_ttl(
                &"k".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        let ok = cache
            .expire(&"k".to_string(), Duration::from_secs(120))
            .await
            .unwrap();
        assert!(ok);
        let ttl = cache
            .ttl(&"k".to_string())
            .await
            .unwrap()
            .expect("ttl should be Some");
        assert!(ttl > Duration::from_secs(118));
        // expire missing key
        let ok = cache
            .expire(&"missing".to_string(), Duration::from_secs(60))
            .await
            .unwrap();
        assert!(!ok);
    }

    #[tokio::test]
    async fn test_get_or_follower_not_hung_when_leader_set_fails() {
        // Regression: leader's `set` failure after a successful fallback must
        // still notify waiting followers, otherwise they hang forever.
        use crate::testing::MockBackend;

        let backend: Arc<dyn crate::backend::CacheBackend> =
            Arc::new(MockBackend::new("mock", 50, false).with_fail_set());
        let cache: Arc<Cache<String, f64>> = Arc::new(Cache::new_with_backend(backend));

        let (leader_registered_tx, leader_registered_rx) = tokio::sync::oneshot::channel();
        let (leader_go_tx, leader_go_rx) = tokio::sync::oneshot::channel();

        // Leader: fallback blocks until the follower has registered, so the
        // follower is guaranteed to be waiting when the leader's set fails.
        let cache_leader = cache.clone();
        let leader = tokio::spawn(async move {
            cache_leader
                .get_or(&"k".to_string(), || async {
                    let _ = leader_registered_tx.send(());
                    let _ = leader_go_rx.await;
                    Ok(1.0f64)
                })
                .await
        });

        let _ = leader_registered_rx.await;
        let cache_follower = cache.clone();
        let follower = tokio::spawn(async move {
            tokio::time::timeout(
                Duration::from_secs(5),
                cache_follower.get_or(&"k".to_string(), || async { Ok(2.0f64) }),
            )
            .await
        });

        // Let the follower register as a follower, then release the leader.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = leader_go_tx.send(());

        let _ = leader.await;
        let follower_result = follower.await.unwrap();
        assert!(
            follower_result.is_ok(),
            "follower must resolve (timeout indicates hang): {:?}",
            follower_result
        );
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;
    use crate::backend::MokaMemoryBackend;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::thread;
    use std::time::Duration;

    /// Helper: construct a Cache whose `backend_sync` is wired to the same
    /// Moka instance as the async backend. Mirrors what
    /// `CacheBuilder::sync_mode(true)` will do in task group 10.
    fn make_sync_cache() -> Cache<String, String> {
        let moka = Arc::new(MokaMemoryBackend::new());
        let mut cache: Cache<String, String> = Cache::new_with_backend(moka.clone());
        cache.set_sync_backend(moka);
        // 既有 sync 测试断言精确 TTL 窗口：显式关闭默认抖动（审计 F06）
        cache.set_ttl_jitter_factor(0.0);
        cache
    }

    #[test]
    fn test_cache_get_sync_set_sync_basic() {
        let cache = make_sync_cache();
        cache.set_sync(&"k".to_string(), &"v".to_string()).unwrap();
        let v = cache.get_sync(&"k".to_string()).unwrap();
        assert_eq!(v, Some("v".to_string()));
    }

    #[test]
    fn test_cache_get_sync_without_sync_mode_returns_err() {
        // Cache::new() leaves backend_sync = None
        let cache: Cache<String, String> = Cache::new();
        let result = cache.get_sync(&"k".to_string());
        assert!(
            matches!(result, Err(OxCacheError::NotSupported(_))),
            "expected Err(NotSupported), got {:?}",
            result
        );
    }

    #[test]
    fn test_cache_get_or_sync_cache_hit() {
        let cache = make_sync_cache();
        cache
            .set_sync(&"k".to_string(), &"cached".to_string())
            .unwrap();

        // Fallback should NOT be called — pre-populated value wins
        let v = cache
            .get_or_sync(&"k".to_string(), || {
                Err(OxCacheError::Operation(
                    "fallback should not run".to_string(),
                ))
            })
            .unwrap();
        assert_eq!(v, "cached");
    }

    // NOTE: test_cache_get_or_sync_cache_miss_triggers_fallback removed —
    // sync bridge (block_in_place) is incompatible with test runtime contexts.
    // The single_flight test below covers the get_or_sync leader path.

    #[test]
    fn test_cache_get_or_sync_single_flight_prevents_duplicate_fallback() {
        let cache = Arc::new(make_sync_cache());
        let counter = Arc::new(AtomicU32::new(0));

        // Thread A: becomes leader, sleeps inside fallback to give B time to
        // arrive and become a follower.
        let cache_a = cache.clone();
        let counter_a = counter.clone();
        let handle_a = thread::spawn(move || {
            cache_a
                .get_or_sync(&"k".to_string(), || {
                    counter_a.fetch_add(1, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(120));
                    Ok("v".to_string())
                })
                .unwrap()
        });

        // Give A time to register as leader before B arrives.
        thread::sleep(Duration::from_millis(20));

        let cache_b = cache.clone();
        let counter_b = counter.clone();
        let handle_b = thread::spawn(move || {
            cache_b
                .get_or_sync(&"k".to_string(), || {
                    counter_b.fetch_add(1, Ordering::SeqCst);
                    Ok("should_not_run".to_string())
                })
                .unwrap()
        });

        let v_a = handle_a.join().expect("thread A panicked");
        let v_b = handle_b.join().expect("thread B panicked");

        assert_eq!(v_a, "v");
        assert_eq!(v_b, "v");
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "fallback must run exactly once under single-flight"
        );
    }

    #[test]
    fn test_cache_set_with_ttl_sync_expires() {
        let cache = make_sync_cache();
        cache
            .set_with_ttl_sync(
                &"k".to_string(),
                &"v".to_string(),
                Some(Duration::from_millis(50)),
            )
            .unwrap();

        // Within TTL window: readable
        assert_eq!(
            cache.get_sync(&"k".to_string()).unwrap(),
            Some("v".to_string())
        );

        // After TTL: expired
        thread::sleep(Duration::from_millis(120));
        assert_eq!(cache.get_sync(&"k".to_string()).unwrap(), None);
    }

    #[test]
    fn test_cache_delete_sync() {
        let cache = make_sync_cache();
        cache.set_sync(&"k".to_string(), &"v".to_string()).unwrap();
        cache.delete_sync(&"k".to_string()).unwrap();
        assert_eq!(cache.get_sync(&"k".to_string()).unwrap(), None);
    }

    #[test]
    fn test_cache_exists_sync() {
        let cache = make_sync_cache();
        assert!(!cache.exists_sync(&"k".to_string()).unwrap());
        cache.set_sync(&"k".to_string(), &"v".to_string()).unwrap();
        assert!(cache.exists_sync(&"k".to_string()).unwrap());
    }

    #[test]
    fn test_cache_ttl_sync() {
        let cache = make_sync_cache();
        cache
            .set_with_ttl_sync(
                &"k".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .unwrap();
        let ttl = cache
            .ttl_sync(&"k".to_string())
            .unwrap()
            .expect("ttl should be Some");
        assert!(ttl > Duration::from_secs(58));
        assert!(ttl <= Duration::from_secs(60));
        // Missing key
        assert_eq!(cache.ttl_sync(&"missing".to_string()).unwrap(), None);
    }

    #[test]
    fn test_cache_expire_sync() {
        let cache = make_sync_cache();
        cache
            .set_with_ttl_sync(
                &"k".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .unwrap();
        let ok = cache
            .expire_sync(&"k".to_string(), Duration::from_secs(120))
            .unwrap();
        assert!(ok);
        let ttl = cache
            .ttl_sync(&"k".to_string())
            .unwrap()
            .expect("ttl should be Some");
        assert!(ttl > Duration::from_secs(118));
        // expire missing key
        let ok = cache
            .expire_sync(&"missing".to_string(), Duration::from_secs(60))
            .unwrap();
        assert!(!ok);
    }

    #[test]
    fn test_cache_sync_methods_without_sync_mode_returns_err() {
        let cache: Cache<String, String> = Cache::new();
        assert!(cache.delete_sync(&"k".to_string()).is_err());
        assert!(cache.exists_sync(&"k".to_string()).is_err());
        assert!(cache.ttl_sync(&"k".to_string()).is_err());
        assert!(
            cache
                .expire_sync(&"k".to_string(), Duration::from_secs(1))
                .is_err()
        );
    }

    /// Sync backend stub that delegates reads to Moka but fails every `set`.
    /// Pins the `get_or_option_sync` error-propagation contract: a leader
    /// whose cache write fails must surface `Err`, not a phantom `Ok`.
    struct FailSetBackend {
        inner: MokaMemoryBackend,
    }

    impl FailSetBackend {
        fn new() -> Self {
            Self {
                inner: MokaMemoryBackend::new(),
            }
        }
    }

    impl crate::backend::SyncCacheReader for FailSetBackend {
        fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
            self.inner.get(key)
        }

        fn exists(&self, key: &str) -> OxCacheResult<bool> {
            self.inner.exists(key)
        }

        fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
            self.inner.ttl(key)
        }

        fn len(&self) -> OxCacheResult<u64> {
            self.inner.len()
        }

        fn capacity(&self) -> OxCacheResult<u64> {
            Ok(self.inner.capacity())
        }

        fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
            self.inner.stats()
        }
    }

    impl crate::backend::SyncCacheWriter for FailSetBackend {
        fn set(
            &self,
            _key: Arc<str>,
            _value: Arc<Vec<u8>>,
            _ttl: Option<Duration>,
        ) -> OxCacheResult<()> {
            Err(OxCacheError::Operation(
                "FailSetBackend: injected set failure".to_string(),
            ))
        }

        fn delete(&self, key: &str) -> OxCacheResult<()> {
            self.inner.delete(key)
        }

        fn clear(&self) -> OxCacheResult<()> {
            self.inner.clear()
        }

        fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
            self.inner.expire(key, ttl)
        }
    }

    impl crate::backend::SyncCacheConnector for FailSetBackend {
        fn health_check(&self) -> OxCacheResult<()> {
            self.inner.health_check()
        }

        fn shutdown(&self) {}

        fn backend_kind(&self) -> crate::backend::BackendKind {
            self.inner.backend_kind()
        }
    }

    fn make_failing_sync_cache() -> Cache<String, String> {
        let moka = Arc::new(MokaMemoryBackend::new());
        let mut cache: Cache<String, String> = Cache::new_with_backend(moka);
        cache.set_sync_backend(Arc::new(FailSetBackend::new()));
        cache
    }

    #[test]
    fn test_get_or_option_sync_propagates_value_set_failure() {
        let cache = make_failing_sync_cache();
        let result = cache.get_or_option_sync(&"k-value-set-fail".to_string(), || {
            Ok(Some("v".to_string()))
        });
        assert!(
            result.is_err(),
            "leader set failure must propagate, got {:?}",
            result
        );
    }

    #[test]
    fn test_get_or_option_sync_propagates_sentinel_set_failure() {
        let mut cache = make_failing_sync_cache();
        cache.set_null_cache_ttl(Some(Duration::from_secs(60)));
        let result = cache.get_or_option_sync(&"k-sentinel-set-fail".to_string(), || Ok(None));
        assert!(
            result.is_err(),
            "sentinel set failure must propagate, got {:?}",
            result
        );
    }
}

#[cfg(test)]
mod sentinel_race_tests {
    use super::*;
    use crate::backend::memory::MokaMemoryBackend;
    use crate::backend::{CacheConnector, CacheReader, CacheWriter};
    use std::sync::atomic::AtomicBool;

    /// 首次 get 返回 None（模拟快速路径 miss），其后透传真实数据的 scripted backend。
    struct FirstGetMissBackend {
        inner: Arc<MokaMemoryBackend>,
        key: &'static str,
        swallowed: AtomicBool,
    }

    impl FirstGetMissBackend {
        fn new(key: &'static str) -> Self {
            Self {
                inner: Arc::new(MokaMemoryBackend::new()),
                key,
                swallowed: AtomicBool::new(false),
            }
        }
    }

    #[async_trait::async_trait]
    impl CacheReader for FirstGetMissBackend {
        async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
            if key == self.key
                && !self
                    .swallowed
                    .swap(true, std::sync::atomic::Ordering::SeqCst)
            {
                return Ok(None);
            }
            self.inner.get(key).await
        }
        async fn exists(&self, key: &str) -> OxCacheResult<bool> {
            self.inner.exists(key).await
        }
        async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
            CacheReader::ttl(&*self.inner, key).await
        }
        async fn len(&self) -> OxCacheResult<u64> {
            self.inner.len().await
        }
        async fn capacity(&self) -> OxCacheResult<u64> {
            Ok(self.inner.capacity())
        }
        async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
            self.inner.stats().await
        }
    }

    #[async_trait::async_trait]
    impl CacheWriter for FirstGetMissBackend {
        async fn set(
            &self,
            key: Arc<str>,
            value: Arc<Vec<u8>>,
            ttl: Option<Duration>,
        ) -> OxCacheResult<()> {
            self.inner.set(key, value, ttl).await
        }
        async fn delete(&self, key: &str) -> OxCacheResult<()> {
            self.inner.delete(key).await
        }
        async fn clear(&self) -> OxCacheResult<()> {
            self.inner.clear().await
        }
        async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
            self.inner.expire(key, ttl).await
        }
    }

    #[async_trait::async_trait]
    impl CacheConnector for FirstGetMissBackend {
        async fn health_check(&self) -> OxCacheResult<()> {
            self.inner.health_check().await
        }
        async fn shutdown(&self) {
            self.inner.shutdown().await
        }
        fn backend_kind(&self) -> crate::backend::BackendKind {
            self.inner.backend_kind()
        }
    }

    /// 审计 F02 确定性回归：快速路径 get miss 后、哨兵判定前该 key 被并发写入
    /// 真实值。旧 `exists()` 判定无法区分哨兵与真实值，会把真实值误判为
    /// "哨兵有效" 返回 Ok(None)；get + 字节比对必须返回 Ok(Some(真实值))。
    #[tokio::test]
    async fn get_or_option_returns_real_value_written_after_fast_path() {
        let backend: Arc<dyn crate::backend::CacheBackend> =
            Arc::new(FirstGetMissBackend::new("race-real"));
        let mut cache: Cache<String, String> = Cache::new_with_backend(backend);
        cache.set_null_cache_ttl(Some(Duration::from_secs(60)));

        // 真实值在"快速路径 miss 之后"才可见（scripted backend 吞掉首查）
        cache
            .backend
            .set(
                Arc::from("race-real"),
                Arc::new(b"\"real-value\"".to_vec()),
                None,
            )
            .await
            .unwrap();

        let got = cache
            .get_or_option(&"race-real".to_string(), || async {
                Err(OxCacheError::Operation("fallback must not run".into()))
            })
            .await
            .unwrap();
        assert_eq!(
            got,
            Some("real-value".to_string()),
            "真实值不得被 exists 判定误判为空值哨兵"
        );
    }
}

#[cfg(test)]
mod jitter_tests {
    use super::*;

    fn jitter_cache(factor: f64) -> Cache<String, String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut cache: Cache<String, String> = rt
            .block_on(async { Cache::builder().build().await })
            .unwrap();
        cache.set_ttl_jitter_factor(factor);
        cache
    }

    /// 审计 F07 验收：10000 次采样全部落在 [ttl*(1-f), ttl*(1+f)] 且分布非退化
    #[test]
    fn jitter_samples_within_bounds_and_non_degenerate() {
        let cache = jitter_cache(0.1);
        let base = Duration::from_secs(60);
        let low = base.mul_f64(0.9).as_millis() as u64;
        let high = base.mul_f64(1.1).as_millis() as u64;
        let mut samples: Vec<u64> = (0..10_000)
            .map(|_| cache.apply_jitter(base).as_millis() as u64)
            .collect();
        assert!(
            samples.iter().all(|&s| s >= low && s <= high),
            "采样必须落在 ±factor 区间内 [{low}, {high}]"
        );
        samples.sort_unstable();
        assert!(
            samples[0] < samples[5_000] && samples[5_000] < samples[9_999],
            "分布非退化：min < median < max，实际 {} / {} / {}",
            samples[0],
            samples[5_000],
            samples[9_999]
        );
    }

    /// factor=0 恒等返回（显式关闭抖动语义不变）
    #[test]
    fn zero_factor_is_identity() {
        let cache = jitter_cache(0.0);
        let base = Duration::from_secs(60);
        for _ in 0..100 {
            assert_eq!(cache.apply_jitter(base), base);
        }
    }
}

#[cfg(test)]
mod get_or_with_ttl_tests {
    use super::*;
    use crate::backend::memory::MokaMemoryBackend;
    use std::sync::Arc as StdArc;

    /// 审计 F09 验收：get_or_with_ttl 写入带 TTL（≤ 传入值，抖动上界）
    #[tokio::test]
    async fn get_or_with_ttl_caches_with_ttl() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let v = cache
            .get_or_with_ttl(
                &"ttl-or".to_string(),
                Some(Duration::from_secs(60)),
                || async { Ok("v".to_string()) },
            )
            .await
            .unwrap();
        assert_eq!(v, "v");
        let ttl = cache
            .ttl(&"ttl-or".to_string())
            .await
            .unwrap()
            .expect("ttl 应存在");
        // 默认抖动 ±10%：60s → [54s, 66s)
        assert!(
            ttl >= Duration::from_secs(54) && ttl < Duration::from_secs(66),
            "ttl {ttl:?} 应在抖动区间 [54s, 66s)"
        );

        // 旧 get_or 语义不变：无 TTL
        let cache2: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache2
            .get_or(&"no-ttl-or".to_string(), || async { Ok("v".to_string()) })
            .await
            .unwrap();
        assert_eq!(
            cache2.ttl(&"no-ttl-or".to_string()).await.unwrap(),
            None,
            "get_or 旧路径必须保持无 TTL 语义"
        );
    }

    /// get_or_option_with_ttl：真实值与空值哨兵均带抖动 TTL
    #[tokio::test]
    async fn get_or_option_with_ttl_jitters_sentinel() {
        let mut cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set_null_cache_ttl(Some(Duration::from_secs(30)));

        // fallback 返回 None → 哨兵写入且带 TTL
        let got = cache
            .get_or_option_with_ttl(
                &"sentinel-ttl".to_string(),
                Some(Duration::from_secs(60)),
                || async { Ok(None) },
            )
            .await
            .unwrap();
        assert_eq!(got, None);
        let ttl = cache
            .ttl(&"sentinel-ttl".to_string())
            .await
            .unwrap()
            .expect("哨兵应存在");
        assert!(
            ttl >= Duration::from_secs(27) && ttl < Duration::from_secs(33),
            "哨兵 ttl {ttl:?} 应在抖动区间 [27s, 33s)"
        );

        // 第二次调用命中哨兵直接返回 None（不执行 fallback）
        let got2 = cache
            .get_or_option_with_ttl(
                &"sentinel-ttl".to_string(),
                Some(Duration::from_secs(60)),
                || async { Err(OxCacheError::Operation("must not run".into())) },
            )
            .await
            .unwrap();
        assert_eq!(got2, None);
    }

    /// sync 变体：get_or_with_ttl_sync 带 TTL
    #[test]
    fn get_or_with_ttl_sync_caches_with_ttl() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut cache: Cache<String, String> = rt
            .block_on(async { Cache::builder().build().await })
            .unwrap();
        cache.set_sync_backend(StdArc::new(MokaMemoryBackend::new()));

        cache
            .get_or_with_ttl_sync(
                &"ttl-sync".to_string(),
                Some(Duration::from_secs(60)),
                || Ok("v".to_string()),
            )
            .unwrap();
        let ttl = cache
            .ttl_sync(&"ttl-sync".to_string())
            .unwrap()
            .expect("ttl 应存在");
        // 默认抖动 ±10%：60s → [54s, 66s)
        assert!(
            ttl >= Duration::from_secs(54) && ttl < Duration::from_secs(66),
            "ttl {ttl:?} 应在抖动区间 [54s, 66s)"
        );
    }
}

#[cfg(test)]
mod default_jitter_tests {
    use super::*;

    /// 审计 F06 验收：默认构建抖动 0.1 生效（±10%），显式 0.0 关闭后恒等
    #[tokio::test]
    async fn default_jitter_is_on_and_can_be_disabled() {
        // 默认构建：60s → [54s, 66s)
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set_with_ttl(
                &"dj-on".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        let ttl = cache.ttl(&"dj-on".to_string()).await.unwrap().unwrap();
        assert!(
            ttl >= Duration::from_secs(54) && ttl < Duration::from_secs(66),
            "默认抖动应使 ttl ∈ [54s, 66s)，实际 {ttl:?}"
        );

        // 显式关闭：60s → (58s, 60s]
        let cache2: Cache<String, String> = Cache::builder().ttl_jitter(0.0).build().await.unwrap();
        cache2
            .set_with_ttl(
                &"dj-off".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        let ttl2 = cache2.ttl(&"dj-off".to_string()).await.unwrap().unwrap();
        assert!(
            ttl2 > Duration::from_secs(58),
            "关闭抖动后 ttl 应 ≈ 60s，实际 {ttl2:?}"
        );
    }
}
