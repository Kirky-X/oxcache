// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 配置驱动构建（`config-confers` feature）
//!
//! [`OxcacheConfig`] 经 confers 配置源加载，容量 / TTL / 熔断参数支持
//! confers [`ConfigBus`](confers::ConfigBus) watch **热更新**（快照原子换装）。
//! 层级合法：oxcache → confers（配置中枢，D3 端口方向）。
//!
//! # confers key 约定
//!
//! | key | 类型 | 默认值 |
//! | --- | --- | --- |
//! | `cache.capacity` | u64 | 10000 |
//! | `cache.default_ttl_ms` | u64 | 60000 |
//! | `cache.tti_ms` | u64 | 未设置 |
//! | `cache.null_cache_ttl_ms` | u64 | 未设置 |
//! | `cache.ttl_jitter_factor` | f64 | 未设置（底层默认 0.1） |
//! | `cache.sync_mode` | bool | 未设置 |
//! | `cache.backend` | string | 未设置（moka/dashmap/redis/valkey/dragonfly/aerospike/chain/mock/disk） |
//! | `cache.metrics_enabled` | bool | 未设置 |
//! | `cache.redis_url` | string | 未设置 |
//! | `cache.disk_path` | string | 未设置 |
//! | `cache.serialization_format` | string | 未设置（json/bincode/postcard） |
//! | `cache.connection_pool_size` | u64 | 未设置（Redis/Dragonfly 消费，底层缺省 8，0 被一致性检查拒绝） |
//! | `cache.circuit_breaker.failure_threshold` | u64 | 5 |
//! | `cache.circuit_breaker.recovery_timeout_ms` | u64 | 30000 |
//!
//! 扩展键经 [`CacheConfig::try_from_confers`] 映射到统一配置中枢；
//! `backend` / `serialization_format` 存原始字符串，映射时解析，
//! 无法识别的值显性报错（不做静默回落）。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::features::confers_config::{ConfersConfigWatcher, OxcacheConfig};
//!
//! let cfg = OxcacheConfig::load_from(&connector).await?;
//! let watcher = Arc::new(ConfersConfigWatcher::new(cfg));
//! watcher.watch(bus, source);   // 订阅 confers 变更，快照热更新
//! let current = watcher.snapshot().get();  // 读侧免锁（Arc 快照）
//! let l1 = crate::cache::L1Builder::from(current.as_ref()); // 参数即时生效
//! ```

use crate::error::{OxCacheError, OxCacheResult};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// confers key 前缀
pub const CACHE_KEY_PREFIX: &str = "cache";

/// 熔断参数（Redis 后端熔断器消费）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CircuitBreakerSettings {
    /// 连续失败阈值（达到后进入 Open）
    pub failure_threshold: u32,
    /// 半开恢复探测超时（毫秒）
    pub recovery_timeout_ms: u64,
}

impl Default for CircuitBreakerSettings {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout_ms: 30_000,
        }
    }
}

/// oxcache 配置（confers 驱动，serde 反序列化 + watch 热更新）
///
/// 基础字段（capacity / default_ttl_ms / circuit_breaker）缺省回落内置默认；
/// 扩展字段（Option 系列）`None` = 未配置，映射到统一配置中枢时保持未设置语义。
/// `backend` / `serialization_format` 存原始字符串，解析在
/// [`CacheConfig::try_from_confers`] 处显性完成。
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OxcacheConfig {
    /// L1 容量（条目数）
    pub capacity: u64,
    /// 默认 TTL（毫秒）
    pub default_ttl_ms: u64,
    /// 默认 TTI（毫秒；`None` = 未设置）
    pub tti_ms: Option<u64>,
    /// 穿透防护空值缓存 TTL（毫秒；`None` = 未设置）
    pub null_cache_ttl_ms: Option<u64>,
    /// TTL 抖动因子（`None` = 未设置，底层默认 0.1）
    pub ttl_jitter_factor: Option<f64>,
    /// 同步 API 模式（`None` = 未设置）
    pub sync_mode: Option<bool>,
    /// 后端类型原始串（moka/dashmap/redis/...；`None` = 未设置）
    pub backend: Option<String>,
    /// 指标开关（`None` = 未设置）
    pub metrics_enabled: Option<bool>,
    /// Redis 兼容后端连接串（`None` = 未设置）
    ///
    /// 可含 `user:password@` 凭证：[`Debug`] 实现脱敏为 `"***"`，
    /// 禁止以其他方式明文落日志。
    pub redis_url: Option<String>,
    /// 磁盘后端数据文件路径（`None` = 未设置）
    pub disk_path: Option<String>,
    /// 序列化格式原始串（json/bincode/postcard；`None` = 未设置）
    pub serialization_format: Option<String>,
    /// 连接池大小（Redis / Dragonfly 后端消费，缺省 8）
    pub connection_pool_size: Option<usize>,
    /// 熔断参数
    pub circuit_breaker: CircuitBreakerSettings,
}

