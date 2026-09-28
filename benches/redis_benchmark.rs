// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Redis L2 缓存性能基准测试

//! Redis 不可达时，各基准函数会打印 `[bench-skip]` 原因并跳过，
//! 避免 `cargo test --all-targets` 在无服务环境下因 bench 假红。

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use oxcache::backend::memory::RedisBackend;
use oxcache::backend::{CacheReader, CacheWriter};
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

mod common;

use common::services_ready;

// ============================= Redis L2 缓存基准测试 =============================

/// 获取 Redis URL（优先使用环境变量）
fn get_redis_url() -> String {
    // SAFETY: edition 2024 下 set_var 为 unsafe；bench 单线程运行，进程级 env 仅此处修改，设置允许不安全 Redis 连接的测试标志。
    unsafe {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
    };
    std::env::var("OXCACHE_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

/// 基准测试Redis的SET操作性能
fn bench_redis_set(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url = get_redis_url();

    if !services_ready(&rt, &[("Redis", &redis_url)]) {
        return;
    }

    // 预先建立连接
    let backend = rt.block_on(async {
        RedisBackend::new(&redis_url)
            .await
            .expect("Failed to connect to Redis")
    });

    c.bench_function("redis_set", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "bench:redis:set:{}",
                std::time::SystemTime::now().elapsed().unwrap().as_nanos()
            );
            let value = vec![0u8; 100];
            let _ = backend
                .set(
                    Arc::from(black_box(&key).as_str()),
                    Arc::new(black_box(value)),
                    Some(Duration::from_secs(300)),
                )
                .await;
        });
    });
}

/// 基准测试Redis的GET操作性能
fn bench_redis_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url = get_redis_url();

    if !services_ready(&rt, &[("Redis", &redis_url)]) {
        return;
    }

    // 预先建立连接并准备测试数据
    let backend = rt.block_on(async {
        let backend = RedisBackend::new(&redis_url)
            .await
            .expect("Failed to connect to Redis");

        // 预填充测试数据
        let key = "bench:redis:get:test";
        let value = vec![0u8; 100];
        let _ = backend
            .set(
                Arc::from(key),
                Arc::new(value),
                Some(Duration::from_secs(300)),
            )
            .await;

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

    if !services_ready(&rt, &[("Redis", &redis_url)]) {
        return;
    }

    // 预先建立连接
    let backend = rt.block_on(async {
        RedisBackend::new(&redis_url)
            .await
            .expect("Failed to connect to Redis")
    });

    let mut group = c.benchmark_group("redis_different_sizes");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let key = format!("bench:redis:size:{}", size);
                let value = vec![0u8; size];
                let _ = backend
                    .set(
                        Arc::from(black_box(&key).as_str()),
                        Arc::new(black_box(value)),
                        Some(Duration::from_secs(300)),
                    )
                    .await;
            });
        });
    }

    group.finish();
}

/// 基准测试Redis的TTL操作性能：秒级/亚秒 set 与 PEXPIRE 同组对比，
/// 覆盖 SET PX 四参 payload 与毫秒参数的命令面
fn bench_redis_ttl(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let redis_url = get_redis_url();

    if !services_ready(&rt, &[("Redis", &redis_url)]) {
        return;
    }

    // 预先建立连接
    let backend = rt.block_on(async {
        RedisBackend::new(&redis_url)
            .await
            .expect("Failed to connect to Redis")
    });

    // 预置永久键供 expire 反复重设 TTL（PEXPIRE 对既有键刷新时限，可循环计量）
    rt.block_on(async {
        backend
            .set(
                Arc::from("bench:redis:expire"),
                Arc::new(vec![0u8; 100]),
                None,
            )
            .await
            .expect("setup set for expire bench failed");
    });

    let mut group = c.benchmark_group("redis_ttl");

    group.bench_function("set_60s", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "bench:redis:ttl:set60:{}",
                std::time::SystemTime::now().elapsed().unwrap().as_nanos()
            );
            let value = vec![0u8; 100];
            let _ = backend
                .set(
                    Arc::from(black_box(&key).as_str()),
                    Arc::new(black_box(value)),
                    Some(Duration::from_secs(60)),
                )
                .await;
        });
    });

    group.bench_function("set_100ms", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!(
                "bench:redis:ttl:set100ms:{}",
                std::time::SystemTime::now().elapsed().unwrap().as_nanos()
            );
            let value = vec![0u8; 100];
            let _ = backend
                .set(
                    Arc::from(black_box(&key).as_str()),
                    Arc::new(black_box(value)),
                    Some(Duration::from_millis(100)),
                )
                .await;
        });
    });

    group.bench_function("expire_60s", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = black_box(
                backend
                    .expire("bench:redis:expire", Duration::from_secs(60))
                    .await,
            );
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_redis_set,
    bench_redis_get,
    bench_redis_different_sizes,
    bench_redis_ttl
);
criterion_main!(benches);
