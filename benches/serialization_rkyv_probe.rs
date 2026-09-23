// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 序列化格式对照探针（决策用，absorb-hitbox-features 后续评估）
//!
//! JSON vs postcard vs rkyv 在三档载荷下的序列化 / 反序列化开销与字节体积
//! 对照，并单独测 rkyv 的零拷贝访问路径（`access`，含校验）——该路径只有
//! 在 "L1 直接持有字节 + 暴露 archived 视图" 的 API 形态下才能兑现，当前
//! `Cache<K, V>` 的 `get` 语义对应的是 `deserialize_rkyv_owned`。
//!
//! rkyv / postcard 在本文件中仅为 dev-dependency，不代表已承诺的特性；
//! 本基准的输出用于决定是否立项 `SerializationFormat::Rkyv`。
//!
//! # 实测结论（2026-09-23，WSL2 / criterion 中位数）
//!
//! | 操作（中位时间） | 1KB | 16KB | 256KB |
//! |---|---|---|---|
//! | 序列化 JSON / postcard / rkyv | 1.20µs / 258ns / 281ns | 17.5µs / 2.24µs / 1.81µs | 268µs / 32.7µs / 26.0µs |
//! | 反序列化为自有值（当前 get 语义） | 1.85µs / 613ns / 394ns | 31.4µs / 13.1µs / 11.3µs | 506µs / 205µs / 173µs |
//! | rkyv 零拷贝 access（含校验） | 54ns | 778ns | 12.4µs |
//! | 体积 JSON / postcard / rkyv | 877 / 889 / 1128B | 12.8K / 12.5K / 15.7KB | 222K / 201K / 251KB |
//!
//! 结论：
//! 1. owned 反序列化语义下，rkyv 相对已发布的 postcard 仅快 16–40%——
//!    不值得为它引入 `Archive` derive 的 API 传染，`SerializationFormat::Rkyv`
//!    不立项。
//! 2. rkyv 的真正分野在零拷贝 access（比 postcard owned 快 11–17 倍，
//!    比 JSON owned 快 34–41 倍），但仅在新增 "get 视图回调" 形态 API 时可兑现，
//!    且 rkyv 缓冲比 postcard 大 13–29%（缓存内存税）。
//! 3. 该视图 API 与 tower/HTTP 大响应缓存场景绑定立项，另做安全走查
//!    （validation 错误 → miss 语义）。

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use serde::{Deserialize, Serialize};
use std::hint::black_box;
use std::time::Duration;

#[derive(
    Debug, Clone, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
struct Record {
    id: u64,
    name: String,
    email: String,
    tags: Vec<String>,
    active: bool,
    scores: Vec<f64>,
}

/// 每条 ~310B（JSON 口径），数量按目标名义尺寸缩放
fn records(count: usize) -> Vec<Record> {
    (0..count)
        .map(|i| Record {
            id: i as u64,
            name: format!("user-{i:06}"),
            email: format!("user{i:06}@example.com"),
            tags: vec![
                "admin".to_string(),
                "cache".to_string(),
                "bench".to_string(),
                format!("tag{i}"),
            ],
            active: i % 2 == 0,
            scores: (0..20).map(|j| (i as f64) * 0.5 + j as f64).collect(),
        })
        .collect()
}

/// (名义尺寸, 记录数)
const SIZES: [(&str, usize); 3] = [("1KB", 4), ("16KB", 56), ("256KB", 896)];

fn bench_probe(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization_rkyv_probe");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(2));

    for (label, count) in SIZES {
        let records = records(count);

        let json = serde_json::to_vec(&records).unwrap();
        let postcard_bytes = postcard::to_allocvec(&records).unwrap();
        let rkyv_bytes: rkyv::util::AlignedVec =
            rkyv::api::high::to_bytes::<rkyv::rancor::Panic>(&records)
                .unwrap_or_else(|_| unreachable!());
        // Criterion 会捕获 stdout；--nocapture 下可见
        eprintln!(
            "[sizes {label} n={count}] json={:>7}B  postcard={:>7}B  rkyv={:>7}B",
            json.len(),
            postcard_bytes.len(),
            rkyv_bytes.len()
        );

        // ---- 序列化 ----
        group.bench_with_input(
            BenchmarkId::new("serialize_json", label),
            &records,
            |b, r| b.iter(|| serde_json::to_vec(black_box(r)).unwrap()),
        );
        group.bench_with_input(
            BenchmarkId::new("serialize_postcard", label),
            &records,
            |b, r| b.iter(|| postcard::to_allocvec(black_box(r)).unwrap()),
        );
        group.bench_with_input(
            BenchmarkId::new("serialize_rkyv", label),
            &records,
            |b, r| {
                b.iter(|| {
                    let bytes: rkyv::util::AlignedVec =
                        rkyv::api::high::to_bytes::<rkyv::rancor::Panic>(black_box(r))
                            .unwrap_or_else(|_| unreachable!());
                    black_box(bytes);
                })
            },
        );

        // ---- 反序列化为自有值（当前 Cache::get 的语义：必须重建 V）----
        let j = json.clone();
        group.bench_with_input(BenchmarkId::new("deserialize_json", label), &j, |b, d| {
            b.iter(|| {
                let v: Vec<Record> = serde_json::from_slice(black_box(d)).unwrap();
                black_box(v);
            })
        });
        let p = postcard_bytes.clone();
        group.bench_with_input(
            BenchmarkId::new("deserialize_postcard", label),
            &p,
            |b, d| {
                b.iter(|| {
                    let v: Vec<Record> = postcard::from_bytes(black_box(d)).unwrap();
                    black_box(v);
                })
            },
        );
        let rb = rkyv_bytes.clone();
        group.bench_with_input(
            BenchmarkId::new("deserialize_rkyv_owned", label),
            &rb,
            |b, d| {
                b.iter(|| {
                    let v: Vec<Record> =
                        rkyv::from_bytes::<Vec<Record>, rkyv::rancor::Panic>(black_box(d))
                            .unwrap_or_else(|_| unreachable!());
                    black_box(v);
                })
            },
        );

        // ---- rkyv 零拷贝访问（含校验）：只有 archived 视图 API 才能兑现 ----
        group.bench_with_input(
            BenchmarkId::new("access_rkyv_zero_copy", label),
            &rkyv_bytes,
            |b, d| {
                b.iter(|| {
                    let archived =
                        rkyv::access::<rkyv::Archived<Vec<Record>>, rkyv::rancor::Error>(
                            black_box(d),
                        )
                        .unwrap();
                    black_box(&archived[0].name);
                    black_box(archived.len());
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_probe);
criterion_main!(benches);
