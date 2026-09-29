// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 同步面三路对照基准：原生（`sync_backend_arc`）vs 桥接（`backend_arc` +
//! `sync_mode`，`block_in_place` + `block_on`）vs async API 基线
//!
//! 量化 R8 解锁的桥接面的每操作开销（桥接税），并按 `worker_threads`
//! 1/4 两档体现桥接对 runtime worker 的侵蚀曲线——桥接在单 worker
//! multi_thread runtime 上会把唯一 worker 转为阻塞线程，I/O 型后端有
//! 挂起风险（见 `AsyncToSyncBridge` 文档），内存型后端仅表现吞吐衰减。
//! 数据用于 `sync_backend_arc` 原生面与桥接面之间的选型决策。
//!
//! 调用环境即语义：桥接面必须在 multi_thread runtime 内调用
//! （`to_async` 承载，sync 调用位于 runtime 上下文）；原生 sync 面以
//! runtime 外主线程调用（其运行时无关的卖点场景）；async 基线在
//! runtime 内 `.await`。

use criterion::{Criterion, criterion_group, criterion_main};
use oxcache::{Cache, MokaMemoryBackend};
use std::hint::black_box;
use std::sync::Arc;

const CAPACITY: u64 = 10_000;

fn moka_backend() -> Arc<MokaMemoryBackend> {
    Arc::new(MokaMemoryBackend::builder().capacity(CAPACITY).build())
}

fn key() -> String {
    "sync_surface_key".to_string()
}

/// 构建 multi_thread runtime 缓存；`sync_first` 选择 sync_backend_arc（原生）
/// 或 backend_arc（桥接）注入，两者均开 `sync_mode(true)`。
fn build_sync_cache(rt: &tokio::runtime::Runtime, sync_first: bool) -> Cache<String, String> {
    rt.block_on(async {
        let builder = Cache::builder().capacity(CAPACITY).sync_mode(true);
        let builder = if sync_first {
            builder.sync_backend_arc(moka_backend())
        } else {
            builder.backend_arc(moka_backend())
        };
        builder.build().await.unwrap()
    })
}

fn build_async_cache(rt: &tokio::runtime::Runtime) -> Cache<String, String> {
    rt.block_on(async {
        Cache::builder()
            .capacity(CAPACITY)
            .backend_arc(moka_backend())
            .build()
            .await
            .unwrap()
    })
}

/// 配置通路（CacheConfig → apply_to_cache_builder）的 async 面：锁定 Dual
/// 槽交付的是原生 moka async 面（与 backend_arc 直注同档，而非门面降级）
fn build_config_async_cache(rt: &tokio::runtime::Runtime) -> Cache<String, String> {
    use oxcache::config::CacheConfig;

    let config = CacheConfig::builder()
        .capacity(CAPACITY)
        .backend("moka")
        .build();
    rt.block_on(async {
        let builder = config
            .apply_to_cache_builder(oxcache::CacheBuilder::<String, String>::default())
            .await
            .unwrap();
        builder.build().await.unwrap()
    })
}

/// 预填充在各缓存的宿主 runtime 内执行（原生/桥接 sync 面在
/// runtime 上下文中均可用）
fn seed(rt: &tokio::runtime::Runtime, cache: &Cache<String, String>) {
    rt.block_on(async {
        for i in 0..1000u32 {
            let key = format!("seed_key_{i}");
            let value = format!("value_{i}");
            cache.set_sync(&key, &value).unwrap();
        }
    });
}

/// async 面 seed（未开 sync_mode 的缓存）
fn seed_async(rt: &tokio::runtime::Runtime, cache: &Cache<String, String>) {
    rt.block_on(async {
        for i in 0..1000u32 {
            let key = format!("seed_key_{i}");
            let value = format!("value_{i}");
            cache.set_by_str(&key, &value, None).await.unwrap();
        }
    });
}

fn bench_sync_surfaces(c: &mut Criterion) {
    let rt_w4 = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let rt_w1 = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    // 四路构建：async 基线、原生 sync、桥接（w4/w1 各一）、配置通路 async 面
    let async_cache_w4 = build_async_cache(&rt_w4);
    let native_sync_w4 = build_sync_cache(&rt_w4, true);
    let bridged_w4 = build_sync_cache(&rt_w4, false);
    let bridged_w1 = build_sync_cache(&rt_w1, false);
    let async_cache_w1 = build_async_cache(&rt_w1);
    let config_async_w4 = build_config_async_cache(&rt_w4);

    seed(&rt_w4, &native_sync_w4);
    seed(&rt_w4, &bridged_w4);
    seed(&rt_w1, &bridged_w1);
    // 配置通路 async 缓存未开 sync_mode：seed 走 async 面
    seed_async(&rt_w4, &config_async_w4);

    let key = key();
    let value = "v".to_string();

    // —— worker=4：三路对照主表 + 配置通路 async 面（回归锁：应与直注同档）——
    c.bench_function("sync_surface_w4_async_get", |b| {
        b.to_async(&rt_w4).iter(|| async {
            black_box(async_cache_w4.get_by_str(black_box(&key)).await.unwrap());
        })
    });
    c.bench_function("sync_surface_w4_config_async_get", |b| {
        b.to_async(&rt_w4).iter(|| async {
            black_box(config_async_w4.get_by_str(black_box(&key)).await.unwrap());
        })
    });
    c.bench_function("sync_surface_w4_async_set", |b| {
        b.to_async(&rt_w4).iter(|| async {
            async_cache_w4
                .set_by_str(black_box(&key), black_box(&value), None)
                .await
                .unwrap();
        })
    });
    // 原生 sync 面：runtime 外主线程调用（运行时无关场景）
    c.bench_function("sync_surface_w4_native_sync_get", |b| {
        b.iter(|| black_box(native_sync_w4.get_sync(black_box(&key)).unwrap()))
    });
    c.bench_function("sync_surface_w4_native_sync_set", |b| {
        b.iter(|| {
            native_sync_w4
                .set_sync(black_box(&key), black_box(&value))
                .unwrap()
        })
    });
    // 桥接面：必须处于 multi_thread runtime 上下文（block_in_place 前提）
    c.bench_function("sync_surface_w4_bridged_sync_get", |b| {
        b.to_async(&rt_w4)
            .iter(|| async { black_box(bridged_w4.get_sync(black_box(&key)).unwrap()) })
    });
    c.bench_function("sync_surface_w4_bridged_sync_set", |b| {
        b.to_async(&rt_w4).iter(|| async {
            bridged_w4
                .set_sync(black_box(&key), black_box(&value))
                .unwrap()
        })
    });

    // —— worker=1：桥接侵蚀曲线（I/O 型后端在此档有挂起风险，内存型仅吞吐衰减）——
    c.bench_function("sync_surface_w1_bridged_sync_get", |b| {
        b.to_async(&rt_w1)
            .iter(|| async { black_box(bridged_w1.get_sync(black_box(&key)).unwrap()) })
    });
    c.bench_function("sync_surface_w1_bridged_sync_set", |b| {
        b.to_async(&rt_w1).iter(|| async {
            bridged_w1
                .set_sync(black_box(&key), black_box(&value))
                .unwrap()
        })
    });
    c.bench_function("sync_surface_w1_async_get_baseline", |b| {
        b.to_async(&rt_w1).iter(|| async {
            black_box(async_cache_w1.get_by_str(black_box(&key)).await.unwrap());
        })
    });
}

criterion_group!(benches, bench_sync_surfaces);
criterion_main!(benches);
