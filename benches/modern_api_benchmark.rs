//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 使用新API的简单性能基准测试
//!
//! 测试L1缓存的性能表现

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use oxcache::Cache;
use std::time::Instant;

/// 基准测试L1缓存的基本操作性能
fn bench_l1_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // 创建L1内存缓存
    let cache = rt.block_on(async { Cache::<String, String>::new().await.unwrap() });

    // 预填充测试数据
    rt.block_on(async {
        for i in 0..1000 {
            let key = format!("bench_key_{}", i);
            let value = format!("bench_value_{}", i);
            let _ = cache.set(&key, &value).await;
        }
    });

    // 基准测试GET操作（缓存命中）
    c.bench_function("l1_get_hit", |b| {
        b.to_async(&rt).iter(|| async {
            let _: Option<String> = cache
                .get(black_box(&"bench_key_42".to_string()))
                .await
                .unwrap();
        });
    });

    // 基准测试GET操作（缓存未命中）
    c.bench_function("l1_get_miss", |b| {
        b.to_async(&rt).iter(|| async {
            let _: Option<String> = cache
                .get(black_box(&"nonexistent_key".to_string()))
                .await
                .unwrap();
        });
    });

    // 基准测试SET操作
    c.bench_function("l1_set", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "set_key_{}",
                std::time::SystemTime::now().elapsed().unwrap().as_nanos()
            );
            let value = "test_value".to_string();
            let _ = cache.set(black_box(&key), black_box(&value)).await;
        });
    });

    // 基准测试GET_OR_SET操作（缓存穿透模式）
    c.bench_function("l1_get_or_set", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "get_or_set_key_{}",
                std::time::SystemTime::now().elapsed().unwrap().as_nanos()
            );
            let value: String = cache
                .get_or(&key, || async {
                    Ok(format!(
                        "computed_value_{}",
                        std::time::SystemTime::now().elapsed().unwrap().as_nanos()
                    ))
                })
                .await
                .unwrap();
            black_box(value);
        });
    });
}

/// 基准测试不同数据大小的性能
fn bench_l1_sizes(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let cache = rt.block_on(async { Cache::<String, Vec<u8>>::new().await.unwrap() });

    let mut group = c.benchmark_group("l1_different_sizes");

    for &size in [10, 100, 1000, 10000].iter() {
        group.bench_with_input(&format!("size_{}", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let key = format!("size_test_{}", size);
                let value = vec![0u8; *size];
                let _ = cache.set(&key, &value).await;
            });
        });
    }

    group.finish();
}

/// 基准测试并发操作性能
fn bench_l1_concurrent(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let cache = rt.block_on(async { Cache::<String, String>::new().await.unwrap() });

    let mut group = c.benchmark_group("l1_concurrent");

    for &concurrency in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            &format!("concurrent_{}", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let start = Instant::now();
                    let mut handles = Vec::new();

                    for i in 0..*concurrency {
                        let cache = cache.clone();
                        let handle = tokio::spawn(async move {
                            let key = format!("concurrent_key_{}", i);
                            let value = format!("concurrent_value_{}", i);
                            let _ = cache.set(&key, &value).await;
                            let _: Option<String> = cache.get(&key).await;
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }

                    black_box(start.elapsed());
                });
            },
        );
    }

    group.finish();
}

/// 基准测试批量操作性能
fn bench_l1_batch(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let cache = rt.block_on(async { Cache::<String, String>::new().await.unwrap() });

    let mut group = c.benchmark_group("l1_batch");

    for &batch_size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            &format!("batch_size_{}", batch_size),
            batch_size,
            |b, &batch_size| {
                b.to_async(&rt).iter(|| async {
                    let items: Vec<(&String, &String)> = (0..*batch_size)
                        .map(|i| (&format!("batch_key_{}", i), &format!("batch_value_{}", i)))
                        .collect();

                    let _ = cache.set_many(items).await;
                });
            },
        );
    }

    group.finish();
}

/// 基准测试吞吐量
fn bench_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let cache = rt.block_on(async { Cache::<String, String>::new().await.unwrap() });

    let mut group = c.benchmark_group("throughput");

    for &ops_count in [100, 500, 1000, 5000].iter() {
        group.bench_with_input(&format!("ops_{}", ops_count), ops_count, |b, &ops_count| {
            b.to_async(&rt).iter(|| async {
                let start = Instant::now();

                for i in 0..*ops_count {
                    let key = format!("throughput_key_{}", i);
                    let value = format!("throughput_value_{}", i);
                    let _ = cache.set(&key, &value).await;
                }

                black_box(start.elapsed());
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_l1_operations,
    bench_l1_sizes,
    bench_l1_concurrent,
    bench_l1_batch,
    bench_throughput
);
criterion_main!(benches);
