// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Cache 基础操作方法

use super::Cache;
use crate::core::constants::NULL_SENTINEL;
use crate::error::{OxCacheError, OxCacheResult};
#[cfg(feature = "stale")]
use crate::i18n::messages::MSG_PANIC_STALE_STATE_PAYLOAD;
use crate::i18n::messages::{
    MSG_DETAIL_GET_OR_LEADER_NOT_CACHED, MSG_DETAIL_GET_OR_OPTION_LEADER_NOT_CACHED, t,
};
#[cfg(all(feature = "stale", feature = "telemetry"))]
use crate::i18n::messages::{MSG_LOG_STALE_HIT_VIA_GET_OR, MSG_LOG_STALE_REVALIDATION_SCHEDULED};
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
    tracing::debug!(
        target: "oxcache::stale",
        key,
        "{}",
        t(MSG_LOG_STALE_HIT_VIA_GET_OR, &[])
    );
}

#[cfg(not(all(feature = "stale", feature = "telemetry")))]
#[inline]
fn telemetry_stale_downgrade(_key: &str) {}

#[cfg(all(feature = "stale", feature = "telemetry"))]
#[inline]
fn telemetry_stale_refresh(key: &str, spawned: bool) {
    tracing::debug!(
        target: "oxcache::stale",
        key,
        spawned,
        "{}",
        t(MSG_LOG_STALE_REVALIDATION_SCHEDULED, &[])
    );
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
                let raw =
                    bytes.unwrap_or_else(|| panic!("{}", t(MSG_PANIC_STALE_STATE_PAYLOAD, &[])));
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
                    OxCacheError::L1Error(t(MSG_DETAIL_GET_OR_LEADER_NOT_CACHED, &[]))
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
                Err(OxCacheError::L1Error(t(
                    MSG_DETAIL_GET_OR_OPTION_LEADER_NOT_CACHED,
                    &[],
                )))
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
mod tests;
