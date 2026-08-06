// Copyright (c) 2025-2026 Kirky.X
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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use oxcache::backend::{CacheConnector, CacheReader, CacheWriter};
use oxcache::integrations::kit::{
    register_cache_decorator, register_cache_shutdown, OxcacheBuildObserver, OxcacheConfig,
    OxcacheModule,
};
use trait_kit::prelude::*;

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
    async fn ttl(
        &self,
        key: &str,
    ) -> oxcache::error::OxCacheResult<Option<std::time::Duration>> {
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
    // === 1. 构建 AsyncKit + OxcacheModule ===
    println!("=== 1. 构建 AsyncKit + OxcacheModule ===");

    let mut kit = AsyncKit::new();
    kit.set_config(OxcacheConfig {
        capacity: 1000,
        ttl: Some(std::time::Duration::from_secs(300)),
        ..Default::default()
    });

    // 注册 observer：监听模块构建事件
    kit.with_observer(Arc::new(OxcacheBuildObserver));
    println!("已注册 OxcacheBuildObserver");

    // 注册 OxcacheModule
    kit.register::<OxcacheModule>()?;
    println!("已注册 OxcacheModule");

    // 注册装饰器：计数代理
    let ops_count = Arc::new(AtomicUsize::new(0));
    let ops_clone = Arc::clone(&ops_count);
    register_cache_decorator(&kit, move |backend| {
        Arc::new(CountingDecorator {
            inner: backend,
            ops: Arc::clone(&ops_clone),
        })
    });
    println!("已注册 CountingDecorator 装饰器");

    // 构建 kit
    let built = kit.build().await?;
    println!("AsyncKit 构建完成\n");

    // === 2. 使用缓存后端 ===
    println!("=== 2. 使用缓存后端 ===");

    let backend = built.require::<OxcacheModule>()?;

    // 写入
    backend
        .set(
            Arc::from("user:1"),
            Arc::new(b"Alice".to_vec()),
            Some(std::time::Duration::from_secs(60)),
        )
        .await?;
    println!("set 'user:1' = 'Alice' (TTL=60s)");

    // 读取
    let value = backend.get("user:1").await?;
    println!("get 'user:1' = {:?}", value.map(|v| String::from_utf8_lossy(&v).to_string()));

    // 不存在的 key
    let miss = backend.get("user:999").await?;
    println!("get 'user:999' = {:?} (应为 None)", miss);

    println!("操作计数: {} (应为 3: set + get + get)", ops_count.load(Ordering::SeqCst));
    println!();

    // === 3. 健康检查 ===
    println!("=== 3. 健康检查 ===");
    let health = built.health_check::<OxcacheModule>();
    match health {
        Ok(status) => println!("健康状态: {:?}", status),
        Err(e) => println!("健康检查失败: {e}"),
    }
    println!();

    // === 4. 三阶段关闭 ===
    println!("=== 4. 三阶段关闭 ===");

    let shutdown_coord = AsyncShutdownCoordinator::new();
    register_cache_shutdown(&shutdown_coord, backend)?;
    println!("已注册三阶段关闭钩子:");
    println!("  - StopRequests: no-op");
    println!("  - DrainQueue: health_check 探活");
    println!("  - CloseConnections: backend.shutdown()");

    let result = shutdown_coord.shutdown().await;
    println!("关闭结果: {:?}\n", result);

    println!("=== 完成 ===");
    println!("trait-kit 集成演示结束。observer/decorator/shutdown 三个特性均已展示。");

    Ok(())
}
