// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 指标 recorder 热路径开销基准
//!
//! 配置中枢把 recorder 变成逐操作开销决策：`metrics_enabled=false` 注入
//! [`NoOpMetricsRecorder`]，否则默认挂全局 [`UnifiedMetricsRecorder`]
//! （原子计数 + 动态指标表）。本基准量化两条路径在 get/set 热路径上的
//! 吞吐差，为该开关的默认值与容量规划提供数据。

use criterion::{Criterion, criterion_group, criterion_main};
use oxcache::Cache;
use oxcache::infra::NoOpMetricsRecorder;
use std::hint::black_box;
use std::sync::Arc;

async fn seed(cache: &Cache<String, String>) {
    for i in 0..1000u32 {
        cache
            .set_by_str(&format!("bench_key_{i}"), &format!("value_{i}"), None)
            .await
            .unwrap();
    }
}

fn bench_recorder_overhead(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // 默认路径：全局 UnifiedMetricsRecorder（rc.6 起的默认行为）
    let default_cache = rt.block_on(async { Cache::builder().build().await.unwrap() });
    // NoOp 路径：metrics_enabled=false 的等价构建
    let noop_cache = rt.block_on(async {
        Cache::builder()
            .metrics(Arc::new(NoOpMetricsRecorder))
            .build()
            .await
            .unwrap()
    });

    rt.block_on(async {
        seed(&default_cache).await;
        seed(&noop_cache).await;
    });

    c.bench_function("recorder_default_global_get", |b| {
        b.to_async(&rt).iter(|| async {
            let _: Option<String> = default_cache
                .get_by_str(black_box("bench_key_42"))
                .await
                .unwrap();
        });
    });

    c.bench_function("recorder_noop_get", |b| {
        b.to_async(&rt).iter(|| async {
            let _: Option<String> = noop_cache
                .get_by_str(black_box("bench_key_42"))
                .await
                .unwrap();
        });
    });

    c.bench_function("recorder_default_global_set", |b| {
        b.to_async(&rt).iter(|| async {
            default_cache
                .set_by_str(black_box("bench_set_42"), &"v".to_string(), None)
                .await
                .unwrap();
        });
    });

    c.bench_function("recorder_noop_set", |b| {
        b.to_async(&rt).iter(|| async {
            noop_cache
                .set_by_str(black_box("bench_set_42"), &"v".to_string(), None)
                .await
                .unwrap();
        });
    });
}

criterion_group!(benches, bench_recorder_overhead);
criterion_main!(benches);
