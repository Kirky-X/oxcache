// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! SWR 三态过期装饰器（absorb-hitbox-features，对标 hitbox stale 策略）
//!
//! 双时间戳 envelope：`expire_at`（逻辑过期）与 `stale_at`（stale 窗口终点）。
//! 读取三态：
//!
//! - `now < expire_at` → **Actual**（新鲜，普通 hit）
//! - `expire_at ≤ now < stale_at` → **Stale**（返回旧值；发布
//!   `CacheEventType::Expire` 事件 + `oxcache_stale_hits_total` 计数）
//! - `now ≥ stale_at` → **Expired**（物理删除，视同 miss）
//!
//! 传给 inner 的物理 TTL = `ttl + stale_ttl`（Redis 等原生 TTL 后端不会在
//! stale 窗口内驱逐）。无 TTL 条目原样透传（无 stale 语义）。非 envelope
//! 数据（装饰器启用前的旧值 / 旁路直写值）按 Actual 透传，零迁移成本。
//!
//! # 组合顺序
//!
//! 与 `CompressingBackend` 叠加时两种顺序均正确：两装饰器各自以 magic 前缀
//! 识别自有格式，互不干扰。推荐 stale 在外层（压缩作用于 payload，envelope
//! 头保持明文定长）。

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::backend::{
    BackendKind, CacheBackend, CacheConnector, CacheReader, CacheSetItem, CacheWriter,
};
use crate::core::events::{CacheEvent, CacheEventType};
use crate::error::OxCacheResult;

/// envelope 魔数（`OWRS`），用于区分自有格式与旁路直写数据
const STALE_MAGIC: u32 = 0x4F_57_52_53;
const ENV_HEADER: usize = 20; // magic(4) + expire_at i64(8) + stale_at i64(8)

/// 三态判定结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleState {
    /// 新鲜（`now < expire_at`）
    Fresh,
    /// 陈旧（`expire_at ≤ now < stale_at`，旧值仍可服务）
    Stale,
    /// 已彻底过期（`now ≥ stale_at`）
    Expired,
    /// 非 envelope 数据（旁路直写 / 无 TTL 透传），按新鲜处理
    Opaque,
}

/// Stale 命中策略。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StalePolicy {
    /// 返回旧值（默认；裸 `get` 一律此语义）
    #[default]
    Return,
    /// 视同 miss，经 single-flight 同步回源刷新
    Revalidate,
    /// 立即返回旧值，`OffloadManager` 后台刷新（需经
    /// `Cache::get_or_refresh` 触发后台任务；`get_or` 下按 Return 处理）
    OffloadRevalidate,
}

