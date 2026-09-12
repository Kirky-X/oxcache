// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 写路径失效广播装饰器
//!
//! [`InvalidatingBackend`] 包装任意 [`CacheBackend`]：写操作（set/delete/
//! set_many/delete_many/clear）成功后经 [`InvalidationBus`] 广播失效事件，
//! 其他实例的监听任务失效各自本地 L1。发布失败不影响写结果（fire-and-forget：
//! 底层写入已成功，广播为尽力而为）。

use super::InvalidationBus;
use crate::backend::interface::{BackendKind, CacheSetItem};
use crate::backend::CacheBackend;
use crate::error::OxCacheResult;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// 写路径失效广播装饰器
pub struct InvalidatingBackend {
    inner: Arc<dyn CacheBackend>,
    bus: Arc<InvalidationBus>,
}

impl InvalidatingBackend {
    /// 包装内部后端与失效总线
    pub fn new(inner: Arc<dyn CacheBackend>, bus: Arc<InvalidationBus>) -> Self {
        Self { inner, bus }
    }

    /// 内部后端
    pub fn inner(&self) -> &Arc<dyn CacheBackend> {
        &self.inner
    }
}

#[async_trait]
impl crate::backend::CacheReader for InvalidatingBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        self.inner.get(key).await
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
        self.inner.stats().await
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        self.inner.keys(pattern).await
    }
}

#[async_trait]
impl crate::backend::CacheWriter for InvalidatingBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        self.inner.set(key.clone(), value, ttl).await?;
        // 写已成功，广播尽力而为：发布失败不回滚写
        let _ = self.bus.invalidate_key(&key).await;
        Ok(())
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.inner.delete(key).await?;
        let _ = self.bus.invalidate_key(key).await;
        Ok(())
    }

    async fn clear(&self) -> OxCacheResult<()> {
        self.inner.clear().await?;
        let _ = self.bus.invalidate_namespace("*").await;
        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        // TTL 变更不改变值的一致性，不广播失效
        self.inner.expire(key, ttl).await
    }

    async fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        self.inner.set_many(items).await?;
        for (key, _, _) in items {
            let _ = self.bus.invalidate_key(key).await;
        }
        Ok(())
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        self.inner.delete_many(keys).await?;
        for key in keys {
            let _ = self.bus.invalidate_key(key).await;
        }
        Ok(())
    }
}

#[async_trait]
impl crate::backend::CacheConnector for InvalidatingBackend {
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

// `CacheBackend` 由 blanket impl 自动提供（Reader+Writer+Connector 均已实现），
// 不可再显式 impl（会与 blanket impl 冲突）。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::interface::{CacheConnector, CacheReader, CacheWriter};
    use crate::backend::{CacheBackend, MockBackend};
    use crate::features::invalidation::{
        InMemoryPubSubTransport, InvalidationConfig, DEFAULT_CHANNEL,
    };

    async fn setup() -> (Arc<InvalidatingBackend>, Arc<dyn CacheBackend>, Arc<dyn CacheBackend>) {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let bus_a = Arc::new(InvalidationBus::new(
            transport.clone(),
            InvalidationConfig::new("instance-a").with_channel(DEFAULT_CHANNEL),
        ));
        let bus_b = Arc::new(InvalidationBus::new(
            transport.clone(),
            InvalidationConfig::new("instance-b").with_channel(DEFAULT_CHANNEL),
        ));

        let inner_a: Arc<dyn CacheBackend> = Arc::new(MockBackend::new("mock-a", 100, false));
        let inner_b: Arc<dyn CacheBackend> = Arc::new(MockBackend::new("mock-b", 100, false));

        let decorated = Arc::new(InvalidatingBackend::new(inner_a.clone(), bus_a));
        // B 实例监听总线，失效自己的 L1
        let _handle = bus_b.spawn_listener(inner_b.clone()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        (decorated, inner_a, inner_b)
    }

    #[tokio::test]
    async fn set_publishes_invalidation_to_other_instances() {
        let (decorated, _inner_a, inner_b) = setup().await;

        // 预先在 B 放一个旧值，模拟 B 上一次读回填
        inner_b
            .set(Arc::from("user:1"), Arc::new(b"stale".to_vec()), None)
            .await
            .unwrap();
        assert!(inner_b.exists("user:1").await.unwrap());

        // A 经装饰器写入 → 广播 → B 失效
        decorated
            .set(Arc::from("user:1"), Arc::new(b"fresh".to_vec()), None)
            .await
            .unwrap();

        for _ in 0..50 {
            if !inner_b.exists("user:1").await.unwrap() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            !inner_b.exists("user:1").await.unwrap(),
            "装饰器 set 后 B 实例的旧条目应被失效"
        );
    }

    #[tokio::test]
    async fn delete_publishes_invalidation_to_other_instances() {
        let (decorated, _inner_a, inner_b) = setup().await;

        inner_b
            .set(Arc::from("user:2"), Arc::new(b"stale".to_vec()), None)
            .await
            .unwrap();

        decorated.delete("user:2").await.unwrap();

        for _ in 0..50 {
            if !inner_b.exists("user:2").await.unwrap() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(!inner_b.exists("user:2").await.unwrap());
    }

    #[tokio::test]
    async fn read_path_is_passthrough() {
        let (decorated, inner_a, _inner_b) = setup().await;
        inner_a
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(decorated.get("k").await.unwrap(), Some(b"v".to_vec()));
        assert!(decorated.exists("k").await.unwrap());
        assert_eq!(
            decorated.backend_kind(),
            inner_a.backend_kind(),
            "装饰器透传 backend_kind"
        );
    }

    #[tokio::test]
    async fn publish_failure_does_not_fail_write() {
        // 独立 transport 无订阅者：publish 为 no-op，写必须照常成功
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let bus = Arc::new(InvalidationBus::new(
            transport,
            InvalidationConfig::new("solo"),
        ));
        let inner: Arc<dyn CacheBackend> = Arc::new(MockBackend::new("mock", 100, false));
        let decorated = InvalidatingBackend::new(inner.clone(), bus);

        decorated
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(inner.get("k").await.unwrap(), Some(b"v".to_vec()));
    }
}
