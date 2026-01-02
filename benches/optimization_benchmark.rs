//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 性能优化基准测试 - 批量操作管道化和指标导出
//!
//! 该模块提供以下性能基准测试：
//! - 批量操作管道化性能测试
//! - 指标收集和导出性能测试
//! - 与传统顺序执行的对比测试

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxcache::backend::l2::L2Backend;
use oxcache::config::{L2Config, RedisMode};
use oxcache::sync::batch_writer::BatchWriter;
use std::sync::Arc;
use tokio::runtime::Runtime;

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

/// 基准测试批量操作管道化性能
///
/// 测试并行执行 set 和 delete 操作相比顺序执行的性能提升
fn bench_pipeline_parallel_set_delete(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_l2_config();

    let l2_backend = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            eprintln!(
                "Cannot connect to Redis, skipping pipeline benchmark: {}",
                e
            );
            return;
        }
    };

    let mut group = c.benchmark_group("pipeline_parallel");

    for batch_size in [50, 100, 200, 500].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.to_async(&rt).iter(|| async {
                    let mut set_items = Vec::with_capacity(batch_size / 2);
                    let mut delete_keys = Vec::with_capacity(batch_size / 2);

                    for i in 0..batch_size {
                        let key = format!("pipeline_key_{}", i);
                        if i % 2 == 0 {
                            set_items.push((key, vec![0u8; 100], Some(300)));
                        } else {
                            delete_keys.push(key);
                        }
                    }

                    let set_future = l2_backend.pipeline_set_batch(set_items);
                    let del_future = l2_backend.pipeline_del_batch(delete_keys);
                    let (_, _) = tokio::join!(set_future, del_future);
                });
            },
        );
    }

    group.finish();
}

/// 基准测试大批量分片处理性能
///
/// 测试将大批量操作分片处理的性能
fn bench_pipeline_chunked_batch(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_l2_config();

    let l2_backend = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            eprintln!("Cannot connect to Redis, skipping chunked benchmark: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("pipeline_chunked");

    for (total_size, chunk_size) in [(1000, 100), (2000, 200), (5000, 500)].iter() {
        group.throughput(Throughput::Elements(*total_size as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(total_size),
            total_size,
            |b, &total_size| {
                b.to_async(&rt).iter(|| async {
                    let items: Vec<(String, Vec<u8>, Option<u64>)> = (0..total_size)
                        .map(|i| (format!("chunked_key_{}", i), vec![0u8; 100], Some(300)))
                        .collect();

                    let chunk_size = *chunk_size;
                    for chunk in items.chunks(chunk_size) {
                        let _ = l2_backend.pipeline_set_batch(chunk.to_vec()).await;
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试批量写入器性能
///
/// 测试 BatchWriter 的整体性能
fn bench_batch_writer_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_l2_config();

    let l2_backend = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            eprintln!(
                "Cannot connect to Redis, skipping batch writer benchmark: {}",
                e
            );
            return;
        }
    };

    let mut group = c.benchmark_group("batch_writer");

    for ops_count in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*ops_count as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(ops_count),
            ops_count,
            |b, &ops_count| {
                b.to_async(&rt).iter(|| async {
                    let writer = BatchWriter::new_with_default_config(
                        "benchmark_service".to_string(),
                        l2_backend.clone(),
                    );

                    for i in 0..ops_count {
                        let key = format!("batch_writer_key_{}", i);
                        let value = vec![0u8; 100];
                        let _ = writer.enqueue(key, value, Some(300)).await;
                    }

                    writer.shutdown().await;
                });
            },
        );
    }

    group.finish();
}

/// 基准测试指标收集性能
///
/// 测试指标收集的开销
fn bench_metrics_collection(c: &mut Criterion) {
    let metrics = &oxcache::metrics::GLOBAL_METRICS;

    let mut group = c.benchmark_group("metrics_collection");

    group.bench_function("record_request", |b| {
        b.iter(|| {
            metrics.record_request(
                black_box("test_service"),
                black_box("L1"),
                black_box("get"),
                black_box("hit"),
            );
        });
    });

    group.bench_function("record_duration", |b| {
        b.iter(|| {
            metrics.record_duration(
                black_box("test_service"),
                black_box("L2"),
                black_box("set"),
                black_box(0.001),
            );
        });
    });

    group.bench_function("set_batch_buffer_size", |b| {
        b.iter(|| {
            metrics.set_batch_buffer_size(black_box("test_service"), black_box(100));
        });
    });

    group.finish();
}

/// 基准测试指标导出性能
///
/// 测试指标字符串生成的性能
fn bench_metrics_export(c: &mut Criterion) {
    let metrics = &oxcache::metrics::GLOBAL_METRICS;

    for _ in 0..100 {
        metrics.record_request("test_service", "L1", "get", "hit");
        metrics.record_request("test_service", "L2", "set", "miss");
        metrics.set_batch_buffer_size("test_service", 500);
    }

    let mut group = c.benchmark_group("metrics_export");

    group.bench_function("get_metrics_string", |b| {
        b.iter(|| {
            let _ = oxcache::metrics::get_metrics_string();
        });
    });

    group.finish();
}

/// 基准测试原子计数器性能
///
/// 测试不同原子操作的性能
fn bench_atomic_counter(c: &mut Criterion) {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    let counter = Arc::new(AtomicU64::new(0));

    let mut group = c.benchmark_group("atomic_counter");

    group.bench_function("increment", |b| {
        b.iter(|| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
    });

    group.bench_function("load", |b| {
        b.iter(|| {
            let _ = counter.load(Ordering::SeqCst);
        });
    });

    group.bench_function("store", |b| {
        b.iter(|| {
            counter.store(42, Ordering::SeqCst);
        });
    });

    group.finish();
}

/// 基准测试预分配性能
///
/// 测试不同容量预分配的性能差异
fn bench_preallocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("preallocation");

    group.bench_function("vec_with_capacity_128", |b| {
        b.iter(|| {
            let mut vec = Vec::with_capacity(128);
            for i in 0..100 {
                vec.push(i);
            }
            black_box(vec);
        });
    });

    group.bench_function("vec_without_capacity", |b| {
        b.iter(|| {
            let mut vec = Vec::new();
            for i in 0..100 {
                vec.push(i);
            }
            black_box(vec);
        });
    });

    group.bench_function("string_with_capacity_1024", |b| {
        b.iter(|| {
            let mut s = String::with_capacity(1024);
            for i in 0..100 {
                s.push_str(&format!("item_{}_", i));
            }
            black_box(s);
        });
    });

    group.bench_function("string_without_capacity", |b| {
        b.iter(|| {
            let mut s = String::new();
            for i in 0..100 {
                s.push_str(&format!("item_{}_", i));
            }
            black_box(s);
        });
    });

    group.finish();
}

// ============================= 基准测试组定义 =============================

criterion_group!(
    pipeline_benches,
    bench_pipeline_parallel_set_delete,
    bench_pipeline_chunked_batch,
    bench_batch_writer_throughput
);

criterion_group!(
    metrics_benches,
    bench_metrics_collection,
    bench_metrics_export,
    bench_atomic_counter
);

criterion_group!(optimization_benches, bench_preallocation);

criterion_main!(pipeline_benches, metrics_benches, optimization_benches);
