//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 缓存基准测试 - 综合L1和L2缓存性能测试
//!
//! 该模块提供两级缓存架构的完整性能基准测试：
//! - L1缓存（本地内存）性能测试
//! - L2缓存（Redis分布式）性能测试
//! - 两级缓存协同工作性能测试
//! - 不同负载模式下的性能对比

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxcache::backend::l1::L1Backend;
use oxcache::backend::l2::L2Backend;
use oxcache::client::two_level::TwoLevelClient;
use oxcache::config::{L2Config, RedisMode, TwoLevelConfig};
use oxcache::serialization::SerializerEnum;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::task::JoinSet;

// ============================= L1缓存基准测试 =============================

/// 基准测试L1缓存的设置操作性能
///
/// 测试向L1缓存中设置键值对的性能
fn bench_l1_set(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let l1 = L1Backend::new(10000);

    c.bench_function("l1_set", |b| {
        b.to_async(&rt).iter(|| async {
            l1.set_bytes(black_box("key"), black_box(vec![0; 100]), Some(300))
                .await
        });
    });
}

/// 基准测试L1缓存的获取操作性能
///
/// 测试从L1缓存中获取键值对的性能
fn bench_l1_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let l1 = L1Backend::new(10000);
    rt.block_on(l1.set_bytes("key", vec![0; 100], Some(300)))
        .unwrap();

    c.bench_function("l1_get", |b| {
        b.to_async(&rt)
            .iter(|| async { l1.get_with_metadata(black_box("key")).await });
    });
}

/// 基准测试L1缓存不同数据大小的性能
///
/// 测试不同数据大小对L1缓存性能的影响
fn bench_l1_different_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let l1 = L1Backend::new(10000);

    let mut group = c.benchmark_group("l1_different_sizes");

    for size in [100, 1000, 10000, 100000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let key = format!("l1_size_test_{}", size);
                let value = vec![0u8; size];
                l1.set_bytes(black_box(&key), black_box(value), Some(300))
                    .await
            });
        });
    }

    group.finish();
}

