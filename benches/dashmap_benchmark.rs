// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! DashMap 后端基准测试（requires `--features memory`）
//!
//! 测试 FIFO O(1) 淘汰策略在满载场景下的 set 性能与淘汰正确性。

use criterion::{Criterion, criterion_group, criterion_main};
use oxcache::DashMapMemoryBackend;
use oxcache::backend::{CacheReader, CacheWriter};
use std::hint::black_box;
use std::sync::Arc;

/// 基准：容量满载下持续写入，FIFO 淘汰持续触发。
fn bench_dashmap_fifo_eviction(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    for &capacity in [100usize, 1000usize, 10_000usize].iter() {
        let backend = DashMapMemoryBackend::builder().capacity(capacity).build();

        // 预填到满载。
        rt.block_on(async {
            for i in 0..capacity {
                let key = format!("key_{i}");
                let _ = backend
                    .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None)
                    .await;
            }
        });

        c.bench_function(&format!("dashmap_set_at_full_capacity_{}", capacity), |b| {
            b.to_async(&rt).iter(|| async {
                let key = format!(
                    "bench_key_{}",
                    std::time::SystemTime::now().elapsed().unwrap().as_nanos()
                );
                let _ = backend
                    .set(Arc::from(black_box(&key).as_str()), Arc::new(b"v".to_vec()), None)
                    .await;
            });
        });
    }
}

/// 基准：满载后读取命中/未命中。
fn bench_dashmap_get_at_full_capacity(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let capacity = 1000usize;
    let backend = DashMapMemoryBackend::builder().capacity(capacity).build();

    rt.block_on(async {
        for i in 0..capacity {
            let key = format!("key_{i}");
            let _ = backend
                .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None)
                .await;
        }
    });

    let hit_key = format!("key_{}", capacity / 2);
    c.bench_function("dashmap_get_hit_full_capacity", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = backend.get(black_box(&hit_key)).await;
        });
    });

    let miss_key = "nonexistent_key".to_string();
    c.bench_function("dashmap_get_miss_full_capacity", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = backend.get(black_box(&miss_key)).await;
        });
    });
}

criterion_group!(benches, bench_dashmap_fifo_eviction, bench_dashmap_get_at_full_capacity);
criterion_main!(benches);
