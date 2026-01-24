//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 缓存基准测试 - L1缓存性能测试

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxcache::Cache;
use tokio::runtime::Runtime;

// ============================= L1缓存基准测试 =============================

/// 基准测试L1缓存的设置操作性能
fn bench_l1_set(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("l1_set", |b| {
        b.to_async(&rt).iter(|| async {
            let key = "key".to_string();
            let value = vec![0u8; 100];
            let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
            cache.set(black_box(&key), black_box(&value)).await
        });
    });
}

/// 基准测试L1缓存的获取操作性能
fn bench_l1_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("l1_get", |b| {
        b.to_async(&rt).iter(|| async {
            let key = "key".to_string();
            let value = vec![0u8; 100];
            let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
            cache.set(&key, &value).await.unwrap();
            cache.get(black_box(&key)).await
        });
    });
}

/// 基准测试L1缓存不同数据大小的性能
fn bench_l1_different_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("l1_different_sizes");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let key = size.to_string();
                let value = vec![0u8; size];
                let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
                cache.set(black_box(&key), black_box(&value)).await
            });
        });
    }

    group.finish();
}

/// 基准测试L1缓存的批量操作性能
fn bench_l1_batch(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("l1_batch");

    for batch_size in [10, 50, 100].iter() {
        let total_bytes = *batch_size * 100;
        group.throughput(Throughput::Bytes(total_bytes as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.to_async(&rt).iter(|| async {
                    let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
                    for i in 0..batch_size {
                        let key = format!("l1_batch_key_{}", i);
                        let value = vec![0u8; 100];
                        cache.set(&key, &value).await.unwrap();
                    }
                    Ok::<_, ()>(())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_l1_set,
    bench_l1_get,
    bench_l1_different_sizes,
    bench_l1_batch
);
criterion_main!(benches);