/// envelope 编码：magic + expire_at_ms + stale_at_ms + payload
fn encode_envelope(expires_at_ms: u64, stale_at_ms: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + ENV_HEADER);
    out.extend_from_slice(&STALE_MAGIC.to_le_bytes());
    out.extend_from_slice(&(expires_at_ms as i64).to_le_bytes());
    out.extend_from_slice(&(stale_at_ms as i64).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// envelope 解码；非本格式数据返回 None（Opaque）
fn decode_envelope(bytes: &[u8]) -> Option<(u64, u64, &[u8])> {
    if bytes.len() < ENV_HEADER {
        return None;
    }
    let magic = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
    if magic != STALE_MAGIC {
        return None;
    }
    let expire_at = i64::from_le_bytes(bytes[4..12].try_into().ok()?);
    let stale_at = i64::from_le_bytes(bytes[12..20].try_into().ok()?);
    if expire_at < 0 || stale_at < 0 {
        return None;
    }
    Some((expire_at as u64, stale_at as u64, &bytes[ENV_HEADER..]))
}

fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// SWR 三态过期装饰器：包装任意 `CacheBackend`。
#[derive(Clone)]
pub struct StaleWhileRevalidateBackend {
    inner: Arc<dyn CacheBackend>,
    stale_ttl: Duration,
    policy: StalePolicy,
    event_publisher: Option<Arc<dyn crate::core::events::EventPublisher>>,
}

impl StaleWhileRevalidateBackend {
    /// 包装 inner 后端，stale 窗口长度 `stale_ttl`。
    pub fn new(inner: Arc<dyn CacheBackend>, stale_ttl: Duration) -> Self {
        Self {
            inner,
            stale_ttl,
            policy: StalePolicy::default(),
            event_publisher: None,
        }
    }

    /// 设置策略（装饰器侧只保存默认策略供读取；Cache 侧策略以
    /// `stale_policy` 字段为准）。
    pub fn with_policy(mut self, policy: StalePolicy) -> Self {
        self.policy = policy;
        self
    }

    /// 注入事件发布器（Stale 命中时发布 `CacheEventType::Expire`）。
    pub fn with_event_publisher(
        mut self,
        publisher: Arc<dyn crate::core::events::EventPublisher>,
    ) -> Self {
        self.event_publisher = Some(publisher);
        self
    }

    /// stale 窗口长度。
    pub fn stale_ttl(&self) -> Duration {
        self.stale_ttl
    }

    async fn emit_stale_event(&self, key: &str) {
        #[cfg(feature = "metrics")]
        crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS
            .increment_counter("oxcache_stale_hits_total", 1);
        if let Some(publisher) = &self.event_publisher {
            let event = CacheEvent::new(CacheEventType::Expire)
                .with_key(key.to_string())
                .with_metadata("state", "stale");
            // 事件失败不影响读路径
            let _ = publisher.publish(event).await;
        }
        telemetry_stale_hit(key);
    }

    /// 读取并判定三态，返回 (payload 字节, 状态)。
    pub async fn get_with_state(&self, key: &str) -> OxCacheResult<(Option<Vec<u8>>, StaleState)> {
        let Some(raw) = self.inner.get(key).await? else {
            return Ok((None, StaleState::Expired));
        };
        let Some((expire_at, stale_at, payload)) = decode_envelope(&raw) else {
            return Ok((Some(raw.to_vec()), StaleState::Opaque));
        };
        let now = now_epoch_ms();
        if now < expire_at {
            Ok((Some(payload.to_vec()), StaleState::Fresh))
        } else if now < stale_at {
            self.emit_stale_event(key).await;
            Ok((Some(payload.to_vec()), StaleState::Stale))
        } else {
            // Expired：物理删除（委托 inner），视同 miss
            let _ = self.inner.delete(key).await;
            Ok((None, StaleState::Expired))
        }
    }
}

// telemetry 双版本 inline 埋点
#[cfg(feature = "telemetry")]
#[inline]
fn telemetry_stale_hit(key: &str) {
    tracing::debug!(target: "oxcache::stale", key, "stale hit served");
}

#[cfg(not(feature = "telemetry"))]
#[inline]
fn telemetry_stale_hit(_key: &str) {}

#[async_trait]
impl CacheReader for StaleWhileRevalidateBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        // Return 语义的透明读：Fresh/Opaque/Stale 均返回 payload
        Ok(self.get_with_state(key).await?.0)
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        match self.get_with_state(key).await? {
            (_, StaleState::Fresh) | (_, StaleState::Stale) | (_, StaleState::Opaque) => Ok(true),
            (Some(_), _) => Ok(true),
            (None, _) => Ok(false),
        }
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        let Some(raw) = self.inner.get(key).await? else {
            return Ok(None);
        };
        let Some((expire_at, _, _)) = decode_envelope(&raw) else {
            // 非 envelope：透传 inner 的 TTL 语义
            return self.inner.ttl(key).await;
        };
        let now = now_epoch_ms();
        if now < expire_at {
            Ok(Some(Duration::from_millis(expire_at - now)))
        } else {
            // stale 状态下逻辑 TTL 已尽（口径固定为 None）
            Ok(None)
        }
    }

    async fn len(&self) -> OxCacheResult<u64> {
        self.inner.len().await
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        self.inner.capacity().await
    }

    async fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
        let mut stats = self.inner.stats().await?;
        stats.insert(
            "stale_ttl_ms".to_string(),
            self.stale_ttl.as_millis().to_string(),
        );
        Ok(stats)
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        // keys 语义保持宽口径（含 stale 窗口内条目），透传 inner
        self.inner.keys(pattern).await
    }
}

