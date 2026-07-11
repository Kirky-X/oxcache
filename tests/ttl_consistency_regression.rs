// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// tests/ttl_consistency_regression.rs
//
// 跨后端 TTL 一致性回归测试 (spec: universal-per-entry-ttl)
//
// 验证 Moka / DashMap / Mock 三个后端在 set(ttl=Some) / ttl(key) / expire(key, ttl)
// 行为上一致。这是任务组 4 的跨后端回归套件，防止后续重构破坏 TTL 语义。
//
// 注意：src/backend/memory/mock.rs 的 MockBackend 是 #[cfg(test)] 类型，无法在
// 集成测试中访问。本文件定义本地 TtlMockBackend 作为第三个后端，行为与
// src/backend/memory/mock.rs 对齐（lazy 过期 + Instant 跟踪）。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use oxcache::backend::{
    BackendScore, CacheConnector, CacheReader, CacheWriter, DashMapMemoryBackend, MokaMemoryBackend,
};
use tokio::sync::RwLock;

// ============================================================================
// 本地 TtlMockBackend：支持 per-entry TTL 的 mock 后端
// ============================================================================

/// 单条缓存条目：(value, expires_at)，`None` 表示永不过期。
type TtlEntry = (Vec<u8>, Option<Instant>);

/// 支持 per-entry TTL 的本地 MockBackend（行为对齐 src/backend/memory/mock.rs）。
///
/// 内部存储 `(value, expires_at)`：`None` 表示永不过期，`Some(Instant)` 表示
/// 在该时刻过期（get 时 lazy 校验并清理）。
#[derive(Clone)]
struct TtlMockBackend {
    name: &'static str,
    score: u8,
    data: Arc<RwLock<HashMap<String, TtlEntry>>>,
}