// redis_url 可能携带凭证，手动实现 Debug 脱敏
impl std::fmt::Debug for OxcacheConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OxcacheConfig")
            .field("capacity", &self.capacity)
            .field("default_ttl_ms", &self.default_ttl_ms)
            .field("tti_ms", &self.tti_ms)
            .field("null_cache_ttl_ms", &self.null_cache_ttl_ms)
            .field("ttl_jitter_factor", &self.ttl_jitter_factor)
            .field("sync_mode", &self.sync_mode)
            .field("backend", &self.backend)
            .field("metrics_enabled", &self.metrics_enabled)
            .field(
                "redis_url",
                &self.redis_url.as_deref().map(|_| "***" as &str),
            )
            .field("disk_path", &self.disk_path)
            .field("serialization_format", &self.serialization_format)
            .field("connection_pool_size", &self.connection_pool_size)
            .field("circuit_breaker", &self.circuit_breaker)
            .finish()
    }
}

impl Default for OxcacheConfig {
    fn default() -> Self {
        Self {
            capacity: 10_000,
            default_ttl_ms: 60_000,
            tti_ms: None,
            null_cache_ttl_ms: None,
            ttl_jitter_factor: None,
            sync_mode: None,
            backend: None,
            metrics_enabled: None,
            redis_url: None,
            disk_path: None,
            serialization_format: None,
            connection_pool_size: None,
            circuit_breaker: CircuitBreakerSettings::default(),
        }
    }
}

impl OxcacheConfig {
    /// 从 confers [`ConfigConnector`](confers::ConfigConnector) 加载。
    ///
    /// key 缺失时回落默认值；值类型不匹配或数值超出目标类型范围时显性报错。
    pub async fn load_from<C>(connector: &C) -> OxCacheResult<Self>
    where
        C: confers::ConfigConnector,
    {
        let mut cfg = Self::default();

        if let Some(v) = get_u64(connector, "cache.capacity").await? {
            cfg.capacity = v;
        }
        if let Some(v) = get_u64(connector, "cache.default_ttl_ms").await? {
            cfg.default_ttl_ms = v;
        }
        if let Some(v) = get_u64(connector, "cache.circuit_breaker.failure_threshold").await? {
            cfg.circuit_breaker.failure_threshold = u32::try_from(v).map_err(|_| {
                out_of_range_u64("cache.circuit_breaker.failure_threshold", v, "u32")
            })?;
        }
        if let Some(v) = get_u64(connector, "cache.circuit_breaker.recovery_timeout_ms").await? {
            cfg.circuit_breaker.recovery_timeout_ms = v;
        }
        if let Some(v) = get_u64(connector, "cache.tti_ms").await? {
            cfg.tti_ms = Some(v);
        }
        if let Some(v) = get_u64(connector, "cache.null_cache_ttl_ms").await? {
            cfg.null_cache_ttl_ms = Some(v);
        }
        if let Some(v) = get_f64(connector, "cache.ttl_jitter_factor").await? {
            cfg.ttl_jitter_factor = Some(v);
        }
        if let Some(v) = get_bool(connector, "cache.sync_mode").await? {
            cfg.sync_mode = Some(v);
        }
        if let Some(v) = get_string(connector, "cache.backend").await? {
            cfg.backend = Some(v);
        }
        if let Some(v) = get_bool(connector, "cache.metrics_enabled").await? {
            cfg.metrics_enabled = Some(v);
        }
        if let Some(v) = get_string(connector, "cache.redis_url").await? {
            cfg.redis_url = Some(v);
        }
        if let Some(v) = get_string(connector, "cache.disk_path").await? {
            cfg.disk_path = Some(v);
        }
        if let Some(v) = get_u64(connector, "cache.connection_pool_size").await? {
            cfg.connection_pool_size = Some(
                usize::try_from(v)
                    .map_err(|_| out_of_range_u64("cache.connection_pool_size", v, "usize"))?,
            );
        }
        if let Some(v) = get_string(connector, "cache.serialization_format").await? {
            cfg.serialization_format = Some(v);
        }
        Ok(cfg)
    }