#[async_trait]
impl CacheWriter for StaleWhileRevalidateBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let Some(ttl) = ttl else {
            // 无 TTL：无 stale 语义，原样透传
            return self.inner.set(key, value, None).await;
        };
        let now = now_epoch_ms();
        let expire_at = now.saturating_add(ttl.as_millis() as u64);
        let stale_at = expire_at.saturating_add(self.stale_ttl.as_millis() as u64);
        let physical = ttl + self.stale_ttl;
        let envelope = Arc::new(encode_envelope(expire_at, stale_at, &value));
        self.inner.set(key, envelope, Some(physical)).await
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.inner.delete(key).await
    }

    async fn clear(&self) -> OxCacheResult<()> {
        self.inner.clear().await
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        // 重算双时间戳：逻辑 TTL = ttl，物理 TTL = ttl + stale_ttl
        let Some(raw) = self.inner.get(key).await? else {
            return Ok(false);
        };
        let payload: Vec<u8> = match decode_envelope(&raw) {
            Some((_, _, payload)) => payload.to_vec(),
            None => raw,
        };
        let now = now_epoch_ms();
        let expire_at = now.saturating_add(ttl.as_millis() as u64);
        let stale_at = expire_at.saturating_add(self.stale_ttl.as_millis() as u64);
        let envelope = Arc::new(encode_envelope(expire_at, stale_at, &payload));
        let physical = ttl + self.stale_ttl;
        self.inner
            .set(Arc::from(key), envelope, Some(physical))
            .await?;
        Ok(true)
    }

    async fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        let mut wrapped: Vec<CacheSetItem> = Vec::with_capacity(items.len());
        for (key, value, ttl) in items {
            let Some(ttl) = *ttl else {
                wrapped.push((key.clone(), value.clone(), None));
                continue;
            };
            let now = now_epoch_ms();
            let expire_at = now.saturating_add(ttl.as_millis() as u64);
            let stale_at = expire_at.saturating_add(self.stale_ttl.as_millis() as u64);
            let physical = ttl + self.stale_ttl;
            wrapped.push((
                key.clone(),
                Arc::new(encode_envelope(expire_at, stale_at, value)),
                Some(physical),
            ));
        }
        self.inner.set_many(&wrapped).await
    }
}

#[async_trait]
impl CacheConnector for StaleWhileRevalidateBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        self.inner.health_check().await
    }

    async fn shutdown(&self) {
        self.inner.shutdown().await;
    }

    fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }
}

#[cfg(all(test, feature = "stale"))]
mod tests {
    use super::*;
    use crate::backend::MokaMemoryBackend;
    use crate::core::events::{CacheEvent, CacheEventType, EventPublisher};
    use crate::error::OxCacheError;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn backend(stale_ttl: Duration) -> StaleWhileRevalidateBackend {
        StaleWhileRevalidateBackend::new(Arc::new(MokaMemoryBackend::new()), stale_ttl)
    }

    fn k(s: &str) -> Arc<str> {
        Arc::from(s)
    }

    /// 记录 Expire 事件的测试发布器
    #[derive(Default)]
    struct RecordingPublisher {
        expire_events: AtomicUsize,
    }

    #[async_trait]
    impl EventPublisher for RecordingPublisher {
        async fn publish(&self, event: CacheEvent) -> Result<(), OxCacheError> {
            if event.event_type == CacheEventType::Expire {
                self.expire_events.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn fresh_read_round_trips() {
        let backend = backend(Duration::from_secs(60));
        backend
            .set(k("f"), Arc::new(vec![1, 2]), Some(Duration::from_secs(5)))
            .await
            .unwrap();
        let (bytes, state) = backend.get_with_state("f").await.unwrap();
        assert_eq!(state, StaleState::Fresh);
        assert_eq!(bytes, Some(vec![1, 2]));
        // 逻辑 TTL 为剩余值（< 设定 TTL）
        let ttl = backend.ttl("f").await.unwrap().unwrap();
        assert!(ttl <= Duration::from_secs(5));
    }

    #[tokio::test]
    async fn stale_window_serves_old_value_and_emits_event() {
        let publisher = Arc::new(RecordingPublisher::default());
        let backend = backend(Duration::from_millis(500)).with_event_publisher(publisher.clone());
        backend
            .set(k("s"), Arc::new(vec![7]), Some(Duration::from_millis(50)))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(80)).await; // 过期，仍在 stale 窗口
        let (bytes, state) = backend.get_with_state("s").await.unwrap();
        assert_eq!(state, StaleState::Stale);
        assert_eq!(
            bytes,
            Some(vec![7]),
            "stale window must serve the old value"
        );
        assert_eq!(
            publisher.expire_events.load(Ordering::SeqCst),
            1,
            "stale hit must publish Expire event with state=stale"
        );
        // 逻辑 TTL 已尽 → None
        assert_eq!(backend.ttl("s").await.unwrap(), None);
        assert!(backend.exists("s").await.unwrap());
    }

    #[tokio::test]
    async fn beyond_stale_window_is_expired_and_deleted() {
        let backend = backend(Duration::from_millis(30));
        backend
            .set(k("e"), Arc::new(vec![7]), Some(Duration::from_millis(30)))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(90)).await;
        let (bytes, state) = backend.get_with_state("e").await.unwrap();
        assert_eq!(state, StaleState::Expired);
        assert_eq!(bytes, None);
        // 物理删除：inner 不再持有条目
        assert!(!backend.exists("e").await.unwrap());
    }

