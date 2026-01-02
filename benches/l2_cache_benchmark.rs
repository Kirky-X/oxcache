//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! L2缓存性能基准测试
//!
//! 该模块提供L2缓存（Redis）的全面性能基准测试，包括：
//! - 基本操作性能测试（SET/GET/DELETE）
//! - 批量操作性能测试
//! - 不同数据大小的性能对比
//! - 并发性能测试
//! - 管道操作性能测试
//! - 集群模式性能测试

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxcache::backend::l2::L2Backend;
use oxcache::config::{L2Config, RedisMode};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::task::JoinSet;

/// 创建L2缓存配置
fn create_l2_config() -> L2Config {
    L2Config {
        mode: RedisMode::Standalone,
        connection_string: "redis://127.0.0.1:6379".to_string().into(),
        command_timeout_ms: 1000,
        connection_timeout_ms: 2000,
        password: None,
        enable_tls: false,
        sentinel: None,
        cluster: None,
        default_ttl: None,
        max_key_length: 256,
        max_value_size: 1024 * 1024 * 10,
    }
}

/// 基准测试L2缓存的基本SET操作性能
///
/// 测试不同数据大小下的SET操作性能
fn bench_l2_set(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_l2_config();

    let l2_backend = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => backend,
        Err(e) => {
            eprintln!("无法连接到Redis，跳过L2基准测试: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("l2_set");

    // 测试不同数据大小
    for size in [100, 1000, 10000, 100000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let key = format!("bench_set_{}", size);
                let value = vec![0u8; size];
                l2_backend
                    .set_with_version(black_box(&key), black_box(value), Some(300))
                    .await
            });
        });
    }

    group.finish();
}

/// 基准测试L2缓存的基本GET操作性能
///
/// 测试不同数据大小下的GET操作性能
fn bench_l2_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_l2_config();

    let l2_backend: Arc<L2Backend> = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            eprintln!("无法连接到Redis，跳过L2基准测试: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("l2_get");

    // 预填充不同大小的数据
    for size in [100, 1000, 10000, 100000].iter() {
        let key = format!("bench_get_{}", size);
        let value = vec![0u8; *size];
        rt.block_on(l2_backend.set_with_version(&key, value, Some(300)))
            .unwrap();

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let key = format!("bench_get_{}", size);
            b.to_async(&rt)
                .iter(|| async { l2_backend.get_with_version(black_box(&key)).await });
        });
    }

    group.finish();
}

/// 基准测试L2缓存的批量SET操作性能
///
/// 测试不同批量大小下的批量SET操作性能
fn bench_l2_batch_set(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_l2_config();

    let l2_backend: Arc<L2Backend> = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            eprintln!("无法连接到Redis，跳过L2基准测试: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("l2_batch_set");

    // 测试不同批量大小
    for batch_size in [10, 50, 100, 500, 1000].iter() {
        let total_bytes = *batch_size * 100; // 每个条目100字节
        group.throughput(Throughput::Bytes(total_bytes as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.to_async(&rt).iter(|| async {
                    let items: Vec<(String, Vec<u8>, Option<u64>)> = (0..batch_size)
                        .map(|i| (format!("batch_key_{}", i), vec![0u8; 100], Some(300)))
                        .collect();
                    l2_backend.pipeline_set_batch(black_box(items)).await
                });
            },
        );
    }

    group.finish();
}

/// 基准测试L2缓存的并发操作性能
///
/// 测试不同并发级别下的操作性能
fn bench_l2_concurrent(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_l2_config();

    let l2_backend: Arc<L2Backend> = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            eprintln!("无法连接到Redis，跳过L2基准测试: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("l2_concurrent");

    for concurrency in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let mut tasks: JoinSet<Result<(), oxcache::error::CacheError>> = JoinSet::new();

                    for i in 0..concurrency {
                        let backend = l2_backend.clone();
                        let key = format!("concurrent_key_{}", i);
                        let value = vec![0u8; 100];

                        tasks.spawn(async move {
                            backend.set_with_version(&key, value, Some(300)).await
                        });
                    }

                    while let Some(result) = tasks.join_next().await {
                        result.unwrap().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试L2缓存的管道操作性能
///
/// 测试WAL重放操作的性能
fn bench_l2_pipeline_wal(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_l2_config();

    let l2_backend: Arc<L2Backend> = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            eprintln!("无法连接到Redis，跳过L2基准测试: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("l2_pipeline_wal");

    for entry_count in [10, 50, 100, 500].iter() {
        let total_bytes = *entry_count * 100;
        group.throughput(Throughput::Bytes(total_bytes as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(entry_count),
            entry_count,
            |b, &entry_count| {
                b.to_async(&rt).iter(|| async {
                    let entries: Vec<oxcache::recovery::wal::WalEntry> = (0..entry_count)
                        .map(|i| oxcache::recovery::wal::WalEntry {
                            key: format!("wal_key_{}", i),
                            value: Some(vec![0u8; 100]),
                            ttl: Some(300),
                            operation: oxcache::recovery::wal::Operation::Set,
                            timestamp: std::time::SystemTime::now(),
                        })
                        .collect();
                    l2_backend.pipeline_replay(black_box(entries)).await
                });
            },
        );
    }

    group.finish();
}

/// 基准测试L2缓存的锁操作性能
///
/// 测试分布式锁的获取和释放性能
fn bench_l2_lock(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_l2_config();

    let l2_backend: Arc<L2Backend> = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            eprintln!("无法连接到Redis，跳过L2基准测试: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("l2_lock");

    group.bench_function("acquire", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "lock_key_{}",
                std::time::SystemTime::now()
                    .elapsed()
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            l2_backend
                .lock(black_box(&key), black_box("test_value"), black_box(10))
                .await
        });
    });

    group.bench_function("release", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "release_key_{}",
                std::time::SystemTime::now()
                    .elapsed()
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            l2_backend.lock(&key, "test_value", 10).await.unwrap();
            l2_backend
                .unlock(black_box(&key), black_box("test_value"))
                .await
        });
    });

    group.finish();
}

/// 基准测试L2缓存的TTL操作性能
///
/// 测试TTL获取操作的性能
fn bench_l2_ttl(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_l2_config();

    let l2_backend: Arc<L2Backend> = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            eprintln!("无法连接到Redis，跳过L2基准测试: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("l2_ttl");

    rt.block_on(l2_backend.set_with_version("ttl_key", vec![0u8; 100], Some(300)))
        .unwrap();

    group.bench_function("get_ttl", |b| {
        b.to_async(&rt)
            .iter(|| async { l2_backend.expire(black_box("ttl_key"), 300).await });
    });

    group.finish();
}

/// 基准测试L2缓存的连接性能
///
/// 测试连接建立和ping操作的性能
fn bench_l2_connection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_l2_config();

    let mut group = c.benchmark_group("l2_connection");

    group.bench_function("connect", |b| {
        b.to_async(&rt)
            .iter(|| async { L2Backend::new(black_box(&config)).await });
    });

    rt.block_on(async { L2Backend::new(&config).await.unwrap() });

    group.finish();
}

criterion_group!(
    benches,
    bench_l2_set,
    bench_l2_get,
    bench_l2_batch_set,
    bench_l2_concurrent,
    bench_l2_pipeline_wal,
    bench_l2_lock,
    bench_l2_ttl,
    bench_l2_connection
);
criterion_main!(benches);
