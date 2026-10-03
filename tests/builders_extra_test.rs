// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 分层构建器与 Cache 构建路径覆盖：`L1Builder` 全 setter 与装饰器、
//! `L2Builder` 的 custom/校验/装饰器、`ttl_jitter` 钳制、stale+sync 冲突
//! 显性拒绝、event_publisher 注入、serialization_format 切换、Dual 槽
//! sync 面装配。

#![cfg(feature = "full")]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use oxcache::OxCacheError;
use oxcache::cache::{CacheBuilder, ChainBuilder, L1Builder, L2Builder};
use oxcache::features::stale::StalePolicy;
use oxcache::{CacheEvent, EventPublisher};

// ============================================================================
// 测试替身：事件发布计数器
// ============================================================================

#[derive(Default)]
struct CountingPublisher {
    events: AtomicU32,
}

#[async_trait]
impl EventPublisher for CountingPublisher {
    async fn publish(&self, _event: CacheEvent) -> Result<(), OxCacheError> {
        self.events.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

// ============================================================================
// L1Builder：全 setter + 装饰器包装
// ============================================================================

#[tokio::test]
async fn l1_builder_full_surface() {
    use oxcache::backend::{CacheReader, CacheWriter};

    // moka 分支 + 全 setter + 装饰器
    let l1 = L1Builder::new()
        .capacity(2_000)
        .ttl(Duration::from_secs(60))
        .tti(Duration::from_secs(30))
        .moka()
        .score(90)
        .decorate(|inner| inner) // 恒等装饰器覆盖 decorate 管线
        .build();
    CacheWriter::set(l1.as_ref(), Arc::from("l1"), Arc::new(b"v".to_vec()), None)
        .await
        .unwrap();
    assert_eq!(
        CacheReader::get(l1.as_ref(), "l1")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"v"[..])
    );

    // dashmap 分支 + capacity
    let l1d = L1Builder::new()
        .dashmap()
        .capacity(1_000)
        .ttl(Duration::from_secs(30))
        .build();
    assert_eq!(CacheReader::len(l1d.as_ref()).await.unwrap(), 0);
}

#[tokio::test]
async fn l2_builder_custom_backend_and_defaults() {
    use oxcache::backend::{CacheReader, CacheWriter, DashMapMemoryBackend};

    // custom 后端 + 装饰器 + persistent/score
    let l2 = L2Builder::new()
        .custom(Arc::new(DashMapMemoryBackend::default()))
        .score(40)
        .persistent(false)
        .decorate(|inner| inner)
        .build()
        .await
        .unwrap();
    CacheWriter::set(l2.as_ref(), Arc::from("l2"), Arc::new(b"w".to_vec()), None)
        .await
        .unwrap();
    assert_eq!(
        CacheReader::get(l2.as_ref(), "l2")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"w"[..])
    );

    // 既无 custom 也无 redis url → 显性报错
    let refused = L2Builder::new().build().await;
    let err = match refused {
        Err(e) => e,
        Ok(_) => panic!("L2Builder 无后端必须显性报错"),
    };
    assert!(err.to_string().contains("requires"));
}

#[tokio::test(flavor = "multi_thread")]
async fn l2_builder_redis_unreachable_fails_explicitly() {
    // redis url 指向不可达端口 → Connection 错误透传
    let l2 = L2Builder::new().redis("redis://127.0.0.1:1");
    assert!(l2.build().await.is_err());
}

// ============================================================================
// ChainBuilder 串联（l1 + l2 完整构建路径）
// ============================================================================

#[tokio::test]
async fn chain_builder_with_tiered_builders() {
    use oxcache::backend::{CacheReader, DashMapMemoryBackend};

    let chain = ChainBuilder::new()
        .l1(L1Builder::new()
            .capacity(500)
            .ttl(Duration::from_secs(30))
            .tti(Duration::from_secs(10))
            .dashmap())
        .l2(L2Builder::new()
            .custom(Arc::new(DashMapMemoryBackend::default()))
            .persistent(true))
        .default_time_to_live(Duration::from_secs(45))
        .read_strategy(oxcache::cache::chain::ChainReadStrategy::Race)
        .enable_backfill()
        .build()
        .await
        .unwrap();
    let _ = CacheReader::len(&chain).await.unwrap();
}

