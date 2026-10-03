// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 自适应 TTL（R5，`adaptive-ttl` feature，默认关闭）。
//!
//! 按访问模式调整条目 TTL 的装饰器后端：hot key 延长、cold key 缩短。
//! 全部阈值均为显式配置常量（[`AdaptiveTtlConfig`]），无黑盒启发式。
//!
//! # 验收口径
//!
//! - **访问频率度量**：get 命中按 key 计数（单调，内存上限见下）；
//!   set 也计为一次访问，使新键自然进入冷热演化。
//! - **调整上下界**：任何策略调整后的 TTL 都钳制在
//!   `[min_ttl, max_ttl]`（显式常量，默认 1s..1h）；`None`（永不过期）
//!   不参与调整，原样透传。
//! - **内存上限**：追踪表至多 `max_tracked_keys`（默认 65_536）条；
//!   满时新键不被追踪（按普通键透传），已有键继续演化。
//!
//! # 策略
//!
//! - **hot 调整**：命中计数 ≥ `hot_threshold` 的键，set 时 TTL 乘
//!   `hot_ttl_multiplier`；get 时若距上次调整 ≥ `adjust_interval`，
//!   对已存条目 `expire` 到 `clamp(剩余 TTL × multiplier)`（方向不限，
//!   基准超出上界同样收敛）——`adjust_interval` 限制写放大。
//! - **cold 缩短**：已知键的最近访问早于 `cold_idle_after` 时，
//!   set 的 TTL 除以 `cold_ttl_divisor`（钳 `min_ttl`）。
//! - hot 与 cold 同时满足时 hot 优先（扩展分支先于缩短分支判定）。
//!
//! # Example
//!
//! ```
//! use oxcache::features::adaptive_ttl::{AdaptiveTtlBackend, AdaptiveTtlConfig};
//! use oxcache::backend::MokaMemoryBackend;
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! let config = AdaptiveTtlConfig {
//!     hot_threshold: 2,
//!     min_ttl: Duration::from_secs(1),
//!     max_ttl: Duration::from_secs(60),
//!     ..AdaptiveTtlConfig::default()
//! };
//! let backend = AdaptiveTtlBackend::new(Arc::new(MokaMemoryBackend::builder().build()), config)
//!     .expect("valid config");
//! ```

use crate::backend::{CacheBackend, CacheConnector, CacheReader, CacheWriter};
use crate::error::{OxCacheError, OxCacheResult};
use crate::i18n::messages::{
    MSG_DETAIL_ADAPTIVE_TTL_DIVISOR_MIN, MSG_DETAIL_ADAPTIVE_TTL_MIN_EXCEEDS_MAX,
    MSG_DETAIL_ADAPTIVE_TTL_MULTIPLIER_FINITE, MSG_DETAIL_ADAPTIVE_TTL_TRACKED_KEYS_MIN, t,
};
use async_trait::async_trait;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// 单键访问状态（freq 单调计数 + 最近访问 + 上次延长时刻）
#[derive(Debug)]
struct AccessState {
    freq: u64,
    last_access: Instant,
    last_extend: Option<Instant>,
}

/// 自适应 TTL 配置：全部阈值显式，无隐式启发式
#[derive(Debug, Clone)]
pub struct AdaptiveTtlConfig {
    /// 命中计数达到该值即视为 hot（set 乘延长系数、get 触发主动延长）
    pub hot_threshold: u64,
    /// hot 键 TTL 延长系数（对基准/剩余 TTL 相乘，结果钳入上下界）
    pub hot_ttl_multiplier: f64,
    /// 已知键最近访问早于该时长即视为 cold（set 时 TTL 缩短）
    pub cold_idle_after: Duration,
    /// cold 键 TTL 缩短除数（结果钳 `min_ttl`）
    pub cold_ttl_divisor: u64,
    /// 调整后 TTL 下界
    pub min_ttl: Duration,
    /// 调整后 TTL 上界
    pub max_ttl: Duration,
    /// 同一键两次主动延长之间的最小间隔（限制 get 路径写放大）
    pub adjust_interval: Duration,
    /// 追踪表条目上限（内存上限；满时新键不被追踪，按普通键透传）。
    /// 0 在 [`AdaptiveTtlConfig::validate`] 显性拒绝——容量下限为 1，
    /// 静默关闭追踪不是合法口径
    pub max_tracked_keys: usize,
}

impl Default for AdaptiveTtlConfig {
    fn default() -> Self {
        Self {
            hot_threshold: 8,
            hot_ttl_multiplier: 2.0,
            cold_idle_after: Duration::from_secs(300),
            cold_ttl_divisor: 4,
            min_ttl: Duration::from_secs(1),
            max_ttl: Duration::from_secs(3600),
            adjust_interval: Duration::from_secs(60),
            max_tracked_keys: 65_536,
        }
    }
}

impl AdaptiveTtlConfig {
    /// 配置合法性校验：builder 装配与 [`AdaptiveTtlBackend::new`] 构造前调用，
    /// 非法配置在构建期显性拒绝（`Err(InvalidInput)`），而非请求路径上
    /// `Duration::clamp` panic 或经乘/除法静默畸变。
    pub fn validate(&self) -> OxCacheResult<()> {
        if self.min_ttl > self.max_ttl {
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_ADAPTIVE_TTL_MIN_EXCEEDS_MAX,
                &[
                    ("min", format!("{:?}", self.min_ttl)),
                    ("max", format!("{:?}", self.max_ttl)),
                ],
            )));
        }
        if !(self.hot_ttl_multiplier.is_finite() && self.hot_ttl_multiplier > 0.0) {
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_ADAPTIVE_TTL_MULTIPLIER_FINITE,
                &[("value", self.hot_ttl_multiplier.to_string())],
            )));
        }
        if self.cold_ttl_divisor == 0 {
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_ADAPTIVE_TTL_DIVISOR_MIN,
                &[],
            )));
        }
        if self.max_tracked_keys == 0 {
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_ADAPTIVE_TTL_TRACKED_KEYS_MIN,
                &[],
            )));
        }
        Ok(())
    }
}

