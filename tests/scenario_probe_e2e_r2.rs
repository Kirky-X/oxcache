// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! E2E 第 2 轮（场景 26-50）补充探针
//!
//! 仅覆盖既有测试面之外的场景：失效应用失败计数（38）、Pub/Sub 构造
//! fail-fast（41）、内存版并发 CAS 单胜者（42 补并发面）、键枚举上限
//! moka/dashmap/redis 边界（48 补齐）、链健康检查慢后端超时（27 补超时面）。
//! 其余场景直接跑既有测试，不经本文件。

#![cfg(feature = "full")]

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use oxcache::OxCacheError;
use oxcache::backend::memory::MokaMemoryBackend;
use oxcache::backend::{
    AtomicCacheWriter, BackendKind, CacheBackend, CacheConnector, CacheReader, CacheSetItem,
    CacheWriter,
};
use oxcache::features::invalidation::transport::RedisPubSubTransport;
use oxcache::features::invalidation::{
    InMemoryPubSubTransport, InvalidationBus, InvalidationConfig,
};

/// delete 恒败的探测后端：失效总线 apply 失败路径（场景 38）
struct FailingDeleteBackend {
    inner: MokaMemoryBackend,
}

#[async_trait]
impl CacheReader for FailingDeleteBackend {
    async fn get(&self, key: &str) -> oxcache::OxCacheResult<Option<Vec<u8>>> {
        CacheReader::get(&self.inner, key).await
    }
    async fn exists(&self, key: &str) -> oxcache::OxCacheResult<bool> {
        CacheReader::exists(&self.inner, key).await
    }
    async fn ttl(&self, key: &str) -> oxcache::OxCacheResult<Option<Duration>> {
        CacheReader::ttl(&self.inner, key).await
    }
    async fn len(&self) -> oxcache::OxCacheResult<u64> {
        CacheReader::len(&self.inner).await
    }
    async fn capacity(&self) -> oxcache::OxCacheResult<u64> {
        CacheReader::capacity(&self.inner).await
    }
    async fn stats(&self) -> oxcache::OxCacheResult<std::collections::HashMap<String, String>> {
        CacheReader::stats(&self.inner).await
    }
}

#[async_trait]
impl CacheWriter for FailingDeleteBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> oxcache::OxCacheResult<()> {
        CacheWriter::set(&self.inner, key, value, ttl).await
    }
    async fn delete(&self, _key: &str) -> oxcache::OxCacheResult<()> {
        Err(OxCacheError::Operation(
            "probe: delete fault injected".into(),
        ))
    }
    async fn clear(&self) -> oxcache::OxCacheResult<()> {
        CacheWriter::clear(&self.inner).await
    }
    async fn expire(&self, key: &str, ttl: Duration) -> oxcache::OxCacheResult<bool> {
        CacheWriter::expire(&self.inner, key, ttl).await
    }
}

#[async_trait]
impl CacheConnector for FailingDeleteBackend {
    async fn health_check(&self) -> oxcache::OxCacheResult<()> {
        CacheConnector::health_check(&self.inner).await
    }
    async fn shutdown(&self) {
        CacheConnector::shutdown(&self.inner).await
    }
    fn backend_kind(&self) -> BackendKind {
        CacheConnector::backend_kind(&self.inner)
    }
}

/// health_check 睡过链健康 5s 超时窗的探测后端（场景 27 超时面）
struct SlowHealthBackend {
    inner: MokaMemoryBackend,
    delay: Duration,
}

#[async_trait]
impl CacheReader for SlowHealthBackend {
    async fn get(&self, key: &str) -> oxcache::OxCacheResult<Option<Vec<u8>>> {
        CacheReader::get(&self.inner, key).await
    }
    async fn exists(&self, key: &str) -> oxcache::OxCacheResult<bool> {
        CacheReader::exists(&self.inner, key).await
    }
    async fn ttl(&self, key: &str) -> oxcache::OxCacheResult<Option<Duration>> {
        CacheReader::ttl(&self.inner, key).await
    }
    async fn len(&self) -> oxcache::OxCacheResult<u64> {
        CacheReader::len(&self.inner).await
    }
    async fn capacity(&self) -> oxcache::OxCacheResult<u64> {
        CacheReader::capacity(&self.inner).await
    }
    async fn stats(&self) -> oxcache::OxCacheResult<std::collections::HashMap<String, String>> {
        CacheReader::stats(&self.inner).await
    }
}

#[async_trait]
impl CacheWriter for SlowHealthBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> oxcache::OxCacheResult<()> {
        CacheWriter::set(&self.inner, key, value, ttl).await
    }
    async fn delete(&self, key: &str) -> oxcache::OxCacheResult<()> {
        CacheWriter::delete(&self.inner, key).await
    }
    async fn clear(&self) -> oxcache::OxCacheResult<()> {
        CacheWriter::clear(&self.inner).await
    }
    async fn expire(&self, key: &str, ttl: Duration) -> oxcache::OxCacheResult<bool> {
        CacheWriter::expire(&self.inner, key, ttl).await
    }
}

