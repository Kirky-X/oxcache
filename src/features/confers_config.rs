// Copyright (c) 2025-2026 Kirky.X
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
//! | `cache.circuit_breaker.failure_threshold` | u64 | 5 |
//! | `cache.circuit_breaker.recovery_timeout_ms` | u64 | 30000 |
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OxcacheConfig {
    /// L1 容量（条目数）
    pub capacity: u64,
    /// 默认 TTL（毫秒）
    pub default_ttl_ms: u64,
    /// 熔断参数
    pub circuit_breaker: CircuitBreakerSettings,
}

impl Default for OxcacheConfig {
    fn default() -> Self {
        Self {
            capacity: 10_000,
            default_ttl_ms: 60_000,
            circuit_breaker: CircuitBreakerSettings::default(),
        }
    }
}

impl OxcacheConfig {
    /// 从 confers [`ConfigConnector`](confers::ConfigConnector) 加载。
    ///
    /// key 缺失时回落默认值；值类型不匹配时忽略该 key（保守降级）。
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
        if let Some(v) = get_u64(
            connector,
            "cache.circuit_breaker.failure_threshold",
        )
        .await?
        {
            cfg.circuit_breaker.failure_threshold = u32::try_from(v).unwrap_or(u32::MAX);
        }
        if let Some(v) = get_u64(
            connector,
            "cache.circuit_breaker.recovery_timeout_ms",
        )
        .await?
        {
            cfg.circuit_breaker.recovery_timeout_ms = v;
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
    connector
        .get_raw(key)
        .await
        .map(|opt| opt.and_then(|v| v.as_u64()))
        .map_err(|e| OxCacheError::Operation(format!("confers read '{key}' failed: {e}")))
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
    pub fn watch<B, S>(
        self: &Arc<Self>,
        bus: Arc<B>,
        source: Arc<S>,
    ) -> tokio::task::JoinHandle<()>
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
                    Err(_) => {
                        // 重载失败：保留旧快照（配置快照永不空窗）
                        continue;
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use confers::{ConfigBus, ConfigChangeEvent, ConfigValue, InMemoryBus, SourceId};
    use confers::{ConfigWriter, new_in_memory};
    use std::sync::atomic::{AtomicU64, Ordering};

    async fn memory_connector_with_capacity(capacity: u64) -> impl confers::ConfigConnector {
        let conn = new_in_memory();
        conn.set(
            "cache.capacity",
            confers::AnnotatedValue::new(ConfigValue::U64(capacity), SourceId::default(), "cache.capacity"),
        )
        .await
        .unwrap();
        conn.set(
            "cache.default_ttl_ms",
            confers::AnnotatedValue::new(ConfigValue::U64(120_000), SourceId::default(), "cache.default_ttl_ms"),
        )
        .await
        .unwrap();
        conn
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
        assert_eq!(listener.calls.load(Ordering::SeqCst), 1, "监听器应被回调 1 次");
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
}
