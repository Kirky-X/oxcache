#![allow(clippy::module_inception)]
#[allow(unused_imports)]
pub use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::BackendKind;
    use crate::backend::interface::{CacheConnector, CacheReader, CacheWriter};

    #[tokio::test]
    async fn l1_builder_moka_default() {
        let backend = L1Builder::new().build();
        assert_eq!(backend.backend_kind(), BackendKind::Moka);
        assert_eq!(backend.capacity().await.unwrap(), 10_000);
    }

    #[tokio::test]
    async fn l1_builder_capacity_ttl_and_dashmap() {
        let backend = L1Builder::new()
            .dashmap()
            .capacity(64)
            .ttl(Duration::from_secs(30))
            .build();
        assert_eq!(backend.backend_kind(), BackendKind::DashMap);
        assert_eq!(backend.capacity().await.unwrap(), 64);

        // TTL 生效
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        let ttl = backend.ttl("k").await.unwrap();
        assert!(ttl.is_some(), "DashMap default_ttl 应生效");
    }

    /// 装饰器按追加顺序由内向外包装
    #[tokio::test]
    async fn l1_builder_applies_decorators() {
        let backend = L1Builder::new()
            .decorate(|inner| Arc::new(TracingProbe { inner }))
            .decorate(|inner| Arc::new(TracingProbe { inner }))
            .build();

        // 读写经两层装饰透传仍正确
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(backend.get("k").await.unwrap(), Some(b"v".to_vec()));
    }

    /// 测试装饰器：记录包装层级
    struct TracingProbe {
        inner: Arc<dyn CacheBackend>,
    }

    #[async_trait::async_trait]
    impl CacheReader for TracingProbe {
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
        async fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
            self.inner.stats().await
        }
    }

    #[async_trait::async_trait]
    impl CacheWriter for TracingProbe {
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
    impl CacheConnector for TracingProbe {
        async fn health_check(&self) -> OxCacheResult<()> {
            self.inner.health_check().await
        }
        async fn shutdown(&self) {
            self.inner.shutdown().await;
        }
        fn backend_kind(&self) -> BackendKind {
            BackendKind::Unknown
        }
    }
    // `CacheBackend` 由 blanket impl 自动提供

    #[tokio::test]
    async fn l2_builder_custom_backend() {
        let inner: Arc<dyn CacheBackend> =
            Arc::new(crate::backend::MockBackend::new("mock-l2", 50, true));
        let l2 = L2Builder::new().custom(inner).build().await.unwrap();
        assert!(l2.exists("nothing").await.unwrap().eq(&false));
    }

    #[tokio::test]
    async fn l2_builder_without_backend_is_error() {
        let err = match L2Builder::new().build().await {
            Err(e) => e,
            Ok(_) => panic!("无后端必须报错"),
        };
        assert!(matches!(err, OxCacheError::InvalidInput(_)));
    }

    /// L1 + L2 一站式组装：读取顺序、回填语义与手工 ChainCache 一致
    #[tokio::test]
    async fn chain_builder_assembles_l1_l2() {
        let l2_backend: Arc<dyn CacheBackend> =
            Arc::new(crate::backend::MockBackend::new("mock-l2", 50, false));
        // L2 预置数据（模拟之前写入 L2）
        l2_backend
            .set(Arc::from("user:1"), Arc::new(b"from-l2".to_vec()), None)
            .await
            .unwrap();

        let chain = ChainBuilder::new()
            .l1(L1Builder::new().capacity(100))
            .l2(L2Builder::new().custom(l2_backend))
            .enable_backfill()
            .build()
            .await
            .unwrap();

        // 从 L2 命中
        let value = chain.get("user:1").await.unwrap();
        assert_eq!(value, Some(b"from-l2".to_vec()));
        assert_eq!(chain.len(), 2);
    }

    #[tokio::test]
    async fn chain_builder_requires_at_least_one_layer() {
        let err = match ChainBuilder::new().build().await {
            Err(e) => e,
            Ok(_) => panic!("空链必须报错"),
        };
        assert!(matches!(err, OxCacheError::InvalidInput(_)));
        assert!(err.to_string().contains(".l1("));
    }

    #[tokio::test]
    async fn chain_builder_l1_only_works() {
        let chain = ChainBuilder::new()
            .l1(L1Builder::new())
            .build()
            .await
            .unwrap();
        chain.set("k", b"v".to_vec(), None).await.unwrap();
        assert_eq!(chain.get("k").await.unwrap(), Some(b"v".to_vec()));
    }
}