/// 自适应 TTL 统计（R5 验收口径：可观测、可清零）
#[derive(Debug, Default)]
pub struct AdaptiveTtlStats {
    /// hot 延长执行次数（set 调整 + get 主动延长成功）
    pub hot_extensions: u64,
    /// cold 缩短执行次数
    pub cold_shortenings: u64,
    /// get 路径主动调整写失败次数（`expire` 返回 `Err`；尽力调整不阻断
    /// 命中，但失败必须显性计数而非静默吞掉）
    pub failed_adjustments: u64,
    /// 当前追踪键数
    pub tracked_keys: u64,
}

/// 自适应 TTL 装饰器：包任意 [`CacheBackend`]，按访问模式调整 TTL
pub struct AdaptiveTtlBackend {
    inner: Arc<dyn CacheBackend>,
    config: AdaptiveTtlConfig,
    accesses: DashMap<String, AccessState>,
    hot_extensions: AtomicU64,
    cold_shortenings: AtomicU64,
    failed_adjustments: AtomicU64,
}

impl AdaptiveTtlBackend {
    /// 包装 inner 后端，按给定显式配置执行 hot 延长 / cold 缩短。
    /// 配置非法（`min_ttl > max_ttl`、乘数非有限正数、除数为 0、
    /// `max_tracked_keys` 为 0）时返回 `Err(InvalidInput)`——拒绝发生在
    /// 构造期，而非请求路径的 clamp/乘法处。
    pub fn new(inner: Arc<dyn CacheBackend>, config: AdaptiveTtlConfig) -> OxCacheResult<Self> {
        config.validate()?;
        Ok(Self {
            inner,
            config,
            accesses: DashMap::new(),
            hot_extensions: AtomicU64::new(0),
            cold_shortenings: AtomicU64::new(0),
            failed_adjustments: AtomicU64::new(0),
        })
    }

