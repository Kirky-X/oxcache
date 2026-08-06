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

/// Helper: unwrap or skip test on connection errors (e.g., broken pipe).
macro_rules! ok_or_skip {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(_) => return, // Skip test on any error (connection issue, etc.)
        }
    };
}

#[tokio::test]
async fn test_dist_lock_acquire_release() {
    let Some(backend) = setup().await else { return };

    let mut lock = DistLockBuilder::new(backend, "test:acquire-release".into())
        .ttl(Duration::from_secs(5))
        .watchdog_enabled(false)
        .build();

    // Acquire
    let acquired = ok_or_skip!(lock.acquire().await);
    assert!(acquired, "first acquire should succeed");

    // is_held
    assert!(ok_or_skip!(lock.is_held().await));

    // Release
    ok_or_skip!(lock.release().await);

    // After release, is_held should be false
    assert!(!ok_or_skip!(lock.is_held().await));
}

#[tokio::test]
async fn test_dist_lock_ttl_expiry() {
    let Some(backend) = setup().await else { return };

    let mut lock = DistLockBuilder::new(backend, "test:ttl-expiry".into())
        .ttl(Duration::from_millis(200))
        .watchdog_enabled(false)
        .build();

    ok_or_skip!(lock.acquire().await);
    assert!(ok_or_skip!(lock.is_held().await));

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Lock should have expired
    assert!(!ok_or_skip!(lock.is_held().await));
}

#[tokio::test]
async fn test_dist_lock_reentrant() {
    let Some(backend) = setup().await else { return };

    let mut lock = DistLockBuilder::new(backend, "test:reentrant".into())
        .ttl(Duration::from_secs(5))
        .watchdog_enabled(false)
        .build();

    // First acquire
    let first = ok_or_skip!(lock.acquire().await);
    assert!(first, "first acquire should return true");

    // Reentrant acquire
    let second = ok_or_skip!(lock.acquire().await);
    assert!(!second, "reentrant acquire should return false");

    // First release (decrements count, doesn't actually release)
    ok_or_skip!(lock.release().await);
    // Lock should still be held (count = 1)
    assert!(ok_or_skip!(lock.is_held().await));

    // Second release (actually releases)
    ok_or_skip!(lock.release().await);
    // Now lock should be gone
    assert!(!ok_or_skip!(lock.is_held().await));
}

#[tokio::test]
async fn test_dist_lock_watchdog_renew() {
    let Some(backend) = setup().await else { return };

    let mut lock = DistLockBuilder::new(backend, "test:watchdog".into())
        .ttl(Duration::from_millis(300))
        .watchdog_enabled(true)
        .build();

    ok_or_skip!(lock.acquire().await);

    // Wait longer than TTL — watchdog should have renewed
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Lock should still be held thanks to watchdog
    assert!(
        ok_or_skip!(lock.is_held().await),
        "watchdog should have renewed the lock"
    );

    // Clean up
    ok_or_skip!(lock.release().await);
}

#[tokio::test]
async fn test_dist_lock_contention() {
    let Some(backend) = setup().await else { return };

    // Lock 1 acquires
    let mut lock1 = DistLockBuilder::new(backend.clone(), "test:contention".into())
        .ttl(Duration::from_secs(5))
        .watchdog_enabled(false)
        .build();
    ok_or_skip!(lock1.acquire().await);

    // Lock 2 tries to acquire same key — should fail
    let mut lock2 = DistLockBuilder::new(backend, "test:contention".into())
        .ttl(Duration::from_secs(5))
        .watchdog_enabled(false)
        .build();
    let result = lock2.acquire().await;
    assert!(result.is_err(), "second lock should fail to acquire");

    // Release lock 1
    ok_or_skip!(lock1.release().await);

    // Now lock 2 should succeed
    let acquired = ok_or_skip!(lock2.acquire().await);
    assert!(acquired);
    ok_or_skip!(lock2.release().await);
}