// ============================================================================
// Cache 构建路径：ttl_jitter 钳制 / stale+sync 冲突 / event_publisher /
// serialization_format / Dual 槽 sync 面
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn ttl_jitter_nan_and_overflow_are_clamped() {
    // NaN → 0.0；>1.0 → 1.0（构建不 panic、写入读取正常）
    for factor in [f64::NAN, 1.5, 0.5] {
        let cache: oxcache::Cache<String, String> = CacheBuilder::default()
            .ttl_jitter(factor)
            .build()
            .await
            .unwrap();
        cache
            .set_with_ttl(
                &"j".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(30)),
            )
            .await
            .unwrap();
        assert_eq!(
            cache.get(&"j".to_string()).await.unwrap().as_deref(),
            Some("v")
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_ttl_with_sync_mode_rejected_at_build() {
    let result = CacheBuilder::<String, String>::default()
        .sync_mode(true)
        .stale_ttl(Duration::from_secs(30))
        .build()
        .await;
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("stale_ttl + sync_mode 必须构建期拒绝"),
    };
    assert!(err.to_string().contains("stale_ttl cannot be combined"));
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_build_with_event_publisher_and_offload() {
    let publisher = Arc::new(CountingPublisher::default());
    // stale 窗口拉宽到 2s：TTL 100ms 过期后有充足余量落在 Stale 态，
    // 避免 llvm-cov 插桩/调度抖动下滑过 180ms 级窄窗口误入 Expired 分支
    let cache: oxcache::Cache<String, String> = CacheBuilder::default()
        .stale_ttl(Duration::from_secs(2))
        .stale_policy(StalePolicy::OffloadRevalidate)
        .event_publisher(Arc::clone(&publisher) as Arc<dyn EventPublisher>)
        .build()
        .await
        .unwrap();

    // 写入 → 过期进 stale 窗口 → get_or_refresh 触发事件发布器与 offload
    cache
        .set_with_ttl(
            &"ev".to_string(),
            &"v1".to_string(),
            Some(Duration::from_millis(100)),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(250)).await;
    let v = cache
        .get_or_refresh(&"ev".to_string(), Some(Duration::from_millis(60)), {
            let publisher = Arc::clone(&publisher);
            move || {
                let _ = &publisher;
                async move { Ok("v2".to_string()) }
            }
        })
        .await
        .unwrap();
    assert_eq!(v, "v1", "OffloadRevalidate：stale 命中返回旧值");
    // 事件发布器至少收到 stale 命中/过期事件
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if publisher.events.load(Ordering::SeqCst) > 0 {
            break;
        }
    }
    assert!(
        publisher.events.load(Ordering::SeqCst) > 0,
        "event_publisher 必须收到过期事件"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn serialization_format_and_dual_slot_sync_surface() {
    use oxcache::infra::serialization::SerializationFormat;

    // serialization_format 切换（Json 恒可用）
    let cache: oxcache::Cache<String, String> = CacheBuilder::default()
        .serialization_format(SerializationFormat::Json)
        .build()
        .await
        .unwrap();
    cache
        .set(&"sf".to_string(), &"v".to_string())
        .await
        .unwrap();
    assert_eq!(
        cache.get(&"sf".to_string()).await.unwrap().as_deref(),
        Some("v")
    );

    // Dual 槽 + sync_mode：默认 moka 后端保留原生同步面
    let cache: oxcache::Cache<String, String> = CacheBuilder::default()
        .sync_mode(true)
        .build()
        .await
        .unwrap();
    cache
        .set_sync(&"ds".to_string(), &"sv".to_string())
        .unwrap();
    assert_eq!(
        cache.get_sync(&"ds".to_string()).unwrap().as_deref(),
        Some("sv")
    );
}