    /// 当前统计快照（追踪键数 / 延长与缩短次数 / 调整写失败次数）
    pub fn stats(&self) -> AdaptiveTtlStats {
        AdaptiveTtlStats {
            hot_extensions: self.hot_extensions.load(Ordering::Relaxed),
            cold_shortenings: self.cold_shortenings.load(Ordering::Relaxed),
            failed_adjustments: self.failed_adjustments.load(Ordering::Relaxed),
            tracked_keys: self.accesses.len() as u64,
        }
    }

    /// 统计清零：清空访问追踪与延长/缩短/失败计数（不影响 inner 数据）
    pub fn reset_stats(&self) {
        self.accesses.clear();
        self.hot_extensions.store(0, Ordering::Relaxed);
        self.cold_shortenings.store(0, Ordering::Relaxed);
        self.failed_adjustments.store(0, Ordering::Relaxed);
    }

    /// 记录一次访问并返回 (freq, last_access_before_this_call)。
    /// 追踪表满时返回 `None`（该键按普通键透传）。
    fn record_access(&self, key: &str) -> Option<(u64, Instant)> {
        let now = Instant::now();
        match self.accesses.get_mut(key) {
            Some(mut state) => {
                state.freq = state.freq.saturating_add(1);
                let last = state.last_access;
                state.last_access = now;
                Some((state.freq, last))
            }
            None => {
                if self.accesses.len() >= self.config.max_tracked_keys {
                    return None;
                }
                self.accesses.insert(
                    key.to_string(),
                    AccessState {
                        freq: 1,
                        last_access: now,
                        last_extend: None,
                    },
                );
                Some((1, now))
            }
        }
    }

    /// set 路径的 TTL 调整：hot 乘延长系数、cold 除缩短系数，均钳上下界
    fn adjusted_set_ttl(&self, ttl: Duration, freq: u64, last_access: Instant) -> Duration {
        let now = Instant::now();
        let clamp = |d: Duration| d.clamp(self.config.min_ttl, self.config.max_ttl);
        if freq >= self.config.hot_threshold {
            let scaled = self.scale_ttl(ttl, self.config.hot_ttl_multiplier);
            self.hot_extensions.fetch_add(1, Ordering::Relaxed);
            return clamp(scaled);
        }
        if now.duration_since(last_access) > self.config.cold_idle_after {
            let divisor = self.config.cold_ttl_divisor.max(1) as f64;
            let shortened = self.scale_ttl(ttl, 1.0 / divisor);
            self.cold_shortenings.fetch_add(1, Ordering::Relaxed);
            return clamp(shortened);
        }
        ttl
    }

    /// 乘系数并饱和到毫秒 u64（f64 精度不足时上取整到 Duration::MAX）
    fn scale_ttl(&self, ttl: Duration, factor: f64) -> Duration {
        let ms = ttl.as_millis();
        let scaled = (ms as f64 * factor).max(1.0);
        if scaled >= u64::MAX as f64 {
            Duration::MAX
        } else {
            Duration::from_millis(scaled as u64)
        }
    }

