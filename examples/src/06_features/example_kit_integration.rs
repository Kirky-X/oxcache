// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// trait-kit 集成示例
//
// 本示例演示 oxcache 的 trait-kit `kit` feature 集成：
// - OxcacheBuildObserver：构建观察者，监听模块构建事件
// - register_cache_shutdown：三阶段优雅关闭
// - register_cache_decorator：后端装饰器（访问计数代理）
//
// trait-kit 提供依赖注入和能力管理，oxcache 通过 OxcacheModule
// 注册为 kit 模块，获得 observer/shutdown/decorator 等生命周期管理。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use oxcache::backend::{CacheConnector, CacheReader, CacheWriter};
use oxcache::integrations::kit::{
    OxcacheBuildObserver, OxcacheConfig, OxcacheModule, register_cache_decorator,
    register_cache_shutdown,
};
use trait_kit::prelude::*;

use oxcache::i18n::messages::{
    MSG_EXAMPLE_KIT_INTEGRATION_1_BUILDING_ASYNCKIT_OXCACHEMODULE,
    MSG_EXAMPLE_KIT_INTEGRATION_2_USING_THE_CACHE,
    MSG_EXAMPLE_KIT_INTEGRATION_3_THREE_PHASE_SHUTDOWN,
    MSG_EXAMPLE_KIT_INTEGRATION_ASYNCKIT_BUILD_COMPLETE, MSG_EXAMPLE_KIT_INTEGRATION_DONE,
    MSG_EXAMPLE_KIT_INTEGRATION_DRAINQUEUE_HEALTH_CHECK_LIVENESS,
    MSG_EXAMPLE_KIT_INTEGRATION_GET_USER_999_SHOULD,
    MSG_EXAMPLE_KIT_INTEGRATION_OPERATION_COUNT_SHOULD_BE,
    MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_OXCACHEBUILDOBSERVER,
    MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_OXCACHEMODULE,
    MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_THE_COUNTINGDECORATOR_DECORAT,
    MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_THREE_PHASE_SHUTDOWN,
    MSG_EXAMPLE_KIT_INTEGRATION_SHUTDOWN_RESULT,
    MSG_EXAMPLE_KIT_INTEGRATION_TRAIT_KIT_INTEGRATION_DEMO, t,
};

/// 计数装饰器：包装 CacheBackend，记录操作次数。
struct CountingDecorator {
    inner: Arc<dyn oxcache::backend::CacheBackend + Send + Sync>,
    ops: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl CacheReader for CountingDecorator {
    async fn get(&self, key: &str) -> oxcache::error::OxCacheResult<Option<Vec<u8>>> {
        self.ops.fetch_add(1, Ordering::SeqCst);
        self.inner.get(key).await
    }
    async fn exists(&self, key: &str) -> oxcache::error::OxCacheResult<bool> {
        self.inner.exists(key).await
    }
    async fn ttl(&self, key: &str) -> oxcache::error::OxCacheResult<Option<std::time::Duration>> {
        self.inner.ttl(key).await
    }
    async fn len(&self) -> oxcache::error::OxCacheResult<u64> {
        self.inner.len().await
    }
    async fn capacity(&self) -> oxcache::error::OxCacheResult<u64> {
        self.inner.capacity().await
    }
    async fn stats(
        &self,
    ) -> oxcache::error::OxCacheResult<std::collections::HashMap<String, String>> {
        self.inner.stats().await
    }
}

#[async_trait::async_trait]
impl CacheWriter for CountingDecorator {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<std::time::Duration>,
    ) -> oxcache::error::OxCacheResult<()> {
        self.ops.fetch_add(1, Ordering::SeqCst);
        self.inner.set(key, value, ttl).await
    }
    async fn delete(&self, key: &str) -> oxcache::error::OxCacheResult<()> {
        self.inner.delete(key).await
    }
    async fn clear(&self) -> oxcache::error::OxCacheResult<()> {
        self.inner.clear().await
    }
    async fn expire(
        &self,
        key: &str,
        ttl: std::time::Duration,
    ) -> oxcache::error::OxCacheResult<bool> {
        self.inner.expire(key, ttl).await
    }
}