    /// 默认 TTL 时长
    pub fn default_ttl(&self) -> Duration {
        Duration::from_millis(self.default_ttl_ms)
    }
}

/// 配置驱动 L1 构建：容量 / TTL 即时来自快照
impl From<&OxcacheConfig> for crate::cache::L1Builder {
    fn from(config: &OxcacheConfig) -> Self {
        crate::cache::L1Builder::new()
            .capacity(config.capacity)
            .ttl(config.default_ttl())
    }
}

async fn get_u64<C>(connector: &C, key: &str) -> OxCacheResult<Option<u64>>
where
    C: confers::ConfigConnector,
{
    match connector
        .get_raw(key)
        .await
        .map_err(|e| OxCacheError::Operation(format!("confers read '{key}' failed: {e}")))?
    {
        None => Ok(None),
        // 键存在但类型不符：显性报错，与模块「不做静默回落」承诺一致
        Some(v) => v.as_u64().map(Some).ok_or_else(|| {
            OxCacheError::InvalidInput(format!("confers key '{key}' expects u64, got {v:?}"))
        }),
    }
}

/// u64 值超出目标整型范围时的显性错误（避免静默钳位/回落）
///
/// `target` 携带真实目标类型（如 u32 / usize）：u64→u32 溢出与平台指针宽度
/// 无关，措辞不预设「窄平台」，防止误导排障方向。
fn out_of_range_u64(key: &str, value: u64, target: &str) -> OxCacheError {
    OxCacheError::InvalidInput(format!(
        "confers key '{key}' value {value} exceeds the {target} range"
    ))
}

async fn get_bool<C>(connector: &C, key: &str) -> OxCacheResult<Option<bool>>
where
    C: confers::ConfigConnector,
{
    match connector
        .get_raw(key)
        .await
        .map_err(|e| OxCacheError::Operation(format!("confers read '{key}' failed: {e}")))?
    {
        None => Ok(None),
        Some(v) => v.as_bool().map(Some).ok_or_else(|| {
            OxCacheError::InvalidInput(format!("confers key '{key}' expects bool, got {v:?}"))
        }),
    }
}

async fn get_f64<C>(connector: &C, key: &str) -> OxCacheResult<Option<f64>>
where
    C: confers::ConfigConnector,
{
    match connector
        .get_raw(key)
        .await
        .map_err(|e| OxCacheError::Operation(format!("confers read '{key}' failed: {e}")))?
    {
        None => Ok(None),
        Some(v) => v.as_f64().map(Some).ok_or_else(|| {
            OxCacheError::InvalidInput(format!("confers key '{key}' expects f64, got {v:?}"))
        }),
    }
}

async fn get_string<C>(connector: &C, key: &str) -> OxCacheResult<Option<String>>
where
    C: confers::ConfigConnector,
{
    match connector
        .get_raw(key)
        .await
        .map_err(|e| OxCacheError::Operation(format!("confers read '{key}' failed: {e}")))?
    {
        None => Ok(None),
        Some(v) => v.as_str().map(|s| Some(s.to_string())).ok_or_else(|| {
            OxCacheError::InvalidInput(format!("confers key '{key}' expects string, got {v:?}"))
        }),
    }
}

/// 统一配置中枢映射：confers 快照 → [`CacheConfig`]
///
/// 纯字段映射（`backend` / `serialization_format` 保留原始串）；
/// 字符串合法性、参数组合与 feature 可用性统一由
/// [`CacheConfig::validate()`] 在应用前显性检查，映射本身不产生解析错误。
impl crate::config::CacheConfig {
    /// 从 confers [`OxcacheConfig`] 快照映射统一配置
    pub fn try_from_confers(config: &OxcacheConfig) -> OxCacheResult<Self> {
        Ok(Self {
            capacity: Some(config.capacity),
            ttl: Some(config.default_ttl()),
            tti: config.tti_ms.map(Duration::from_millis),
            null_cache_ttl: config.null_cache_ttl_ms.map(Duration::from_millis),
            ttl_jitter_factor: config.ttl_jitter_factor,
            sync_mode: config.sync_mode,
            backend: config.backend.clone(),
            metrics_enabled: config.metrics_enabled,
            serialization_format: config.serialization_format.clone(),
            redis_url: config.redis_url.clone(),
            disk_path: config.disk_path.clone(),
            connection_pool_size: config.connection_pool_size,
            circuit_breaker_failure_threshold: Some(config.circuit_breaker.failure_threshold),
            circuit_breaker_reset_timeout: Some(Duration::from_millis(
                config.circuit_breaker.recovery_timeout_ms,
            )),
        })
    }
}