    /// get 路径的 hot 主动延长：受 `adjust_interval` 限速，对已存条目
    /// `expire` 到 `clamp(multiplier × 剩余 TTL)`——hot 政策要求 TTL 收敛到
    /// 钳制区间（方向不限），与剩余相等时才跳过这次物理写
    async fn maybe_extend_hot(&self, key: &str, freq: u64) {
        if freq < self.config.hot_threshold {
            return;
        }
        let now = Instant::now();
        // 判定与 last_extend 戳记在单次 entry 内完成（判定通过才盖戳），
        // 避免同一键读/写两次分片锁
        let should_extend = match self.accesses.get_mut(key) {
            Some(mut state) => {
                let due = match state.last_extend {
                    Some(last) => now.duration_since(last) >= self.config.adjust_interval,
                    None => true,
                };
                if due {
                    state.last_extend = Some(now);
                }
                due
            }
            None => false,
        };
        if !should_extend {
            return;
        }
        // inner.ttl 在装饰器内可能为 None（永不过期）：无基准不延长
        if let Ok(Some(remaining)) = self.inner.ttl(key).await {
            let scaled = self.scale_ttl(remaining, self.config.hot_ttl_multiplier);
            let target = scaled.clamp(self.config.min_ttl, self.config.max_ttl);
            if target != remaining {
                // 尽力调整：expire 失败不阻断 get 命中本身，但必须显性计数
                // （failed_adjustments）而非静默吞掉
                match self.inner.expire(key, target).await {
                    Ok(_) => {
                        self.hot_extensions.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => {
                        self.failed_adjustments.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }
}

#[async_trait]
impl CacheReader for AdaptiveTtlBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        let value = self.inner.get(key).await?;
        if value.is_some()
            && let Some((freq, _)) = self.record_access(key)
        {
            self.maybe_extend_hot(key, freq).await;
        }
        Ok(value)
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        self.inner.exists(key).await
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
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
            "adaptive_tracked_keys".to_string(),
            self.accesses.len().to_string(),
        );
        stats.insert(
            "adaptive_hot_extensions".to_string(),
            self.hot_extensions.load(Ordering::Relaxed).to_string(),
        );
        stats.insert(
            "adaptive_cold_shortenings".to_string(),
            self.cold_shortenings.load(Ordering::Relaxed).to_string(),
        );
        stats.insert(
            "adaptive_failed_adjustments".to_string(),
            self.failed_adjustments.load(Ordering::Relaxed).to_string(),
        );
        Ok(stats)
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        self.inner.keys(pattern).await
    }
}

#[async_trait]
impl CacheWriter for AdaptiveTtlBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let Some(ttl) = ttl else {
            // 永不过期键不参与自适应（验收口径：调整仅作用于有限 TTL）
            return self.inner.set(key, value, None).await;
        };
        let (freq, last_access) = match self.record_access(&key) {
            Some(pair) => pair,
            None => return self.inner.set(key, value, Some(ttl)).await,
        };
        let adjusted = self.adjusted_set_ttl(ttl, freq, last_access);
        self.inner.set(key, value, Some(adjusted)).await
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.inner.delete(key).await
    }

    async fn clear(&self) -> OxCacheResult<()> {
        // 数据清空即访问历史失义：统计一并清零
        self.reset_stats();
        self.inner.clear().await
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        self.inner.expire(key, ttl).await
    }

    async fn set_many(&self, items: &[crate::backend::CacheSetItem]) -> OxCacheResult<()> {
        let mut adjusted: Vec<crate::backend::CacheSetItem> = Vec::with_capacity(items.len());
        for item in items {
            let (key, value, ttl) = (&item.0, &item.1, &item.2);
            let (freq, last_access) = match self.record_access(key) {
                Some(pair) => pair,
                None => {
                    adjusted.push((key.clone(), value.clone(), *ttl));
                    continue;
                }
            };
            match *ttl {
                Some(base) => {
                    let adjusted_ttl = self.adjusted_set_ttl(base, freq, last_access);
                    adjusted.push((key.clone(), value.clone(), Some(adjusted_ttl)));
                }
                None => adjusted.push((key.clone(), value.clone(), None)),
            }
        }
        self.inner.set_many(&adjusted).await
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        self.inner.delete_many(keys).await
    }
}

#[async_trait]
impl CacheConnector for AdaptiveTtlBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        self.inner.health_check().await
    }

    async fn shutdown(&self) {
        self.inner.shutdown().await;
    }

    fn backend_kind(&self) -> crate::backend::BackendKind {
        self.inner.backend_kind()
    }
}

// CacheBackend 由 blanket impl 自动提供

// 测试经由 MokaMemoryBackend 充当 inner：adaptive-ttl 隐含 memory 基线
// （与 degradation 同口径），单开 adaptive-ttl 即满足 moka 门控
#[cfg(all(test, feature = "adaptive-ttl"))]
mod tests {
    use super::*;
    use crate::backend::{MockBackend, MokaMemoryBackend};
    use crate::error::OxCacheError;

