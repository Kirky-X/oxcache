//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis L2 缓存性能基准测试

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxcache::backend::client::RedisBackend;
use oxcache::backend::CacheBackend;
use std::time::Duration;
use tokio::runtime::Runtime;

// ============================= Redis L2 缓存基准测试 =============================

/// 基准测试Redis的SET操作性能
fn bench_redis_set(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("redis_set", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "bench:redis:set:{}",
                std::time::SystemTime::now().elapsed().unwrap().as_nanos()
            );
            let value = vec![0u8; 100];
            if let Ok(backend) = RedisBackend::new("redis://127.0.0.1:6381").await {
                let _ = backend
                    .set(black_box(&key), black_box(value), Some(Duration::from_secs(300)))
                    .await;
            }
        });
    });
}

/// 基准测试Redis的GET操作性能
fn bench_redis_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Pre-populate test data
    rt.block_on(async {
        if let Ok(backend) = RedisBackend::new("redis://127.0.0.1:6381").await {
            let key = "bench:redis:get:test";
            let value = vec![0u8; 100];
            let _ = backend.set(key, value, Some(Duration::from_secs(300))).await;
        }
    });

    c.bench_function("redis_get", |b| {
        b.to_async(&rt).iter(|| async {
            if let Ok(backend) = RedisBackend::new("redis://127.0.0.1:6381").await {
                let _ = backend.get(black_box("bench:redis:get:test")).await;
            }
        });
    });
}

/// 基准测试Redis不同数据大小的SET性能
fn bench_redis_different_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("redis_different_sizes");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let key = format!("bench:redis:size:{}", size);
                let value = vec![0u8; size];
                if let Ok(backend) = RedisBackend::new("redis://127.0.0.1:6381").await {
                    let _ = backend
                        .set(black_box(&key), black_box(value), Some(Duration::from_secs(300)))
                        .await;
                }
            });
        });
    }

    group.finish();
}

/// 基准测试Redis的TTL操作性能
fn bench_redis_ttl(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("redis_ttl", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "bench:redis:ttl:{}",
                std::time::SystemTime::now().elapsed().unwrap().as_nanos()
            );
            let value = vec![0u8; 100];
            if let Ok(backend) = RedisBackend::new("redis://127.0.0.1:6381").await {
                let _ = backend
                    .set(black_box(&key), black_box(value), Some(Duration::from_secs(60)))
                    .await;
            }
        });
    });
}

criterion_group!(
    benches,
    bench_redis_set,
    bench_redis_get,
    bench_redis_different_sizes,
    bench_redis_ttl
);
criterion_main!(benches);
