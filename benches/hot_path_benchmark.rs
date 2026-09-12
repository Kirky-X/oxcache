// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 热路径零分配基准
//!
//! 对比 `Cache::get`（`K::to_key_string()` 每次分配 String）与
//! `Cache::get_by_str`（借用查询，零堆分配）以及 `set` / `set_by_str`
//! 的吞吐差异。`get_by_str` 是热路径 API。

use criterion::{Criterion, criterion_group, criterion_main};
use oxcache::Cache;
use std::hint::black_box;

fn bench_hot_path_borrowed_keys(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let cache = rt.block_on(async { Cache::builder().build().await.unwrap() });

    rt.block_on(async {
        for i in 0..1000u32 {
            let key = format!("bench_key_{i}");
            cache.set_by_str(&key, &format!("value_{i}"), None).await.unwrap();
        }
    });

    // 既有路径：owned String 键（每次 get 分配 1 次 String）
    c.bench_function("hot_path_get_owned_key", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!("bench_key_{}", black_box(42u32));
            let _: Option<String> = cache.get(&key).await.unwrap();
        });
    });

    // 新路径：借用键（get 热路径零分配）
    c.bench_function("hot_path_get_borrowed_key", |b| {
        b.to_async(&rt).iter(|| async {
            let _: Option<String> =
                cache.get_by_str(black_box("bench_key_42")).await.unwrap();
        });
    });

    // 写路径对比
    c.bench_function("hot_path_set_owned_key", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!("bench_set_{}", black_box(42u32));
            cache.set(&key, &"v".to_string()).await.unwrap();
        });
    });

    c.bench_function("hot_path_set_borrowed_key", |b| {
        b.to_async(&rt).iter(|| async {
            cache
                .set_by_str(black_box("bench_set_42"), &"v".to_string(), None)
                .await
                .unwrap();
        });
    });
}

criterion_group!(benches, bench_hot_path_borrowed_keys);
criterion_main!(benches);