    fn hot_config() -> AdaptiveTtlConfig {
        AdaptiveTtlConfig {
            hot_threshold: 2,
            hot_ttl_multiplier: 100.0,
            min_ttl: Duration::from_secs(1),
            max_ttl: Duration::from_secs(5),
            ..AdaptiveTtlConfig::default()
        }
    }

    fn cold_config() -> AdaptiveTtlConfig {
        AdaptiveTtlConfig {
            hot_threshold: 1_000,
            cold_idle_after: Duration::ZERO,
            cold_ttl_divisor: 1_000,
            min_ttl: Duration::from_secs(7),
            max_ttl: Duration::from_secs(3_600),
            ..AdaptiveTtlConfig::default()
        }
    }

    async fn store(backend: &AdaptiveTtlBackend, key: &str, ttl: Option<Duration>) {
        backend
            .set(Arc::from(key), Arc::new(b"v".to_vec()), ttl)
            .await
            .unwrap();
    }

    /// 上限钳制：hot 系数 100× 远超 max_ttl，调整结果必须钳在 5s 上界
    #[tokio::test]
    async fn hot_extension_clamped_to_max_ttl() {
        let backend =
            AdaptiveTtlBackend::new(Arc::new(MokaMemoryBackend::builder().build()), hot_config())
                .unwrap();
        // 两次 set：首 set freq=1（普通，透传 60s），次 set freq=2 达
        // hot_threshold → 调整被钳在 5s 上界
        store(&backend, "hot-key", Some(Duration::from_secs(60))).await;
        store(&backend, "hot-key", Some(Duration::from_secs(60))).await;

        let ttl = backend.ttl("hot-key").await.unwrap().unwrap();
        assert!(
            ttl <= Duration::from_secs(5),
            "adjusted ttl must clamp to max_ttl=5s, got {ttl:?}"
        );
        assert_eq!(backend.stats().hot_extensions, 1, "one hot adjustment");
    }

    /// 最小值钳制：cold 除数 1000× 远低于 min_ttl，调整结果必须钳在 7s 下界
    #[tokio::test]
    async fn cold_shortening_clamped_to_min_ttl() {
        let backend = AdaptiveTtlBackend::new(
            Arc::new(MokaMemoryBackend::builder().build()),
            cold_config(),
        )
        .unwrap();
        store(&backend, "cold-key", Some(Duration::from_secs(60))).await;
        store(&backend, "cold-key", Some(Duration::from_secs(60))).await;

        let ttl = backend.ttl("cold-key").await.unwrap().unwrap();
        // 存入 7s，读出扣去 set→read 的亚毫秒流逝（容差 100ms）
        assert!(
            ttl >= Duration::from_millis(6_900),
            "adjusted ttl must clamp to min_ttl=7s, got {ttl:?}"
        );
        // cold_idle_after=ZERO：两次 set 之间必有微秒流逝，均判 cold
        assert_eq!(backend.stats().cold_shortenings, 2);
    }

    /// 统计清零：reset 后追踪表与延长/缩短计数归零，新周期从零开始
    #[tokio::test]
    async fn reset_stats_clears_everything() {
        let backend =
            AdaptiveTtlBackend::new(Arc::new(MokaMemoryBackend::builder().build()), hot_config())
                .unwrap();
        store(&backend, "k1", Some(Duration::from_secs(60))).await;
        store(&backend, "k1", Some(Duration::from_secs(60))).await;
        assert!(backend.stats().tracked_keys > 0);
        assert!(backend.stats().hot_extensions > 0);

        backend.reset_stats();

        let stats = backend.stats();
        assert_eq!(stats.tracked_keys, 0);
        assert_eq!(stats.hot_extensions, 0);
        assert_eq!(stats.cold_shortenings, 0);
        // reset 后同一键重新从普通键开始（freq=1 < hot_threshold → 透传）
        store(&backend, "k1", Some(Duration::from_secs(60))).await;
        let ttl = backend.ttl("k1").await.unwrap().unwrap();
        assert!(
            ttl > Duration::from_secs(50),
            "after reset the key must be treated as plain again: {ttl:?}"
        );
    }