    #[tokio::test]
    async fn opaque_passthrough_for_legacy_values() {
        let inner = Arc::new(MokaMemoryBackend::new());
        // 绕过装饰器直写（旧数据 / 旁路写入）
        inner
            .set(
                k("legacy"),
                Arc::new(vec![3, 3]),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        let backend = StaleWhileRevalidateBackend::new(inner.clone(), Duration::from_secs(60));
        let (bytes, state) = backend.get_with_state("legacy").await.unwrap();
        assert_eq!(state, StaleState::Opaque);
        assert_eq!(bytes, Some(vec![3, 3]));
        assert!(backend.ttl("legacy").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn no_ttl_passthrough_has_no_stale_semantics() {
        let backend = backend(Duration::from_millis(50));
        backend
            .set(k("forever"), Arc::new(vec![1]), None)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(80)).await;
        // 无 TTL 条目不包 envelope，永不过期
        let (bytes, state) = backend.get_with_state("forever").await.unwrap();
        assert_eq!(state, StaleState::Opaque);
        assert_eq!(bytes, Some(vec![1]));
    }

    #[tokio::test]
    async fn delete_clear_and_set_many_transparent() {
        let backend = backend(Duration::from_secs(60));
        let items: Vec<CacheSetItem> = vec![
            (k("m1"), Arc::new(vec![1]), Some(Duration::from_secs(60))),
            (k("m2"), Arc::new(vec![2]), None),
        ];
        backend.set_many(&items).await.unwrap();
        assert_eq!(backend.get("m1").await.unwrap(), Some(vec![1]));
        assert_eq!(backend.get("m2").await.unwrap(), Some(vec![2]));
        backend.delete("m1").await.unwrap();
        assert_eq!(backend.get("m1").await.unwrap(), None);
        backend.clear().await.unwrap();
        assert_eq!(backend.get("m2").await.unwrap(), None);
    }

    /// 与 CompressingBackend 双向叠加：两种包装顺序下三态判定与压缩均正确。
    #[tokio::test]
    async fn composes_with_compression_in_both_orders() {
        use crate::features::compression::CompressingBackend;
        let base = Arc::new(MokaMemoryBackend::new());
        // 顺序一：stale(压缩(base))
        let compressed = Arc::new(CompressingBackend::new(base.clone()));
        let stale_outer = StaleWhileRevalidateBackend::new(compressed, Duration::from_secs(60));
        stale_outer
            .set(
                k("c1"),
                Arc::new(vec![b'x'; 4096]),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        assert_eq!(
            stale_outer.get("c1").await.unwrap(),
            Some(vec![b'x'; 4096]),
            "stale(compress(base)) order must round-trip"
        );
        // 顺序二：压缩(stale(base))
        let base2 = Arc::new(MokaMemoryBackend::new());
        let stale_inner = Arc::new(StaleWhileRevalidateBackend::new(
            base2.clone(),
            Duration::from_secs(60),
        ));
        let compressed_outer = CompressingBackend::new(stale_inner);
        let payload = vec![b'y'; 4096];
        compressed_outer
            .set(
                k("c2"),
                Arc::new(payload.clone()),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        assert_eq!(
            compressed_outer.get("c2").await.unwrap(),
            Some(payload),
            "compress(stale(base)) order must round-trip"
        );
    }
}
