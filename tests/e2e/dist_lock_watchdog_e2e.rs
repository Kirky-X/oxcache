// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! E2E：分布式锁看门狗（watchdog）组合语义（场景 ID：OXL-WD-01/02/03）。
//!
//! 场景来源：`docs/TEST_SCENARIOS.md`「分布式锁（LOCK）」域。
//!
//! 既有覆盖引用声明：`tests/integration/redis/dist_lock_test.rs`（6 测试）
//! 已覆盖单持有方续期（`test_dist_lock_watchdog_renew`：ttl=300ms，sleep
//! 500ms 后 `is_held` 仍为 true）、重入计数、TTL 过期、争用互斥与
//! acquire/release 基本环（其余测试均 `watchdog_enabled(false)`）。本文件
//! 补看门狗与其他语义的**组合行为**：
//!
//! - OXL-WD-01：watchdog 续期期间互斥保持——A 持锁跨多个 TTL（2.5s >
//!   2×TTL，续期间隔 ttl/3 ≈ 333ms）后 B 仍被拒，错误表明 "already held"；
//! - OXL-WD-02：`release` 停止续期并立即转手——B 随即 acquire 成功；
//! - OXL-WD-03：release 后看门狗确已停止——TTL 到期后键消失、无"复活"
//!   续期（`is_held` 为 false 且 `backend.exists` 为 false）。
//!
//! 环境依赖：Docker（testcontainers 自起 redis:7-alpine，测毕自清）；
//! Docker 不可用时静默跳过（沿用 `dist_lock_test.rs` 的 setup 口径）。

#[path = "../common/mod.rs"]
mod common;

use common::test_containers::RedisContainer;

use oxcache::backend::{CacheConnector, CacheReader, RedisBackend};
use oxcache::features::dist_lock::DistLockBuilder;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 纳秒级唯一锁 key，避免并发测试间键冲突。
fn unique_key(label: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH")
        .as_nanos();
    format!("e2e:wd:{label}:{ts}")
}

/// Set up Redis container and backend; skip test if Docker unavailable.
///
/// 返回容器句柄由测试体持有至结束：testcontainers 0.28 的 `ContainerAsync`
/// 在 drop 时即删除容器，若容器锁在 setup 局部作用域内，首个后续连接操作
/// 必报 broken pipe（既有 `dist_lock_test.rs` 的 `ok_or_skip!` 宏恰好掩盖
/// 了该问题）。
///
/// `RedisBackend::new` 对非 TLS 连接要求 `OXCACHE_ALLOW_INSECURE_REDIS`
/// （同值重复设置幂等安全，沿用 `valkey_test.rs` 的口径）。
async fn setup() -> Option<(RedisContainer, Arc<RedisBackend>)> {
    // SAFETY: test-only environment variable, idempotent same-value set
    unsafe { std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS") };
    let container = RedisContainer::start().await.ok()?;
    container.wait_ready().await.ok()?;
    let backend = RedisBackend::new(&container.url()).await.ok()?;
    backend.health_check().await.ok()?;
    Some((container, Arc::new(backend)))
}

/// OXL-WD-01/02：watchdog 续期保持互斥；release 停止续期并立即可转手。
#[tokio::test]
async fn test_dist_lock_watchdog_exclusive_then_handover() {
    // 容器句柄绑定到测试体作用域，防止 drop 提前删除容器
    let Some((_container, backend)) = setup().await else {
        return;
    };

    let key = unique_key("exclusive-handover");

    // OXL-WD-01：A 持锁，watchdog 每 ttl/3 ≈ 333ms 自动续期
    let mut lock_a = DistLockBuilder::new(backend.clone(), key.clone())
        .ttl(Duration::from_secs(1))
        .watchdog_enabled(true)
        .build();
    assert!(
        lock_a.acquire().await.expect("A acquire"),
        "A 首次获取应成功"
    );

    // 跨多个 TTL 等待（2.5s > 2×TTL），期间看门狗持续续期
    tokio::time::sleep(Duration::from_millis(2500)).await;

    // B 仍应被互斥——续期不得产生可乘之隙
    let mut lock_b = DistLockBuilder::new(backend.clone(), key.clone())
        .ttl(Duration::from_secs(1))
        .watchdog_enabled(false)
        .build();
    let err = lock_b
        .acquire()
        .await
        .expect_err("watchdog 续期期间 B 应被互斥");
    assert!(
        err.to_string().contains("already held"),
        "互斥错误应表明锁被他人持有: {err}"
    );

    // OXL-WD-02：release 停止 A 的看门狗并删键，B 立即可转手
    lock_a.release().await.expect("A release");
    assert!(
        lock_b.acquire().await.expect("B 转手获取"),
        "release 后 B 应立即获取成功"
    );
    lock_b.release().await.expect("B release");
}

/// OXL-WD-03：release 后看门狗确已停止——TTL 到期后键消失，无"复活"续期。
#[tokio::test]
async fn test_dist_lock_no_renewal_after_release() {
    // 容器句柄绑定到测试体作用域，防止 drop 提前删除容器
    let Some((_container, backend)) = setup().await else {
        return;
    };

    let key = unique_key("cease");

    let mut lock = DistLockBuilder::new(backend.clone(), key.clone())
        .ttl(Duration::from_secs(1))
        .watchdog_enabled(true)
        .build();
    assert!(lock.acquire().await.expect("acquire"));
    assert!(lock.is_held().await.expect("is_held"));

    lock.release().await.expect("release");

    // TTL(1s) 过后：若看门狗未被停止，会以 ttl/3 间隔持续续期、键"复活"；
    // 真实行为：release 已 abort 看门狗 + Lua 删键，键应保持消失。
    tokio::time::sleep(Duration::from_millis(1300)).await;

    assert!(
        !lock.is_held().await.expect("is_held after ttl"),
        "release 后键不应被看门狗续期复活"
    );
    assert!(
        !backend.exists(&key).await.expect("exists"),
        "release 并过期后 Redis 中键应不存在"
    );
}