    /// get 路径 hot 主动延长：命中 hot_threshold 后对已存条目 expire 延长，
    /// 且受 adjust_interval 限速（默认 60s 内第二次 get 不再延长）
    #[tokio::test]
    async fn get_hits_extend_hot_entry_ttl() {
        let config = AdaptiveTtlConfig {
            hot_threshold: 2,
            hot_ttl_multiplier: 10.0,
            adjust_interval: Duration::from_secs(3600),
            min_ttl: Duration::from_secs(1),
            max_ttl: Duration::from_secs(30),
            ..AdaptiveTtlConfig::default()
        };
        let backend =
            AdaptiveTtlBackend::new(Arc::new(MokaMemoryBackend::builder().build()), config)
                .unwrap();
        store(&backend, "get-hot", Some(Duration::from_secs(2))).await;
        // set 已计 freq=1

        // 首次 get：freq=2 达阈值 → expire 延长到 clamp(2s×10, 1s, 30s)=20s
        backend.get("get-hot").await.unwrap();
        assert_eq!(backend.stats().hot_extensions, 1);
        let ttl = backend.ttl("get-hot").await.unwrap().unwrap();
        assert!(
            ttl > Duration::from_secs(15),
            "extension must land near 20s, got {ttl:?}"
        );
        // 第二次 get：adjust_interval=3600s 限速 → 不重复延长
        backend.get("get-hot").await.unwrap();
        assert_eq!(backend.stats().hot_extensions, 1);
    }

    /// None TTL（永不过期）不参与调整：原样透传且不计数
    #[tokio::test]
    async fn none_ttl_passes_through_unadjusted() {
        let backend =
            AdaptiveTtlBackend::new(Arc::new(MokaMemoryBackend::builder().build()), hot_config())
                .unwrap();
        store(&backend, "eternal", None).await;
        store(&backend, "eternal", None).await;
        assert_eq!(backend.stats().hot_extensions, 0);
        assert_eq!(backend.ttl("eternal").await.unwrap(), None);
    }

    /// 内存上限：max_tracked_keys=1 时第二个键不被追踪（透传，不计数）
    #[tokio::test]
    async fn tracked_keys_cap_prevents_growth() {
        let config = AdaptiveTtlConfig {
            max_tracked_keys: 1,
            ..hot_config()
        };
        let backend =
            AdaptiveTtlBackend::new(Arc::new(MokaMemoryBackend::builder().build()), config)
                .unwrap();
        store(&backend, "tracked", Some(Duration::from_secs(60))).await;
        store(&backend, "untracked", Some(Duration::from_secs(60))).await;

        assert_eq!(backend.stats().tracked_keys, 1);
        // 未追踪键不做任何调整计数
        assert_eq!(backend.stats().hot_extensions, 0);
    }

    /// 未达阈值的普通键透传基准 TTL（不被策略触碰）
    #[tokio::test]
    async fn plain_key_passes_base_ttl_through() {
        let backend =
            AdaptiveTtlBackend::new(Arc::new(MokaMemoryBackend::builder().build()), hot_config())
                .unwrap();
        store(&backend, "plain", Some(Duration::from_secs(60))).await;
        let ttl = backend.ttl("plain").await.unwrap().unwrap();
        assert!(
            ttl > Duration::from_secs(50),
            "plain key must keep base ttl, got {ttl:?}"
        );
        assert_eq!(backend.stats().hot_extensions, 0);
    }

    /// min_ttl > max_ttl 必须在构造期显性拒绝（Err 而非请求路径 clamp panic）
    #[tokio::test]
    async fn rejects_min_ttl_above_max_ttl_at_construction() {
        let config = AdaptiveTtlConfig {
            min_ttl: Duration::from_secs(30),
            max_ttl: Duration::from_secs(1),
            ..hot_config()
        };
        let err =
            match AdaptiveTtlBackend::new(Arc::new(MokaMemoryBackend::builder().build()), config) {
                Err(e) => e,
                Ok(_) => panic!("min_ttl > max_ttl must be rejected at construction"),
            };
        assert!(
            matches!(
                &err,
                OxCacheError::InvalidInput(m) if m.contains("min_ttl") && m.contains("max_ttl")
            ),
            "rejection must name both bounds: {err:?}"
        );
    }

