// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Distributed lock integration tests (require Redis testcontainer).

#[path = "../../common/mod.rs"]
mod common;
use common::test_containers::{
    RedisContainer, backend_skip, container_or_skip, start_redis_container,
};

use oxcache::backend::CacheConnector;
use oxcache::backend::RedisBackend;
use oxcache::features::dist_lock::DistLockBuilder;
use std::sync::Arc;
use std::time::Duration;

/// Set up Redis container and backend.
///
/// 容器句柄随返回值一起交给测试体持有：testcontainers 0.28 的
/// `ContainerAsync` 在 drop 时删除容器，若锁在 setup 局部作用域内，
/// 返回后全部锁操作连接失败。容器不可用/超时/backend 握手失败时经
/// `container_or_skip`/`backend_skip` 显性跳过（`OXCACHE_TEST_STRICT`
/// 置位时改为失败），不再静默 `return`。
async fn setup(label: &str) -> Option<(RedisContainer, Arc<RedisBackend>)> {
    let (container, url) = container_or_skip(label, start_redis_container).await?;
    let backend = match RedisBackend::new(&url).await {
        Ok(b) => b,
        Err(e) => return backend_skip(label, &e.to_string()),
    };
    if let Err(e) = backend.health_check().await {
        return backend_skip(label, &e.to_string());
    }
    Some((container, Arc::new(backend)))
}

#[tokio::test]
async fn test_dist_lock_acquire_release() {
    let Some((_container, backend)) = setup("dist_lock(acquire-release)").await else {
        return;
    };

    let mut lock = DistLockBuilder::new(backend, "test:acquire-release".into())
        .ttl(Duration::from_secs(5))
        .watchdog_enabled(false)
        .build();

    // Acquire
    let acquired = lock.acquire().await.expect("acquire");
    assert!(acquired, "first acquire should succeed");

    // is_held
    assert!(lock.is_held().await.expect("is_held"));

    // Release
    lock.release().await.expect("release");

    // After release, is_held should be false
    assert!(!lock.is_held().await.expect("is_held"));
}

#[tokio::test]
async fn test_dist_lock_ttl_expiry() {
    let Some((_container, backend)) = setup("dist_lock(ttl-expiry)").await else {
        return;
    };

    let mut lock = DistLockBuilder::new(backend, "test:ttl-expiry".into())
        .ttl(Duration::from_millis(200))
        .watchdog_enabled(false)
        .build();

    lock.acquire().await.expect("acquire");
    assert!(lock.is_held().await.expect("is_held"));

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Lock should have expired
    assert!(!lock.is_held().await.expect("is_held"));
}

#[tokio::test]
async fn test_dist_lock_reentrant() {
    let Some((_container, backend)) = setup("dist_lock(reentrant)").await else {
        return;
    };

    let mut lock = DistLockBuilder::new(backend, "test:reentrant".into())
        .ttl(Duration::from_secs(5))
        .watchdog_enabled(false)
        .build();

    // First acquire
    let first = lock.acquire().await.expect("acquire");
    assert!(first, "first acquire should return true");

    // Reentrant acquire
    let second = lock.acquire().await.expect("acquire");
    assert!(!second, "reentrant acquire should return false");

    // First release (decrements count, doesn't actually release)
    lock.release().await.expect("release");
    // Lock should still be held (count = 1)
    assert!(lock.is_held().await.expect("is_held"));

    // Second release (actually releases)
    lock.release().await.expect("release");
    // Now lock should be gone
    assert!(!lock.is_held().await.expect("is_held"));
}

#[tokio::test]
async fn test_dist_lock_watchdog_renew() {
    let Some((_container, backend)) = setup("dist_lock(watchdog)").await else {
        return;
    };

    let mut lock = DistLockBuilder::new(backend, "test:watchdog".into())
        .ttl(Duration::from_millis(300))
        .watchdog_enabled(true)
        .build();

    lock.acquire().await.expect("acquire");

    // Wait longer than TTL — watchdog should have renewed
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Lock should still be held thanks to watchdog
    assert!(
        lock.is_held().await.expect("is_held"),
        "watchdog should have renewed the lock"
    );

    lock.release().await.expect("release");
}

#[tokio::test]
async fn test_dist_lock_contention() {
    let Some((_container, backend)) = setup("dist_lock(contention)").await else {
        return;
    };

    // Lock 1 acquires
    let mut lock1 = DistLockBuilder::new(backend.clone(), "test:contention".into())
        .ttl(Duration::from_secs(5))
        .watchdog_enabled(false)
        .build();
    lock1.acquire().await.expect("acquire");

    // Lock 2 tries to acquire same key — should fail
    let mut lock2 = DistLockBuilder::new(backend, "test:contention".into())
        .ttl(Duration::from_secs(5))
        .watchdog_enabled(false)
        .build();
    let result = lock2.acquire().await;
    assert!(result.is_err(), "second lock should fail to acquire");

    // Release lock 1
    lock1.release().await.expect("release");

    // Now lock 2 should succeed
    let acquired = lock2.acquire().await.expect("acquire");
    assert!(acquired);
    lock2.release().await.expect("release");
}