/// 配置来源端口（与 confers 解耦的可测试抽象）
#[async_trait::async_trait]
pub trait CacheConfigSource: Send + Sync {
    /// 加载当前完整配置
    async fn load(&self) -> OxCacheResult<OxcacheConfig>;
}

/// confers 连接器适配器（实现 [`CacheConfigSource`]）
pub struct ConfersConfigSource<C> {
    connector: Arc<C>,
}

impl<C> ConfersConfigSource<C> {
    pub fn new(connector: Arc<C>) -> Self {
        Self { connector }
    }
}

#[async_trait::async_trait]
impl<C> CacheConfigSource for ConfersConfigSource<C>
where
    C: confers::ConfigConnector + 'static,
{
    async fn load(&self) -> OxCacheResult<OxcacheConfig> {
        OxcacheConfig::load_from(self.connector.as_ref()).await
    }
}

/// 配置快照（读侧免锁：`get()` 返回 `Arc` 克隆）
#[derive(Clone)]
pub struct ConfigSnapshot {
    inner: Arc<RwLock<Arc<OxcacheConfig>>>,
}

impl ConfigSnapshot {
    pub fn new(config: OxcacheConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Arc::new(config))),
        }
    }

    /// 当前配置快照（读侧零克隆语义：仅 Arc 引用计数 +1）
    pub fn get(&self) -> Arc<OxcacheConfig> {
        self.inner
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| Arc::new(OxcacheConfig::default()))
    }

    /// 原子换装，返回旧快照
    fn swap(&self, config: OxcacheConfig) -> Arc<OxcacheConfig> {
        match self.inner.write() {
            Ok(mut guard) => std::mem::replace(&mut guard, Arc::new(config)),
            Err(_) => Arc::new(OxcacheConfig::default()),
        }
    }
}

/// 热更新监听器（容量 / TTL / 熔断参数变化回调）
pub trait ConfigChangeListener: Send + Sync {
    fn on_change(&self, old: &OxcacheConfig, new: &OxcacheConfig);
}

/// confers watch 热更新管理器
///
/// 订阅 confers [`ConfigBus`](confers::ConfigBus)，收到 `cache.*` 变更事件后
/// 从 [`CacheConfigSource`] 重载完整配置并原子换装快照、通知监听器。
pub struct ConfersConfigWatcher {
    snapshot: ConfigSnapshot,
    listeners: Vec<Arc<dyn ConfigChangeListener>>,
}

impl ConfersConfigWatcher {
    /// 以初始配置创建 watcher
    pub fn new(initial: OxcacheConfig) -> Self {
        Self {
            snapshot: ConfigSnapshot::new(initial),
            listeners: Vec::new(),
        }
    }

    /// 注册变更监听器
    pub fn on_change(mut self, listener: Arc<dyn ConfigChangeListener>) -> Self {
        self.listeners.push(listener);
        self
    }

    /// 快照句柄（可克隆给任意读方）
    pub fn snapshot(&self) -> ConfigSnapshot {
        self.snapshot.clone()
    }

    /// 订阅 confers 总线并热更新。
    ///
    /// 事件含任何 `cache.*` 变更 key（或 key 列表为空的全量刷新）时重载。
    /// 返回后台任务句柄。
    pub fn watch<B, S>(self: &Arc<Self>, bus: Arc<B>, source: Arc<S>) -> tokio::task::JoinHandle<()>
    where
        B: confers::ConfigBus + 'static,
        S: CacheConfigSource + 'static,
    {
        let watcher = Arc::clone(self);
        tokio::spawn(async move {
            let Ok(mut stream) = bus.subscribe().await else {
                return;
            };
            use futures::stream::StreamExt;
            while let Some(event) = stream.next().await {
                let relevant = event.changed_keys.is_empty()
                    || event
                        .changed_keys
                        .iter()
                        .any(|k| k.starts_with(CACHE_KEY_PREFIX));
                if !relevant {
                    continue;
                }
                match source.load().await {
                    Ok(new_cfg) => {
                        let old_arc = watcher.snapshot.swap(new_cfg);
                        let new_arc = watcher.snapshot.get();
                        for listener in &watcher.listeners {
                            listener.on_change(&old_arc, &new_arc);
                        }
                    }
                    Err(err) => {
                        // 重载失败：保留旧快照（配置快照永不空窗），
                        // 拒绝原因必须可观测，禁止静默丢弃坏配置
                        oxcache_report_config_reload_rejected(&err);
                        continue;
                    }
                }
            }
        })
    }
}

