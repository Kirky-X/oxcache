// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Dragonfly vs Redis 性能基准对比测试
//!
//! 在 GET / SET / MGET 场景下对比 Dragonfly 与 Redis 的吞吐差异。
//!
//! # 运行方式
//!
//! 需要同时启动 Redis 和 Dragonfly 服务：
//!
//! ```bash
//! # Redis
//! docker run -d -p 6379:6379 redis:7-alpine
//! # Dragonfly
//! docker run -d -p 6380:6379 dragonflydb/dragonfly:v1.27.1
//!
//! cargo bench --bench dragonfly_benchmark --features dragonfly
//! ```
//!
//! 可通过环境变量覆盖地址：
//! - `OXCACHE_REDIS_URL`（默认 `redis://127.0.0.1:6379`）
//! - `OXCACHE_DRAGONFLY_URL`（默认 `redis://127.0.0.1:6380`）

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use oxcache::backend::memory::RedisBackend;
use oxcache::backend::DragonflyBackend;
use oxcache::backend::{CacheReader, CacheWriter};
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

// ============================= 辅助函数 =============================

/// 获取 Redis URL
fn get_redis_url() -> String {
    unsafe {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
    };
    std::env::var("OXCACHE_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

/// 获取 Dragonfly URL
fn get_dragonfly_url() -> String {
    unsafe {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
    };
    std::env::var("OXCACHE_DRAGONFLY_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6380".to_string())
}

// ============================= SET 对比 =============================

fn bench_set_comparison(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url = get_redis_url();
    let dragonfly_url = get_dragonfly_url();

    let redis = rt.block_on(async {
        RedisBackend::new(&redis_url).await.expect("Failed to connect to Redis")
    });
    let dragonfly = rt.block_on(async {
        DragonflyBackend::new(&dragonfly_url, 8)
            .await
            .expect("Failed to connect to Dragonfly")
    });

    let mut group = c.benchmark_group("set_comparison");
    group.throughput(Throughput::Bytes(100));

    group.bench_function("redis_set", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "bench:set:{}",
                std::time::SystemTime::now().elapsed().unwrap().as_nanos()
            );
            let _ = redis
                .set(
                    Arc::from(black_box(&key).as_str()),
                    Arc::new(black_box(vec![0u8; 100])),
                    Some(Duration::from_secs(300)),
                )
                .await;
        });
    });

    group.bench_function("dragonfly_set", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "bench:set:{}",
                std::time::SystemTime::now().elapsed().unwrap().as_nanos()
            );
            let _ = dragonfly
                .set(
                    Arc::from(black_box(&key).as_str()),
                    Arc::new(black_box(vec![0u8; 100])),
                    Some(Duration::from_secs(300)),
                )
                .await;
        });
    });

    group.finish();
}

// ============================= GET 对比 =============================

fn bench_get_comparison(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url = get_redis_url();
    let dragonfly_url = get_dragonfly_url();

    let redis = rt.block_on(async {
        let backend = RedisBackend::new(&redis_url).await.expect("Failed to connect to Redis");
        let _ = backend
            .set(
                Arc::from("bench:get:test"),
                Arc::new(vec![0u8; 100]),
                Some(Duration::from_secs(300)),
            )
            .await;
        backend
    });
    let dragonfly = rt.block_on(async {
        let backend = DragonflyBackend::new(&dragonfly_url, 8)
            .await
            .expect("Failed to connect to Dragonfly");
        let _ = backend
            .set(
                Arc::from("bench:get:test"),
                Arc::new(vec![0u8; 100]),
                Some(Duration::from_secs(300)),
            )
            .await;
        backend
    });

    let mut group = c.benchmark_group("get_comparison");

    group.bench_function("redis_get", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = redis.get(black_box("bench:get:test")).await;
        });
    });

    group.bench_function("dragonfly_get", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = dragonfly.get(black_box("bench:get:test")).await;
        });
    });

    group.finish();
}

// ============================= MGET 对比 =============================

fn bench_mget_comparison(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url = get_redis_url();
    let dragonfly_url = get_dragonfly_url();

    let key_count = 10;
    let keys: Vec<String> = (0..key_count).map(|i| format!("bench:mget:{i}")).collect();

    let redis = rt.block_on(async {
        let backend = RedisBackend::new(&redis_url).await.expect("Failed to connect to Redis");
        for (i, key) in keys.iter().enumerate() {
            let _ = backend
                .set(
                    Arc::from(key.as_str()),
                    Arc::new(vec![i as u8; 100]),
                    Some(Duration::from_secs(300)),
                )
                .await;
        }
        backend
    });
    let dragonfly = rt.block_on(async {
        let backend = DragonflyBackend::new(&dragonfly_url, 8)
            .await
            .expect("Failed to connect to Dragonfly");
        for (i, key) in keys.iter().enumerate() {
            let _ = backend
                .set(
                    Arc::from(key.as_str()),
                    Arc::new(vec![i as u8; 100]),
                    Some(Duration::from_secs(300)),
                )
                .await;
        }
        backend
    });

    let mut group = c.benchmark_group("mget_comparison");
    group.throughput(Throughput::Elements(key_count as u64));

    group.bench_function("redis_mget_10", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = redis.get_many(black_box(&keys)).await;
        });
    });

    group.bench_function("dragonfly_mget_10", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = dragonfly.get_many(black_box(&keys)).await;
        });
    });

    group.finish();
}

// ============================= 不同数据大小 SET 对比 =============================

fn bench_set_size_comparison(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url = get_redis_url();
    let dragonfly_url = get_dragonfly_url();

    let redis = rt.block_on(async {
        RedisBackend::new(&redis_url).await.expect("Failed to connect to Redis")
    });
    let dragonfly = rt.block_on(async {
        DragonflyBackend::new(&dragonfly_url, 8)
            .await
            .expect("Failed to connect to Dragonfly")
    });

    let mut group = c.benchmark_group("set_size_comparison");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("redis_set", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let key = format!("bench:size:{size}");
                let _ = redis
                    .set(
                        Arc::from(black_box(&key).as_str()),
                        Arc::new(black_box(vec![0u8; size])),
                        Some(Duration::from_secs(300)),
                    )
                    .await;
            });
        });

        group.bench_with_input(
            BenchmarkId::new("dragonfly_set", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let key = format!("bench:size:{size}");
                    let _ = dragonfly
                        .set(
                            Arc::from(black_box(&key).as_str()),
                            Arc::new(black_box(vec![0u8; size])),
                            Some(Duration::from_secs(300)),
                        )
                        .await;
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_set_comparison,
    bench_get_comparison,
    bench_mget_comparison,
    bench_set_size_comparison,
);
criterion_main!(benches);