/// 基准测试L1缓存的并发操作性能
///
/// 测试不同并发级别下的L1缓存性能
fn bench_l1_concurrent(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let l1 = L1Backend::new(10000);

    let mut group = c.benchmark_group("l1_concurrent");

    for concurrency in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let mut tasks = JoinSet::new();

                    for i in 0..concurrency {
                        let l1_clone = l1.clone();
                        let key = format!("l1_concurrent_key_{}", i);
                        let value = vec![0u8; 100];

                        tasks
                            .spawn(async move { l1_clone.set_bytes(&key, value, Some(300)).await });
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

// ============================= L2缓存基准测试 =============================

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

    for size in [100, 1000, 10000, 100000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let key = format!("bench_l2_set_{}", size);
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

    let l2_backend = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => backend,
        Err(e) => {
            eprintln!("无法连接到Redis，跳过L2基准测试: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("l2_get");

    // 预填充测试数据
    for size in [100, 1000, 10000, 100000].iter() {
        let key = format!("bench_l2_get_{}", size);
        let value = vec![0u8; *size];
        rt.block_on(l2_backend.set_with_version(&key, value, Some(300)))
            .unwrap();

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let key = format!("bench_l2_get_{}", size);
            b.to_async(&rt)
                .iter(|| async { l2_backend.get_with_version(black_box(&key)).await });
        });
    }

    group.finish();
}

/// 基准测试L2缓存的批量操作性能
///
/// 测试不同批量大小下的批量SET操作性能
fn bench_l2_batch(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_l2_config();

    let l2_backend = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => backend,
        Err(e) => {
            eprintln!("无法连接到Redis，跳过L2基准测试: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("l2_batch");

    for batch_size in [10, 50, 100, 500, 1000].iter() {
        let total_bytes = *batch_size * 100;
        group.throughput(Throughput::Bytes(total_bytes as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.to_async(&rt).iter(|| async {
                    let items: Vec<(String, Vec<u8>, Option<u64>)> = (0..batch_size)
                        .map(|i| (format!("l2_batch_key_{}", i), vec![0u8; 100], Some(300)))
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

    let l2_backend = match rt.block_on(async { L2Backend::new(&config).await }) {
        Ok(backend) => backend,
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
                    let mut tasks = JoinSet::new();

                    for i in 0..concurrency {
                        let backend = l2_backend.clone();
                        let key = format!("l2_concurrent_key_{}", i);
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

// ============================= 两级缓存协同测试 =============================

/// 基准测试两级缓存的协同性能
///
/// 测试L1+L2组合缓存的性能表现
fn bench_two_level_cache(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let l1 = Arc::new(L1Backend::new(10000));

    let l2_config = create_l2_config();
    let l2 = match rt.block_on(async { L2Backend::new(&l2_config).await }) {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            eprintln!("无法连接到Redis，跳过两级缓存基准测试: {}", e);
            return;
        }
    };

    let config = TwoLevelConfig {
        promote_on_hit: true,
        enable_batch_write: false,
        batch_size: 100,
        batch_interval_ms: 100,
        invalidation_channel: None,
        bloom_filter: None,
        warmup: None,
        max_key_length: Some(256),
        max_value_size: Some(1024 * 1024 * 10),
    };

    let cache = rt.block_on(async {
        TwoLevelClient::new(
            "bench_service".to_string(),
            config,
            l1.clone(),
            l2.clone(),
            SerializerEnum::Json(oxcache::serialization::json::JsonSerializer::new()),
        )
        .await
        .unwrap()
    });

    let mut group = c.benchmark_group("two_level_cache");

    // 测试缓存未命中（需要访问L2）的情况
    group.bench_function("miss_then_hit", |b| {
        b.to_async(&rt).iter(|| async {
            let _: Option<String> = cache.get(black_box("two_level_key")).await.unwrap();
        });
    });

    // 测试写入操作的性能（写入L1和L2）
    group.bench_function("write_both_levels", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "two_level_write_{}",
                std::time::SystemTime::now()
                    .elapsed()
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            cache
                .set(black_box(&key), black_box(&vec![0u8; 100]), Some(300))
                .await
                .unwrap();
        });
    });

    group.finish();
}

// ============================= 缓存命中率影响测试 =============================

/// 基准测试不同缓存命中率对整体性能的影响
///
/// 模拟不同的访问模式，测量命中率对响应时间的影响
fn bench_cache_hit_ratio(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let l1 = Arc::new(L1Backend::new(1000));

    let l2_config = create_l2_config();
    let l2 = match rt.block_on(async { L2Backend::new(&l2_config).await }) {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            eprintln!("无法连接到Redis，跳过命中率基准测试: {}", e);
            return;
        }
    };

    let config = TwoLevelConfig {
        promote_on_hit: true,
        enable_batch_write: false,
        batch_size: 100,
        batch_interval_ms: 100,
        invalidation_channel: None,
        bloom_filter: None,
        warmup: None,
        max_key_length: Some(256),
        max_value_size: Some(1024 * 1024 * 10),
    };

    let cache = rt.block_on(async {
        TwoLevelClient::new(
            "bench_service".to_string(),
            config,
            l1.clone(),
            l2.clone(),
            SerializerEnum::Json(oxcache::serialization::json::JsonSerializer::new()),
        )
        .await
        .unwrap()
    });

    let mut group = c.benchmark_group("cache_hit_ratio");

    // 100% 命中率 - 所有key都在L1中
    group.bench_function("hit_ratio_100", |b| {
        b.to_async(&rt).iter(|| async {
            let _: Option<String> = cache.get(black_box("hot_key_1")).await.unwrap();
        });
    });

    // 50% 命中率 - 交替访问热点和非热点key
    group.bench_function("hit_ratio_50", |b| {
        b.to_async(&rt).iter(|| async {
            let key = if rand::random::<bool>() {
                "hot_key_1"
            } else {
                "cold_key_1"
            };
            let _: Option<String> = cache.get(black_box(key)).await.unwrap();
        });
    });

    // 10% 命中率 - 大多数访问是冷key
    group.bench_function("hit_ratio_10", |b| {
        b.to_async(&rt).iter(|| async {
            let key_num: u32 = rand::random();
            let key = if key_num % 10 == 0 {
                "hot_key_1".to_string()
            } else {
                format!("cold_key_{}", key_num)
            };
            let _: Option<String> = cache.get(black_box(&key)).await.unwrap();
        });
    });

    // 0% 命中率 - 所有key都是新的
    group.bench_function("hit_ratio_0", |b| {
        b.to_async(&rt).iter(|| async {
            let key_num: u32 = rand::random();
            let key = format!("new_key_{}", key_num);
            let _: Option<String> = cache.get(black_box(&key)).await.unwrap();
        });
    });

    group.finish();
}

/// 基准测试L1缓存大小对性能的影响
///
/// 测试不同L1容量设置下的性能表现
fn bench_l1_size_impact(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let l2_config = create_l2_config();
    let l2 = match rt.block_on(async { L2Backend::new(&l2_config).await }) {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            eprintln!("无法连接到Redis，跳过L1大小影响基准测试: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("l1_size_impact");

    let two_level_config = TwoLevelConfig {
        promote_on_hit: true,
        enable_batch_write: false,
        batch_size: 100,
        batch_interval_ms: 100,
        invalidation_channel: None,
        bloom_filter: None,
        warmup: None,
        max_key_length: Some(256),
        max_value_size: Some(1024 * 1024 * 10),
    };

    let l1_empty = Arc::new(L1Backend::new(10000));
    let cache_for_prefill = rt.block_on(async {
        TwoLevelClient::new(
            "bench_service".to_string(),
            two_level_config.clone(),
            l1_empty.clone(),
            l2.clone(),
            SerializerEnum::Json(oxcache::serialization::json::JsonSerializer::new()),
        )
        .await
        .unwrap()
    });

    let keys: Vec<String> = (0..1000).map(|i| format!("l1_size_key_{}", i)).collect();
    for key in &keys {
        let value = "test_value_for_benchmark".to_string();
        rt.block_on(cache_for_prefill.set(key, &value, Some(300)))
            .unwrap();
    }

    let l1_small = Arc::new(L1Backend::new(100));
    let cache_small = rt.block_on(async {
        TwoLevelClient::new(
            "bench_service".to_string(),
            two_level_config.clone(),
            l1_small.clone(),
            l2.clone(),
            SerializerEnum::Json(oxcache::serialization::json::JsonSerializer::new()),
        )
        .await
        .unwrap()
    });

    let l1_medium = Arc::new(L1Backend::new(500));
    let cache_medium = rt.block_on(async {
        TwoLevelClient::new(
            "bench_service".to_string(),
            two_level_config.clone(),
            l1_medium.clone(),
            l2.clone(),
            SerializerEnum::Json(oxcache::serialization::json::JsonSerializer::new()),
        )
        .await
        .unwrap()
    });

    let l1_large = Arc::new(L1Backend::new(2000));
    let cache_large = rt.block_on(async {
        TwoLevelClient::new(
            "bench_service".to_string(),
            two_level_config,
            l1_large.clone(),
            l2.clone(),
            SerializerEnum::Json(oxcache::serialization::json::JsonSerializer::new()),
        )
        .await
        .unwrap()
    });

    group.bench_function("l1_size_100", |b| {
        b.to_async(&rt).iter(|| async {
            for key in &keys {
                let _: Option<String> = cache_small.get(black_box(key)).await.unwrap();
            }
        });
    });

    group.bench_function("l1_size_500", |b| {
        b.to_async(&rt).iter(|| async {
            for key in &keys {
                let _: Option<String> = cache_medium.get(black_box(key)).await.unwrap();
            }
        });
    });

    group.bench_function("l1_size_2000", |b| {
        b.to_async(&rt).iter(|| async {
            for key in &keys {
                let _: Option<String> = cache_large.get(black_box(key)).await.unwrap();
            }
        });
    });

    for key in &keys {
        rt.block_on(l2.delete(key)).unwrap();
    }

    group.finish();
}

// ============================= 延迟基准测试 =============================

fn benchmark_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let l1 = Arc::new(L1Backend::new(10000));

    let l2_config = L2Config {
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
    };
    let l2 = rt.block_on(async {
        Arc::new(
            L2Backend::new(&l2_config)
                .await
                .expect("Failed to connect to Redis"),
        )
    });

    let config = TwoLevelConfig {
        promote_on_hit: true,
        enable_batch_write: false,
        batch_size: 100,
        batch_interval_ms: 100,
        invalidation_channel: None,
        bloom_filter: None,
        warmup: None,
        max_key_length: Some(256),
        max_value_size: Some(1024 * 1024 * 10),
    };

    let client = rt.block_on(async {
        Arc::new(
            TwoLevelClient::new(
                "bench_service".to_string(),
                config,
                l1.clone(),
                l2.clone(),
                SerializerEnum::Json(oxcache::serialization::json::JsonSerializer::new()),
            )
            .await
            .unwrap(),
        )
    });

    let mut group = c.benchmark_group("oxcache_latency");

    group.bench_function("l1_set", |b| {
        b.to_async(&rt).iter(|| async {
            l1.set_bytes("bench_key_l1", b"value".to_vec(), None)
                .await
                .unwrap();
        })
    });

    group.bench_function("l1_get", |b| {
        b.to_async(&rt).iter(|| async {
            l1.get_with_metadata("bench_key_l1").await.unwrap();
        })
    });

    group.bench_function("l2_set", |b| {
        b.to_async(&rt).iter(|| async {
            l2.set_with_version("bench_key_l2", b"value".to_vec(), None)
                .await
                .unwrap();
        })
    });

    group.bench_function("l2_get", |b| {
        b.to_async(&rt).iter(|| async {
            l2.get_with_version("bench_key_l2").await.unwrap();
        })
    });

    rt.block_on(async {
        client.set("bench_key_hit", &"value", None).await.unwrap();
    });

    group.bench_function("client_get_hit", |b| {
        b.to_async(&rt).iter(|| async {
            client.get::<String>("bench_key_hit").await.unwrap();
        })
    });

    group.finish();
}

// ============================= 综合基准测试组 =============================

criterion_group!(
    l1_benches,
    bench_l1_set,
    bench_l1_get,
    bench_l1_different_sizes,
    bench_l1_concurrent
);

criterion_group!(
    l2_benches,
    bench_l2_set,
    bench_l2_get,
    bench_l2_batch,
    bench_l2_concurrent
);

criterion_group!(two_level_benches, bench_two_level_cache);

criterion_group!(
    hit_ratio_benches,
    bench_cache_hit_ratio,
    bench_l1_size_impact
);

criterion_group!(latency_benches, benchmark_latency);

criterion_main!(
    l1_benches,
    l2_benches,
    two_level_benches,
    hit_ratio_benches,
    latency_benches
);
