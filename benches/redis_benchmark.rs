//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis L2 缓存性能基准测试

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxcache::backend::memory::RedisBackend;
use oxcache::backend::{CacheReader, CacheWriter};
use std::time::Duration;
use tokio::runtime::Runtime;

// ============================= Redis L2 缓存基准测试 =============================

/// 获取 Redis URL（优先使用环境变量）
fn get_redis_url() -> String {
    // 先设置环境变量以允许不安全的 Redis 连接（仅用于测试）
    // 必须在读取 URL 之前设置
    std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
    std::env::var("OXCACHE_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

/// 基准测试Redis的SET操作性能
fn bench_redis_set(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url = get_redis_url();

    // 预先建立连接
    let backend = rt.block_on(async { RedisBackend::new(&redis_url).await.expect("Failed to connect to Redis") });

    c.bench_function("redis_set", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "bench:redis:set:{}",
                std::time::SystemTime::now().elapsed().unwrap().as_nanos()
            );
            let value = vec![0u8; 100];
            let _ = backend
                .set(black_box(&key), black_box(value), Some(Duration::from_secs(300)))
                .await;
        });
    });
}

/// 基准测试Redis的GET操作性能
fn bench_redis_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url = get_redis_url();

    // 预先建立连接并准备测试数据
    let backend = rt.block_on(async {
        let backend = RedisBackend::new(&redis_url).await.expect("Failed to connect to Redis");

        // 预填充测试数据
        let key = "bench:redis:get:test";
        let value = vec![0u8; 100];
        let _ = backend.set(key, value, Some(Duration::from_secs(300))).await;

        backend
    });

    c.bench_function("redis_get", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = backend.get(black_box("bench:redis:get:test")).await;
        });
    });
}

/// 基准测试Redis不同数据大小的SET性能
fn bench_redis_different_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url = get_redis_url();

    // 预先建立连接
    let backend = rt.block_on(async { RedisBackend::new(&redis_url).await.expect("Failed to connect to Redis") });

    let mut group = c.benchmark_group("redis_different_sizes");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let key = format!("bench:redis:size:{}", size);
                let value = vec![0u8; size];
                let _ = backend
                    .set(black_box(&key), black_box(value), Some(Duration::from_secs(300)))
                    .await;
            });
        });
    }

    group.finish();
}

/// 基准测试Redis的TTL操作性能
fn bench_redis_ttl(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url = get_redis_url();

    // 预先建立连接
    let backend = rt.block_on(async { RedisBackend::new(&redis_url).await.expect("Failed to connect to Redis") });

    c.bench_function("redis_ttl", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "bench:redis:ttl:{}",
                std::time::SystemTime::now().elapsed().unwrap().as_nanos()
            );
            let value = vec![0u8; 100];
            let _ = backend
                .set(black_box(&key), black_box(value), Some(Duration::from_secs(60)))
                .await;
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
