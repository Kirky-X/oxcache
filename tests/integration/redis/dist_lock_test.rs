// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Distributed lock integration tests (require Redis testcontainer).

#[path = "../../common/mod.rs"]
mod common;
use common::test_containers::RedisContainer;

use oxcache::backend::CacheConnector;
use oxcache::backend::RedisBackend;
use oxcache::features::dist_lock::DistLockBuilder;
use std::sync::Arc;
use std::time::Duration;

/// Set up Redis container and backend; skip test if Docker unavailable.
async fn setup() -> Option<Arc<RedisBackend>> {
    let container = RedisContainer::start().await.ok()?;
    container.wait_ready().await.ok()?;
    let backend = RedisBackend::new(&container.url()).await.ok()?;
    backend.health_check().await.ok()?;
    Some(Arc::new(backend))
}

#[tokio::test]
async fn test_dist_lock_acquire_release() {
    let Some(backend) = setup().await else { return };

    let mut lock = DistLockBuilder::new(backend, "test:acquire-release".into())
        .ttl(Duration::from_secs(5))
        .watchdog_enabled(false)
        .build();

    // Acquire
    let acquired = lock.acquire().await.unwrap();
    assert!(acquired, "first acquire should succeed");

    // is_held
    assert!(lock.is_held().await.unwrap());

    // Release
    lock.release().await.unwrap();

    // After release, is_held should be false
    assert!(!lock.is_held().await.unwrap());
}

#[tokio::test]
async fn test_dist_lock_ttl_expiry() {
    let Some(backend) = setup().await else { return };

    let mut lock = DistLockBuilder::new(backend, "test:ttl-expiry".into())
        .ttl(Duration::from_millis(200))
        .watchdog_enabled(false)
        .build();

    lock.acquire().await.unwrap();
    assert!(lock.is_held().await.unwrap());

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Lock should have expired
    assert!(!lock.is_held().await.unwrap());
}

#[tokio::test]
async fn test_dist_lock_reentrant() {
    let Some(backend) = setup().await else { return };

    let mut lock = DistLockBuilder::new(backend, "test:reentrant".into())
        .ttl(Duration::from_secs(5))
        .watchdog_enabled(false)
        .build();

    // First acquire
    let first = lock.acquire().await.unwrap();
    assert!(first, "first acquire should return true");

    // Reentrant acquire
    let second = lock.acquire().await.unwrap();
    assert!(!second, "reentrant acquire should return false");

    // First release (decrements count, doesn't actually release)
    lock.release().await.unwrap();
    // Lock should still be held (count = 1)
    assert!(lock.is_held().await.unwrap());

    // Second release (actually releases)
    lock.release().await.unwrap();
    // Now lock should be gone
    assert!(!lock.is_held().await.unwrap());
}

#[tokio::test]
async fn test_dist_lock_watchdog_renew() {
    let Some(backend) = setup().await else { return };

    let mut lock = DistLockBuilder::new(backend, "test:watchdog".into())
        .ttl(Duration::from_millis(300))
        .watchdog_enabled(true)
        .build();

    lock.acquire().await.unwrap();

    // Wait longer than TTL — watchdog should have renewed
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Lock should still be held thanks to watchdog
    assert!(lock.is_held().await.unwrap(), "watchdog should have renewed the lock");

    // Clean up
    lock.release().await.unwrap();
}

#[tokio::test]
async fn test_dist_lock_contention() {
    let Some(backend) = setup().await else { return };

    // Lock 1 acquires
    let mut lock1 = DistLockBuilder::new(backend.clone(), "test:contention".into())
        .ttl(Duration::from_secs(5))
        .watchdog_enabled(false)
        .build();
    lock1.acquire().await.unwrap();

    // Lock 2 tries to acquire same key — should fail
    let mut lock2 = DistLockBuilder::new(backend, "test:contention".into())
        .ttl(Duration::from_secs(5))
        .watchdog_enabled(false)
        .build();
    let result = lock2.acquire().await;
    assert!(result.is_err(), "second lock should fail to acquire");

    // Release lock 1
    lock1.release().await.unwrap();

    // Now lock 2 should succeed
    let acquired = lock2.acquire().await.unwrap();
    assert!(acquired);
    lock2.release().await.unwrap();
}