#[async_trait::async_trait]
impl CacheConnector for CountingDecorator {
    async fn health_check(&self) -> oxcache::error::OxCacheResult<()> {
        self.inner.health_check().await
    }
    async fn shutdown(&self) {
        self.inner.shutdown().await;
    }
    fn backend_kind(&self) -> oxcache::backend::BackendKind {
        self.inner.backend_kind()
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // === 构建 AsyncKit + OxcacheModule ===
    println!(
        "{}",
        t(
            MSG_EXAMPLE_KIT_INTEGRATION_1_BUILDING_ASYNCKIT_OXCACHEMODULE,
            &[]
        )
    );

    let mut kit = AsyncKit::new();
    kit.set_config(OxcacheConfig {
        capacity: 1000,
        ttl: Some(std::time::Duration::from_secs(300)),
        ..Default::default()
    });

    // 注册 observer：监听模块构建事件
    kit.with_observer(Arc::new(OxcacheBuildObserver));
    println!(
        "{}",
        t(
            MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_OXCACHEBUILDOBSERVER,
            &[]
        )
    );

    // 注册 OxcacheModule
    kit.register::<OxcacheModule>()?;
    println!(
        "{}",
        t(MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_OXCACHEMODULE, &[])
    );

    // 注册装饰器：计数代理
    let ops_count = Arc::new(AtomicUsize::new(0));
    let ops_clone = Arc::clone(&ops_count);
    register_cache_decorator(&kit, move |backend| {
        Arc::new(CountingDecorator {
            inner: backend,
            ops: Arc::clone(&ops_clone),
        })
    });
    println!(
        "{}",
        t(
            MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_THE_COUNTINGDECORATOR_DECORAT,
            &[]
        )
    );

    // 构建 kit
    let built = kit.build().await?;
    println!(
        "{}\n",
        t(MSG_EXAMPLE_KIT_INTEGRATION_ASYNCKIT_BUILD_COMPLETE, &[])
    );

    // === 使用缓存后端 ===
    println!("{}", t(MSG_EXAMPLE_KIT_INTEGRATION_2_USING_THE_CACHE, &[]));

    let backend = built.require::<OxcacheModule>()?;

    backend
        .set(
            Arc::from("user:1"),
            Arc::new(b"Alice".to_vec()),
            Some(std::time::Duration::from_secs(60)),
        )
        .await?;
    println!("set 'user:1' = 'Alice' (TTL=60s)");

    let value = backend.get("user:1").await?;
    println!(
        "get 'user:1' = {:?}",
        value.map(|v| String::from_utf8_lossy(&v).to_string())
    );

    // 不存在的 key
    let miss = backend.get("user:999").await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_KIT_INTEGRATION_GET_USER_999_SHOULD,
            &[("miss", format!("{:?}", miss))]
        )
    );

    println!(
        "{}",
        t(
            MSG_EXAMPLE_KIT_INTEGRATION_OPERATION_COUNT_SHOULD_BE,
            &[("seqcst", ops_count.load(Ordering::SeqCst).to_string())]
        )
    );
    println!();

    // === 三阶段关闭 ===
    println!(
        "{}",
        t(MSG_EXAMPLE_KIT_INTEGRATION_3_THREE_PHASE_SHUTDOWN, &[])
    );

    let shutdown_coord = AsyncShutdownCoordinator::new();
    register_cache_shutdown(&shutdown_coord, backend)?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_THREE_PHASE_SHUTDOWN,
            &[]
        )
    );
    println!("  - StopRequests: no-op");
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_KIT_INTEGRATION_DRAINQUEUE_HEALTH_CHECK_LIVENESS,
            &[]
        )
    );
    println!("  - CloseConnections: backend.shutdown()");

    let result = shutdown_coord.shutdown().await;
    println!(
        "{}\n",
        t(
            MSG_EXAMPLE_KIT_INTEGRATION_SHUTDOWN_RESULT,
            &[("result", format!("{:?}", result))]
        )
    );

    println!("{}", t(MSG_EXAMPLE_KIT_INTEGRATION_DONE, &[]));
    println!(
        "{}",
        t(MSG_EXAMPLE_KIT_INTEGRATION_TRAIT_KIT_INTEGRATION_DEMO, &[])
    );

    Ok(())
}