    /// 乘数非有限正数（0 / 负 / NaN / inf）与除数为 0 在配置校验期显性拒绝
    #[test]
    fn config_validate_rejects_degenerate_numeric_bounds() {
        for bad in [0.0, -2.0, f64::NAN, f64::INFINITY] {
            let config = AdaptiveTtlConfig {
                hot_ttl_multiplier: bad,
                ..hot_config()
            };
            let err = config
                .validate()
                .expect_err("non-finite/non-positive multiplier must be rejected");
            assert!(
                matches!(&err, OxCacheError::InvalidInput(m) if m.contains("hot_ttl_multiplier")),
                "multiplier {bad}: {err:?}"
            );
        }
        let config = AdaptiveTtlConfig {
            cold_ttl_divisor: 0,
            ..hot_config()
        };
        let err = config
            .validate()
            .expect_err("cold_ttl_divisor = 0 must be rejected");
        assert!(
            matches!(&err, OxCacheError::InvalidInput(m) if m.contains("cold_ttl_divisor")),
            "{err:?}"
        );
    }

    /// max_tracked_keys = 0 在配置校验期显性拒绝：追踪表容量下限为 1，
    /// 「静默关闭追踪、全部键退化透传」不是合法口径
    #[test]
    fn config_validate_rejects_zero_tracked_capacity() {
        let config = AdaptiveTtlConfig {
            max_tracked_keys: 0,
            ..hot_config()
        };
        let err = config
            .validate()
            .expect_err("max_tracked_keys = 0 must be rejected");
        assert!(
            matches!(&err, OxCacheError::InvalidInput(m) if m.contains("max_tracked_keys")),
            "{err:?}"
        );
    }

    /// get 主动延长遇后端 expire 故障：命中不被阻断，失败显性计数而非静默吞掉
    #[tokio::test]
    async fn expire_fault_keeps_hit_and_counts_failure() {
        let backend = AdaptiveTtlBackend::new(
            Arc::new(MockBackend::new("adaptive-ttl-fault", 50, false).with_fail_expire()),
            hot_config(),
        )
        .unwrap();
        store(&backend, "fault", Some(Duration::from_secs(60))).await;
        // set 已计 freq=1：首次 get 达 hot_threshold=2 → 触发 expire 调整
        // → 注入故障返回 Err
        let hit = backend.get("fault").await.unwrap();
        assert_eq!(
            hit.as_deref(),
            Some(b"v".as_slice()),
            "expire failure must not block the hit"
        );
        let stats = backend.stats();
        assert_eq!(
            stats.failed_adjustments, 1,
            "failure must be observable in stats"
        );
        assert_eq!(
            stats.hot_extensions, 0,
            "a failed adjustment must not count as an extension"
        );
    }

    /// get 路径钳制收敛：基准 TTL 高于 max_ttl 时，hot 键主动调整把 TTL
    /// 收敛到上界（方向不限，非仅延长）
    #[tokio::test]
    async fn get_hits_clamp_down_over_max_ttl() {
        let backend =
            AdaptiveTtlBackend::new(Arc::new(MokaMemoryBackend::builder().build()), hot_config())
                .unwrap();
        store(&backend, "over-max", Some(Duration::from_secs(60))).await;
        // freq=1（set 计数）；首次 get 达 hot_threshold=2 → 收敛到 max_ttl=5s
        backend.get("over-max").await.unwrap();
        let ttl = backend.ttl("over-max").await.unwrap().unwrap();
        assert!(
            ttl <= Duration::from_secs(5),
            "hot key ttl must clamp down to max_ttl, got {ttl:?}"
        );
    }
}
