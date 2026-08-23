// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// tests/ttl_consistency_regression.rs
//
// 跨后端 TTL 一致性回归测试 (spec: universal-per-entry-ttl)
//
// 验证 Moka / DashMap 真实内存后端在 set(ttl=Some) / ttl(key) / expire(key, ttl)
// 行为上一致。DashMap 使用两个独立实例（默认构建 + builder 显式容量），
// 覆盖构建参数差异下 TTL 语义不变。这是任务组 4 的跨后端回归套件，
// 防止后续重构破坏 TTL 语义。
//
// 变更记录（production-mock-purge T027）：原本地 TtlMockBackend（mock）已移除，
// 替换为真实 DashMapMemoryBackend；集成/e2e 禁止 mock。

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::{dashmap_memory, CacheReader, CacheWriter, DashMapMemoryBackend, MokaMemoryBackend};

// ============================================================================
// 跨后端 TTL 一致性回归测试
// ============================================================================

/// 构建三个真实后端：Moka（默认）+ 两个独立 DashMap 实例，返回 triple。
fn build_three_backends() -> (MokaMemoryBackend, DashMapMemoryBackend, DashMapMemoryBackend) {
    (
        MokaMemoryBackend::new(),
        dashmap_memory(),
        DashMapMemoryBackend::builder().capacity(10_000).build(),
    )
}

#[tokio::test]
async fn test_all_backends_set_with_ttl_expires_consistently() {
    // Moka / 两个 DashMap 分别 set 50ms TTL，等 100ms，都返回 None
    let (moka, dashmap, dashmap2) = build_three_backends();

    moka.set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_millis(50)))
        .await
        .unwrap();
    dashmap
        .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_millis(50)))
        .await
        .unwrap();
    dashmap2
        .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_millis(50)))
        .await
        .unwrap();

    // 立即查询三者都应返回 Some
    assert_eq!(moka.get("k").await.unwrap(), Some(b"v".to_vec()));
    assert_eq!(dashmap.get("k").await.unwrap(), Some(b"v".to_vec()));
    assert_eq!(dashmap2.get("k").await.unwrap(), Some(b"v".to_vec()));

    // 等 100ms 让 TTL 过期
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 两个 DashMap 都是 lazy 过期，立即查询应返回 None
    assert_eq!(dashmap.get("k").await.unwrap(), None, "dashmap should expire");
    assert_eq!(dashmap2.get("k").await.unwrap(), None, "dashmap should expire");

    // Moka 异步清理可能略有延迟，循环等待最多 500ms
    let mut moka_expired = false;
    for _ in 0..10 {
        if moka.get("k").await.unwrap().is_none() {
            moka_expired = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(moka_expired, "moka should expire after TTL");

    // 三者都过期后，再次确认仍为 None
    assert_eq!(dashmap.get("k").await.unwrap(), None);
    assert_eq!(dashmap2.get("k").await.unwrap(), None);
}

#[tokio::test]
async fn test_all_backends_ttl_returns_remaining_consistently() {
    // 三个真实后端分别 set 60s TTL，立即 ttl(key) 都返回 Some(d)，
    // 且 58s < d <= 60s
    let (moka, dashmap, dashmap2) = build_three_backends();

    moka.set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_secs(60)))
        .await
        .unwrap();
    dashmap
        .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_secs(60)))
        .await
        .unwrap();
    dashmap2
        .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_secs(60)))
        .await
        .unwrap();

    let moka_ttl = moka.ttl("k").await.unwrap().expect("moka ttl should be Some");
    let dashmap_ttl = dashmap.ttl("k").await.unwrap().expect("dashmap ttl should be Some");
    let dashmap2_ttl = dashmap2.ttl("k").await.unwrap().expect("dashmap ttl should be Some");

    let lower = Duration::from_secs(58);
    let upper = Duration::from_secs(60);

    for (name, d) in [("moka", moka_ttl), ("dashmap", dashmap_ttl), ("dashmap2", dashmap2_ttl)] {
        assert!(
            d > lower && d <= upper,
            "{} ttl={} should be in (58s, 60s]",
            name,
            d.as_secs_f64()
        );
    }
}

#[tokio::test]
async fn test_all_backends_expire_returns_true_consistently() {
    // 三个真实后端分别 set 60s，然后 expire 120s，都返回 Ok(true)
    let (moka, dashmap, dashmap2) = build_three_backends();

    moka.set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_secs(60)))
        .await
        .unwrap();
    dashmap
        .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_secs(60)))
        .await
        .unwrap();
    dashmap2
        .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_secs(60)))
        .await
        .unwrap();

    let moka_ok = moka.expire("k", Duration::from_secs(120)).await.unwrap();
    let dashmap_ok = dashmap.expire("k", Duration::from_secs(120)).await.unwrap();
    let dashmap2_ok = dashmap2.expire("k", Duration::from_secs(120)).await.unwrap();

    assert!(moka_ok, "moka expire should return true for existing key");
    assert!(dashmap_ok, "dashmap expire should return true for existing key");
    assert!(dashmap2_ok, "dashmap expire should return true for existing key");

    // 验证 expire 后 ttl 反映新的剩余时间（> 118s）
    let moka_ttl = moka
        .ttl("k")
        .await
        .unwrap()
        .expect("moka ttl should be Some after expire");
    let dashmap_ttl = dashmap
        .ttl("k")
        .await
        .unwrap()
        .expect("dashmap ttl should be Some after expire");
    let dashmap2_ttl = dashmap2
        .ttl("k")
        .await
        .unwrap()
        .expect("dashmap ttl should be Some after expire");

    let threshold = Duration::from_secs(118);
    for (name, d) in [("moka", moka_ttl), ("dashmap", dashmap_ttl), ("dashmap2", dashmap2_ttl)] {
        assert!(
            d > threshold,
            "{} ttl={} should be > 118s after expire(120s)",
            name,
            d.as_secs_f64()
        );
    }
}