impl TtlMockBackend {
    fn new(name: &'static str, score: u8) -> Self {
        Self {
            name,
            score,
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl BackendScore for TtlMockBackend {
    fn score(&self) -> u8 {
        self.score
    }
    fn is_persistent(&self) -> bool {
        false
    }
    fn backend_name(&self) -> &'static str {
        self.name
    }
}

#[async_trait]
impl CacheReader for TtlMockBackend {
    async fn get(&self, key: &str) -> oxcache::error::Result<Option<Vec<u8>>> {
        let now = Instant::now();
        let mut data = self.data.write().await;
        if let Some((_v, expires_at)) = data.get(key) {
            if let Some(exp) = expires_at {
                if *exp <= now {
                    data.remove(key);
                    return Ok(None);
                }
            }
            return Ok(Some(data.get(key).unwrap().0.clone()));
        }
        Ok(None)
    }

    async fn exists(&self, key: &str) -> oxcache::error::Result<bool> {
        let now = Instant::now();
        let mut data = self.data.write().await;
        if let Some((_v, expires_at)) = data.get(key) {
            if let Some(exp) = expires_at {
                if *exp <= now {
                    data.remove(key);
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        Ok(false)
    }

    async fn ttl(&self, key: &str) -> oxcache::error::Result<Option<Duration>> {
        let now = Instant::now();
        let data = self.data.read().await;
        if let Some((_v, Some(exp))) = data.get(key) {
            return Ok(exp.checked_duration_since(now));
        }
        Ok(None)
    }

    async fn len(&self) -> oxcache::error::Result<u64> {
        Ok(self.data.read().await.len() as u64)
    }

    async fn is_empty(&self) -> oxcache::error::Result<bool> {
        Ok(self.data.read().await.is_empty())
    }

    async fn capacity(&self) -> oxcache::error::Result<u64> {
        Ok(0)
    }

    async fn stats(&self) -> oxcache::error::Result<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), self.name.to_string());
        Ok(stats)
    }
}

#[async_trait]
impl CacheWriter for TtlMockBackend {
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> oxcache::error::Result<()> {
        let expires_at = ttl.map(|d| Instant::now() + d);
        self.data.write().await.insert(key.to_string(), (value, expires_at));
        Ok(())
    }

    async fn delete(&self, key: &str) -> oxcache::error::Result<()> {
        self.data.write().await.remove(key);
        Ok(())
    }

    async fn clear(&self) -> oxcache::error::Result<()> {
        self.data.write().await.clear();
        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> oxcache::error::Result<bool> {
        let mut data = self.data.write().await;
        if data.contains_key(key) {
            data.get_mut(key).unwrap().1 = Some(Instant::now() + ttl);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[async_trait]
impl CacheConnector for TtlMockBackend {
    async fn health_check(&self) -> oxcache::error::Result<()> {
        Ok(())
    }
    async fn shutdown(&self) {}
    fn backend_kind(&self) -> oxcache::backend::interface::BackendKind {
        oxcache::backend::interface::BackendKind::Mock
    }
}

// CacheBackend 通过 blanket impl 自动实现

// ============================================================================
// 跨后端 TTL 一致性回归测试
// ============================================================================

/// 构建三个后端：Moka / DashMap / TtlMock，返回 triple。
fn build_three_backends() -> (MokaMemoryBackend, DashMapMemoryBackend, TtlMockBackend) {
    (
        MokaMemoryBackend::new(),
        DashMapMemoryBackend::new(),
        TtlMockBackend::new("mock", 30),
    )
}

#[tokio::test]
async fn test_all_backends_set_with_ttl_expires_consistently() {
    // Moka / DashMap / Mock 三个后端分别 set 50ms TTL，等 100ms，都返回 None
    let (moka, dashmap, mock) = build_three_backends();

    moka.set("k", b"v".to_vec(), Some(Duration::from_millis(50)))
        .await
        .unwrap();
    dashmap
        .set("k", b"v".to_vec(), Some(Duration::from_millis(50)))
        .await
        .unwrap();
    mock.set("k", b"v".to_vec(), Some(Duration::from_millis(50)))
        .await
        .unwrap();

    // 立即查询三者都应返回 Some
    assert_eq!(moka.get("k").await.unwrap(), Some(b"v".to_vec()));
    assert_eq!(dashmap.get("k").await.unwrap(), Some(b"v".to_vec()));
    assert_eq!(mock.get("k").await.unwrap(), Some(b"v".to_vec()));

    // 等 100ms 让 TTL 过期
    tokio::time::sleep(Duration::from_millis(100)).await;

    // DashMap / Mock 是 lazy 过期，立即查询应返回 None
    assert_eq!(dashmap.get("k").await.unwrap(), None, "dashmap should expire");
    assert_eq!(mock.get("k").await.unwrap(), None, "mock should expire");

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
    assert_eq!(mock.get("k").await.unwrap(), None);
}

#[tokio::test]
async fn test_all_backends_ttl_returns_remaining_consistently() {
    // Moka / DashMap / Mock 三个后端分别 set 60s TTL，立即 ttl(key) 都返回 Some(d)，
    // 且 58s < d <= 60s
    let (moka, dashmap, mock) = build_three_backends();

    moka.set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();
    dashmap
        .set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();
    mock.set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();

    let moka_ttl = moka.ttl("k").await.unwrap().expect("moka ttl should be Some");
    let dashmap_ttl = dashmap.ttl("k").await.unwrap().expect("dashmap ttl should be Some");
    let mock_ttl = mock.ttl("k").await.unwrap().expect("mock ttl should be Some");

    let lower = Duration::from_secs(58);
    let upper = Duration::from_secs(60);

    for (name, d) in [("moka", moka_ttl), ("dashmap", dashmap_ttl), ("mock", mock_ttl)] {
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
    // Moka / DashMap / Mock 三个后端分别 set 60s，然后 expire 120s，都返回 Ok(true)
    let (moka, dashmap, mock) = build_three_backends();

    moka.set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();
    dashmap
        .set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();
    mock.set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();

    let moka_ok = moka.expire("k", Duration::from_secs(120)).await.unwrap();
    let dashmap_ok = dashmap.expire("k", Duration::from_secs(120)).await.unwrap();
    let mock_ok = mock.expire("k", Duration::from_secs(120)).await.unwrap();

    assert!(moka_ok, "moka expire should return true for existing key");
    assert!(dashmap_ok, "dashmap expire should return true for existing key");
    assert!(mock_ok, "mock expire should return true for existing key");

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
    let mock_ttl = mock
        .ttl("k")
        .await
        .unwrap()
        .expect("mock ttl should be Some after expire");

    let threshold = Duration::from_secs(118);
    for (name, d) in [("moka", moka_ttl), ("dashmap", dashmap_ttl), ("mock", mock_ttl)] {
        assert!(
            d > threshold,
            "{} ttl={} should be > 118s after expire(120s)",
            name,
            d.as_secs_f64()
        );
    }
}