// ---- 重载拒绝上报（计数器为默认信号，tracing warn 为 telemetry 增强信号）----

/// 拒绝计数（`metrics` feature：unified 动态计数器；metrics 在全部预设组合中在场，
/// 保证默认/全量构建下重载被拒可观测，不再仅依赖不在任何预设中的 telemetry feature）
#[cfg(feature = "metrics")]
#[inline]
fn record_reload_rejected_counter() {
    crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS
        .increment_counter("oxcache_config_reload_rejected_total", 1);
}

#[cfg(not(feature = "metrics"))]
#[inline]
fn record_reload_rejected_counter() {}

#[cfg(feature = "telemetry")]
#[inline]
fn warn_reload_rejected(err: &OxCacheError) {
    tracing::warn!(
        target = "oxcache::confers_config",
        %err,
        "confers config reload rejected; keeping previous snapshot"
    );
}

#[cfg(not(feature = "telemetry"))]
#[inline]
fn warn_reload_rejected(_err: &OxCacheError) {}

/// 热更新重载被拒的统一上报：计数器 + warn（各自 feature 门控，关闭时零开销）
#[inline]
fn oxcache_report_config_reload_rejected(err: &OxCacheError) {
    record_reload_rejected_counter();
    warn_reload_rejected(err);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CacheConfig;
    use confers::{ConfigBus, ConfigChangeEvent, ConfigValue, InMemoryBus, SourceId};
    use confers::{ConfigWriter, new_in_memory};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::Duration;

    /// 读取 unified 动态计数器快照（缺失视为 0），用于重载拒绝的 delta 断言
    #[cfg(feature = "metrics")]
    fn dynamic_counter(metrics: &crate::infra::metrics::unified::UnifiedMetrics, key: &str) -> u64 {
        use crate::infra::metrics::unified::MetricValue;
        metrics
            .get_dynamic_metrics()
            .get(key)
            .and_then(|v| match v {
                MetricValue::Counter(c) => Some(*c),
                _ => None,
            })
            .unwrap_or(0)
    }

    async fn memory_connector_with_capacity(capacity: u64) -> impl confers::ConfigConnector {
        let conn = new_in_memory();
        conn.set(
            "cache.capacity",
            confers::AnnotatedValue::new(
                ConfigValue::U64(capacity),
                SourceId::default(),
                "cache.capacity",
            ),
        )
        .await
        .unwrap();
        conn.set(
            "cache.default_ttl_ms",
            confers::AnnotatedValue::new(
                ConfigValue::U64(120_000),
                SourceId::default(),
                "cache.default_ttl_ms",
            ),
        )
        .await
        .unwrap();
        conn
    }

    #[tokio::test]
    async fn load_from_extended_keys() {
        let conn = new_in_memory();
        let entries: &[(&str, ConfigValue)] = &[
            ("cache.capacity", ConfigValue::U64(2048)),
            ("cache.default_ttl_ms", ConfigValue::U64(90_000)),
            ("cache.tti_ms", ConfigValue::U64(15_000)),
            ("cache.null_cache_ttl_ms", ConfigValue::U64(500)),
            ("cache.ttl_jitter_factor", ConfigValue::F64(0.3)),
            ("cache.sync_mode", ConfigValue::Bool(true)),
            ("cache.backend", ConfigValue::String("redis".into())),
            ("cache.metrics_enabled", ConfigValue::Bool(false)),
            (
                "cache.redis_url",
                ConfigValue::String("redis://cfg:6379".into()),
            ),
            (
                "cache.serialization_format",
                ConfigValue::String("postcard".into()),
            ),
            ("cache.connection_pool_size", ConfigValue::U64(16)),
            (
                "cache.circuit_breaker.failure_threshold",
                ConfigValue::U64(9),
            ),
            (
                "cache.circuit_breaker.recovery_timeout_ms",
                ConfigValue::U64(45_000),
            ),
        ];
        for (key, value) in entries {
            conn.set(
                key,
                confers::AnnotatedValue::new(value.clone(), SourceId::default(), *key),
            )
            .await
            .unwrap();
        }
        let cfg = OxcacheConfig::load_from(&conn).await.unwrap();
        assert_eq!(cfg.capacity, 2048);
        assert_eq!(cfg.default_ttl_ms, 90_000);
        assert_eq!(cfg.tti_ms, Some(15_000));
        assert_eq!(cfg.null_cache_ttl_ms, Some(500));
        assert_eq!(cfg.ttl_jitter_factor, Some(0.3));
        assert_eq!(cfg.sync_mode, Some(true));
        assert_eq!(cfg.backend.as_deref(), Some("redis"));
        assert_eq!(cfg.metrics_enabled, Some(false));
        assert_eq!(cfg.redis_url.as_deref(), Some("redis://cfg:6379"));
        assert_eq!(cfg.serialization_format.as_deref(), Some("postcard"));
        assert_eq!(cfg.connection_pool_size, Some(16));
        assert_eq!(cfg.circuit_breaker.failure_threshold, 9);
        assert_eq!(cfg.circuit_breaker.recovery_timeout_ms, 45_000);
    }

    #[tokio::test]
    async fn load_from_type_mismatch_fails_loudly() {
        let conn = new_in_memory();
        let key = "cache.sync_mode";
        conn.set(
            key,
            confers::AnnotatedValue::new(ConfigValue::U64(1), SourceId::default(), key),
        )
        .await
        .unwrap();
        let err = OxcacheConfig::load_from(&conn).await.unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains(key)));
    }

    #[tokio::test]
    async fn load_from_threshold_overflow_fails_loudly() {
        let conn = new_in_memory();
        let key = "cache.circuit_breaker.failure_threshold";
        conn.set(
            key,
            confers::AnnotatedValue::new(
                ConfigValue::U64(u32::MAX as u64 + 1),
                SourceId::default(),
                key,
            ),
        )
        .await
        .unwrap();
        let err = OxcacheConfig::load_from(&conn).await.unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains(key)));
    }

    // 连接池超界仅在窄指针宽度平台可达（64 位上 u64 与 usize 等宽），测试随平台门控
    #[cfg(target_pointer_width = "32")]
    #[tokio::test]
    async fn load_from_pool_size_overflow_fails_loudly() {
        let conn = new_in_memory();
        let key = "cache.connection_pool_size";
        conn.set(
            key,
            confers::AnnotatedValue::new(
                ConfigValue::U64(usize::MAX as u64 + 1),
                SourceId::default(),
                key,
            ),
        )
        .await
        .unwrap();
        let err = OxcacheConfig::load_from(&conn).await.unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains(key)));
    }

    #[test]
    fn debug_redacts_redis_url() {
        let cfg = OxcacheConfig {
            redis_url: Some("redis://admin:s3cret@host:6379".into()),
            ..OxcacheConfig::default()
        };
        let debug = format!("{cfg:?}");
        assert!(!debug.contains("s3cret"), "credentials leaked: {debug}");
        assert!(
            debug.contains("\"***\""),
            "redaction marker missing: {debug}"
        );
    }

    #[tokio::test]
    async fn try_from_confers_maps_all_fields() {
        let cfg = OxcacheConfig {
            capacity: 4096,
            default_ttl_ms: 60_000,
            tti_ms: Some(12_000),
            null_cache_ttl_ms: Some(400),
            ttl_jitter_factor: Some(0.15),
            sync_mode: Some(true),
            backend: Some("moka".into()),
            metrics_enabled: Some(false),
            redis_url: None,
            disk_path: Some("/tmp/cfg.redb".into()),
            serialization_format: Some("bincode".into()),
            connection_pool_size: Some(16),
            circuit_breaker: CircuitBreakerSettings {
                failure_threshold: 7,
                recovery_timeout_ms: 25_000,
            },
        };
        let unified = CacheConfig::try_from_confers(&cfg).unwrap();
        assert_eq!(unified.capacity, Some(4096));
        assert_eq!(unified.ttl, Some(Duration::from_millis(60_000)));
        assert_eq!(unified.tti, Some(Duration::from_millis(12_000)));
        assert_eq!(unified.null_cache_ttl, Some(Duration::from_millis(400)));
        assert_eq!(unified.ttl_jitter_factor, Some(0.15));
        assert_eq!(unified.sync_mode, Some(true));
        assert_eq!(unified.backend, Some("moka".to_string()));
        assert_eq!(unified.metrics_enabled, Some(false));
        assert_eq!(unified.disk_path.as_deref(), Some("/tmp/cfg.redb"));
        assert_eq!(unified.connection_pool_size, Some(16));
        // 字段存原始串，解析在 validate/apply 处显性完成
        assert_eq!(unified.serialization_format.as_deref(), Some("bincode"));
        assert_eq!(unified.circuit_breaker_failure_threshold, Some(7));
        assert_eq!(
            unified.circuit_breaker_reset_timeout,
            Some(Duration::from_millis(25_000))
        );
    }

    #[tokio::test]
    async fn try_from_confers_unknown_backend_fails_loudly() {
        let cfg = OxcacheConfig {
            backend: Some("memcache".into()),
            ..OxcacheConfig::default()
        };
        // 映射保留原始串；一致性检查在 validate 处显性拒绝
        let unified = CacheConfig::try_from_confers(&cfg).unwrap();
        let err = unified.validate().unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("memcache")));
    }

    #[tokio::test]
    async fn try_from_confers_defaults_map_to_unset() {
        let unified = CacheConfig::try_from_confers(&OxcacheConfig::default()).unwrap();
        // capacity/ttl 来自默认值，其余字段保持未配置（None）
        assert_eq!(unified.capacity, Some(10_000));
        assert_eq!(unified.ttl, Some(Duration::from_millis(60_000)));
        assert_eq!(unified.backend, None);
        assert_eq!(unified.sync_mode, None);
        assert_eq!(unified.serialization_format, None);
    }

    #[tokio::test]
    async fn load_from_confers_connector() {
        let conn = memory_connector_with_capacity(1234).await;
        let cfg = OxcacheConfig::load_from(&conn).await.unwrap();
        assert_eq!(cfg.capacity, 1234);
        assert_eq!(cfg.default_ttl_ms, 120_000);
        // 未设置的 key 回落默认
        assert_eq!(cfg.circuit_breaker.failure_threshold, 5);
    }

    #[tokio::test]
    async fn load_from_empty_connector_uses_defaults() {
        let conn = new_in_memory();
        let cfg = OxcacheConfig::load_from(&conn).await.unwrap();
        assert_eq!(cfg, OxcacheConfig::default());
    }

    #[test]
    fn config_serde_roundtrip() {
        let cfg = OxcacheConfig {
            capacity: 500,
            default_ttl_ms: 30_000,
            circuit_breaker: CircuitBreakerSettings {
                failure_threshold: 3,
                recovery_timeout_ms: 10_000,
            },
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: OxcacheConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);

        // 部分字段反序列化回落默认值
        let partial: OxcacheConfig = serde_json::from_str(r#"{"capacity": 42}"#).unwrap();
        assert_eq!(partial.capacity, 42);
        assert_eq!(partial.default_ttl_ms, 60_000);
    }

    #[cfg(feature = "memory")]
    #[tokio::test]
    async fn config_drives_l1_builder() {
        let cfg = OxcacheConfig {
            capacity: 777,
            ..Default::default()
        };
        let l1 = crate::cache::L1Builder::from(&cfg);
        let backend = l1.build();
        assert_eq!(backend.capacity().await.unwrap(), 777);
    }

    struct RecordingListener {
        calls: AtomicU64,
        last_capacity: AtomicU64,
    }

    impl RecordingListener {
        fn new() -> Self {
            Self {
                calls: AtomicU64::new(0),
                last_capacity: AtomicU64::new(0),
            }
        }
    }

    impl ConfigChangeListener for RecordingListener {
        fn on_change(&self, _old: &OxcacheConfig, new: &OxcacheConfig) {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.last_capacity.store(new.capacity, Ordering::SeqCst);
        }
    }

    /// 热更新：confers 总线事件 → 重载 → 快照换装 + 监听器回调
    #[tokio::test]
    async fn watch_hot_reloads_on_confers_event() {
        let connector = Arc::new(memory_connector_with_capacity(100).await);

        // 修改 confers 值（模拟 watch 检测到的配置变更）
        connector
            .set(
                "cache.capacity",
                confers::AnnotatedValue::new(
                    ConfigValue::U64(9999),
                    SourceId::default(),
                    "cache.capacity",
                ),
            )
            .await
            .unwrap();

        let bus = Arc::new(InMemoryBus::new());
        let source = Arc::new(ConfersConfigSource::new(connector));

        let initial = source.load().await.unwrap();
        assert_eq!(initial.capacity, 9999, "重载前应读到新值");

        let listener = Arc::new(RecordingListener::new());
        let watcher = Arc::new(
            ConfersConfigWatcher::new(OxcacheConfig {
                capacity: 100,
                ..Default::default()
            })
            .on_change(listener.clone()),
        );
        assert_eq!(watcher.snapshot().get().capacity, 100);

        // 订阅总线
        let handle = watcher.watch(bus.clone(), source.clone());
        tokio::time::sleep(Duration::from_millis(50)).await;

        // 发布 cache.* 变更事件
        bus.publish(ConfigChangeEvent::new(
            "test",
            "unit-test",
            vec!["cache.capacity".to_string()],
            "",
        ))
        .await
        .unwrap();

        for _ in 0..50 {
            if watcher.snapshot().get().capacity == 9999 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert_eq!(
            watcher.snapshot().get().capacity,
            9999,
            "热更新后快照应换装"
        );
        assert_eq!(
            listener.calls.load(Ordering::SeqCst),
            1,
            "监听器应被回调 1 次"
        );
        assert_eq!(listener.last_capacity.load(Ordering::SeqCst), 9999);

        handle.abort();
    }

    /// 非 cache.* 前缀的事件不触发重载
    #[tokio::test]
    async fn watch_ignores_non_cache_events() {
        let connector = Arc::new(memory_connector_with_capacity(100).await);
        let bus = Arc::new(InMemoryBus::new());
        let source = Arc::new(ConfersConfigSource::new(connector));

        let watcher = Arc::new(ConfersConfigWatcher::new(OxcacheConfig {
            capacity: 100,
            ..Default::default()
        }));
        let handle = watcher.watch(bus.clone(), source);
        tokio::time::sleep(Duration::from_millis(50)).await;

        bus.publish(ConfigChangeEvent::new(
            "test",
            "unit-test",
            vec!["other.key".to_string()],
            "",
        ))
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(watcher.snapshot().get().capacity, 100, "无关事件不应换装");
        handle.abort();
    }

    /// 重载失败：保留旧快照且 watch 任务存活；metrics 下拒绝计数器增量即确定性
    /// 拒绝信号（无 metrics 构建回退为 sleep 等待后断言快照语义）
    #[tokio::test]
    // delta 断言依赖全局计数器窗口期不被清零，须与重置全局指标的串行测试互斥
    #[serial_test::serial]
    async fn watch_keeps_snapshot_when_reload_fails() {
        struct FlakySource {
            fail: AtomicBool,
        }
        #[async_trait::async_trait]
        impl CacheConfigSource for FlakySource {
            async fn load(&self) -> OxCacheResult<OxcacheConfig> {
                if self.fail.load(Ordering::SeqCst) {
                    Err(OxCacheError::Operation("injected reload failure".into()))
                } else {
                    Ok(OxcacheConfig {
                        capacity: 4321,
                        ..OxcacheConfig::default()
                    })
                }
            }
        }

        let source = Arc::new(FlakySource {
            fail: AtomicBool::new(false),
        });
        let bus = Arc::new(InMemoryBus::new());
        let watcher = Arc::new(ConfersConfigWatcher::new(OxcacheConfig {
            capacity: 100,
            ..OxcacheConfig::default()
        }));
        let handle = watcher.watch(bus.clone(), source.clone());
        tokio::time::sleep(Duration::from_millis(50)).await;

        // 先成功换装
        bus.publish(ConfigChangeEvent::new(
            "test",
            "unit-test",
            vec!["cache.capacity".to_string()],
            "",
        ))
        .await
        .unwrap();
        for _ in 0..50 {
            if watcher.snapshot().get().capacity == 4321 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(watcher.snapshot().get().capacity, 4321);

        // 翻转为失败后发布变更：快照保持旧值，任务不得退出
        #[cfg(feature = "metrics")]
        let rejected_before = dynamic_counter(
            &crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS,
            "oxcache_config_reload_rejected_total",
        );
        source.fail.store(true, Ordering::SeqCst);
        bus.publish(ConfigChangeEvent::new(
            "test",
            "unit-test",
            vec!["cache.capacity".to_string()],
            "",
        ))
        .await
        .unwrap();
        // metrics 下以拒绝计数器增量确定性等待信号落地（替代盲等固定毫秒）；
        // 无 metrics 构建无该信号可测，回退 sleep 后仅断言快照语义
        #[cfg(feature = "metrics")]
        {
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while dynamic_counter(
                &crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS,
                "oxcache_config_reload_rejected_total",
            ) <= rejected_before
                && std::time::Instant::now() < deadline
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert!(
                dynamic_counter(
                    &crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS,
                    "oxcache_config_reload_rejected_total",
                ) > rejected_before,
                "reload rejection must increment oxcache_config_reload_rejected_total"
            );
        }
        #[cfg(not(feature = "metrics"))]
        tokio::time::sleep(Duration::from_millis(150)).await;

        assert_eq!(
            watcher.snapshot().get().capacity,
            4321,
            "重载失败应保留旧快照"
        );
        assert!(!handle.is_finished(), "watch 任务在重载失败后必须存活");
        handle.abort();
    }
}