#[async_trait]
impl CacheConnector for SlowHealthBackend {
    async fn health_check(&self) -> oxcache::OxCacheResult<()> {
        tokio::time::sleep(self.delay).await;
        CacheConnector::health_check(&self.inner).await
    }
    async fn shutdown(&self) {
        CacheConnector::shutdown(&self.inner).await
    }
    fn backend_kind(&self) -> BackendKind {
        CacheConnector::backend_kind(&self.inner)
    }
}

impl oxcache::backend::BackendScore for SlowHealthBackend {
    fn score(&self) -> u8 {
        50
    }
    fn is_persistent(&self) -> bool {
        false
    }
    fn backend_name(&self) -> &'static str {
        "slow-health-probe"
    }
}

fn counter_value(export: &str, name: &str) -> f64 {
    export
        .lines()
        .filter(|l| l.starts_with(name) && !l.starts_with('#'))
        .filter_map(|l| l.rsplit(' ').next()?.parse::<f64>().ok())
        .sum()
}

/// 场景 38：跨实例失效应用失败计入 oxcache_invalidation_apply_failed_total
/// 并输出 tracing warn，不再静默空操作。
#[tokio::test]
async fn s38_invalidation_apply_failure_is_counted_and_logged() {
    use std::sync::Mutex;

    #[derive(Clone)]
    struct LogSink(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for LogSink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let log_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let sink = LogSink(log_buf.clone());
    let installed = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_writer(move || sink.clone())
        .try_init()
        .is_ok();

    let transport = Arc::new(InMemoryPubSubTransport::new());
    let bus_a = InvalidationBus::new(
        transport.clone(),
        InvalidationConfig::new("instance-a").with_channel("probe38"),
    );
    let bus_b = InvalidationBus::new(
        transport,
        InvalidationConfig::new("instance-b").with_channel("probe38"),
    );
    let l1: Arc<dyn CacheBackend> = Arc::new(FailingDeleteBackend {
        inner: MokaMemoryBackend::new(),
    });
    let handle = bus_b.spawn_listener(l1).await.expect("spawn listener");
    tokio::time::sleep(Duration::from_millis(100)).await;

    bus_a.invalidate_key("user:probe38").await.expect("publish");

    // 应用失败路径异步生效，轮询计数出现
    let mut seen = 0.0f64;
    for _ in 0..50 {
        let export = oxcache::infra::metrics::unified::GLOBAL_UNIFIED_METRICS.export_prometheus();
        seen = counter_value(&export, "oxcache_invalidation_apply_failed_total");
        if seen >= 1.0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        seen >= 1.0,
        "apply failure must be counted in oxcache_invalidation_apply_failed_total, got {seen}"
    );

    handle.stop();
    if installed {
        let guard = log_buf.lock().unwrap();
        let log = String::from_utf8_lossy(&guard);
        assert!(
            log.contains("oxcache::invalidation"),
            "apply failure must emit a tracing warn under the invalidation target, got: {log}"
        );
    } else {
        println!(
            "global tracing subscriber already installed; warn capture skipped (counter asserted)"
        );
    }
}

/// 场景 41：RedisPubSubTransport::new 构造 fail-fast——URL 非法与端口
/// 不可达都立即 Err，不留假成功实例。
#[tokio::test]
async fn s41_pubsub_transport_new_fails_fast() {
    let started = Instant::now();
    let bad_url = RedisPubSubTransport::new("not-a-valid-url").await;
    let bad_url_elapsed = started.elapsed();
    assert!(bad_url.is_err(), "invalid URL must fail construction");
    assert!(
        bad_url_elapsed < Duration::from_secs(5),
        "invalid URL must fail fast, took {bad_url_elapsed:?}"
    );

    let started = Instant::now();
    let unreachable = RedisPubSubTransport::new("redis://127.0.0.1:1").await;
    let unreachable_elapsed = started.elapsed();
    assert!(
        unreachable.is_err(),
        "unreachable port must fail construction"
    );
    assert!(
        unreachable_elapsed < Duration::from_secs(8),
        "unreachable endpoint must fail fast (connect window), took {unreachable_elapsed:?}"
    );
}

/// 场景 42（并发面）：读-改-写竞争下 compare_and_swap 仅一方成功，
/// 失败方返回 false，最终值与唯一胜者写入一致。
#[tokio::test]
async fn s42_concurrent_cas_has_single_winner() {
    let backend = Arc::new(MokaMemoryBackend::new());
    let key = "cas:probe42";
    CacheWriter::set(
        backend.as_ref(),
        Arc::from(key),
        Arc::new(b"v0".to_vec()),
        None,
    )
    .await
    .unwrap();

    let contenders = 8usize;
    let mut tasks = Vec::new();
    for i in 0..contenders {
        // 写入值必须互异且不等于当前值：CAS 允许「先后两次都成功」的合法
        // 状态迁移（expected 匹配即写），互异值才能断言唯一胜者
        let value = format!("v{}", i + 1);
        let backend = backend.clone();
        tasks.push(tokio::spawn(async move {
            AtomicCacheWriter::compare_and_swap(
                backend.as_ref(),
                key,
                Some(b"v0".as_slice()),
                value.clone().into_bytes(),
                None,
            )
            .await
            .map(|won| (won, value))
        }));
    }
    let mut winners = Vec::new();
    for task in tasks {
        let (won, value) = task.await.unwrap().unwrap();
        if won {
            winners.push(value);
        }
    }
    assert_eq!(
        winners.len(),
        1,
        "exactly one contender must win the CAS race, got {winners:?}"
    );
    let final_value = CacheReader::get(backend.as_ref(), key).await.unwrap();
    assert_eq!(
        final_value,
        Some(winners[0].clone().into_bytes()),
        "final value must equal the single winner's write"
    );
}

/// 场景 48（moka/dashmap 面）：键枚举上限「恰好 10000 正常、超限报错」。
#[tokio::test]
async fn s48_keys_limit_boundary_on_memory_backends() {
    async fn probe(backend: &(dyn CacheBackend + Send + Sync)) {
        let limit = oxcache::MAX_KEYS_RETURN;
        let items: Vec<CacheSetItem> = (0..=limit)
            .map(|i| {
                (
                    Arc::from(format!("k:{i:06}").as_str()),
                    Arc::new(vec![1u8]),
                    None,
                )
            })
            .collect();
        CacheWriter::set_many(backend, &items).await.unwrap();

        let err = CacheReader::keys(backend, "k:*").await.unwrap_err();
        assert!(
            err.to_string().contains(&limit.to_string()),
            "over-limit enumeration error must mention the limit, got {err}"
        );
        CacheWriter::clear(backend).await.unwrap();

        let items: Vec<CacheSetItem> = (0..limit)
            .map(|i| {
                (
                    Arc::from(format!("k:{i:06}").as_str()),
                    Arc::new(vec![1u8]),
                    None,
                )
            })
            .collect();
        CacheWriter::set_many(backend, &items).await.unwrap();
        assert_eq!(
            CacheReader::keys(backend, "k:*").await.unwrap().len(),
            limit
        );
    }

    probe(&MokaMemoryBackend::builder().capacity(20000).build()).await;
    // 默认容量 10000 会在第 10001 条写入时 FIFO 淘汰，干扰上限判定，
    // 容量须大于枚举上限
    let dashmap = oxcache::backend::memory::dashmap_memory_with_capacity(20000);
    probe(&dashmap).await;
}

/// 场景 48（redis 面）：上限口径与内存后端一致（独立容器，不共享栈）。
#[tokio::test]
async fn s48_redis_keys_limit_boundary() {
    use oxcache::backend::memory::RedisBackend;

    let Some((_container, redis_url)) =
        common::test_containers::container_or_skip("probe48(redis)", || {
            common::test_containers::start_redis_container()
        })
        .await
    else {
        return;
    };
    let backend = RedisBackend::builder()
        .connection_string(&redis_url)
        .retry_count(0)
        .build()
        .await
        .expect("connect redis container");

    let limit = oxcache::MAX_KEYS_RETURN;
    let prefix = format!("probe48:{}", uuid::Uuid::new_v4().simple());
    let items: Vec<CacheSetItem> = (0..=limit)
        .map(|i| {
            (
                Arc::from(format!("{prefix}:{i:06}").as_str()),
                Arc::new(vec![1u8]),
                None,
            )
        })
        .collect();
    CacheWriter::set_many(&backend, &items).await.unwrap();

    let err = CacheReader::keys(&backend, &format!("{prefix}:*"))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains(&limit.to_string()),
        "over-limit enumeration error must mention the limit, got {err}"
    );

    let deleted = CacheBackend::delete_by_pattern(&backend, &format!("{prefix}:*"))
        .await
        .unwrap();
    assert_eq!(deleted, (limit + 1) as u64, "cleanup must remove all keys");
}

/// 场景 27（超时面）：链健康检查对 5s 内不返回的后端整体报错。
#[tokio::test]
async fn s27_chain_health_check_times_out_slow_backend() {
    use oxcache::cache::{ChainCache, ChainLink};

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(MokaMemoryBackend::new()))
        .link(ChainLink::from_backend(SlowHealthBackend {
            inner: MokaMemoryBackend::new(),
            delay: Duration::from_secs(30),
        }))
        .build();

    let started = Instant::now();
    let result = chain.health_check().await;
    let elapsed = started.elapsed();
    assert!(
        result.is_err(),
        "slow-backend link must fail the chain health check"
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "per-link 5s timeout must bound the wait, took {elapsed:?}"
    );
}
