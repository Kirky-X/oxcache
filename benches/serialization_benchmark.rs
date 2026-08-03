// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 序列化路径基准测试（requires `--features serialization`）
//!
//! 测试 JsonSerializer / UnifiedSerializer 在有、无压缩下的序列化与反序列化
//! 性能，以及内嵌序列化对 Cache set/get 路径的叠加开销。

use criterion::{Criterion, criterion_group, criterion_main};
use oxcache::infra::serialization::{JsonSerializer, Serializer};
use std::hint::black_box;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
    tags: Vec<String>,
    active: bool,
    scores: Vec<f64>,
}

fn sample_user() -> User {
    User {
        id: 42,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        tags: vec![
            "admin".to_string(),
            "cache".to_string(),
            "bench".to_string(),
            "memory".to_string(),
        ],
        active: true,
        scores: (0..20).map(|i| i as f64 * 0.5).collect(),
    }
}

fn bench_serialize_plain(c: &mut Criterion) {
    let serializer = JsonSerializer::new();
    let data = black_box(
        oxcache::infra::serialization::unified::UnifiedSerializer::json()
            .serialize(&sample_user())
            .unwrap(),
    );

    c.bench_function("serialize_json_plain", |b| {
        b.iter(|| {
            let out = serializer.serialize(black_box("User"), black_box(&data)).unwrap();
            black_box(out);
        });
    });

    let serialized = serializer.serialize("User", &data).unwrap();
    c.bench_function("deserialize_json_plain", |b| {
        b.iter(|| {
            let out = serializer
                .deserialize(black_box("User"), black_box(&serialized))
                .unwrap();
            black_box(out);
        });
    });
}

fn bench_serialize_compressed(c: &mut Criterion) {
    let serializer = JsonSerializer::with_compression();
    let data = black_box(
        oxcache::infra::serialization::unified::UnifiedSerializer::json()
            .serialize(&sample_user())
            .unwrap(),
    );

    c.bench_function("serialize_json_compressed", |b| {
        b.iter(|| {
            let out = serializer.serialize(black_box("User"), black_box(&data)).unwrap();
            black_box(out);
        });
    });

    let serialized = serializer.serialize("User", &data).unwrap();
    c.bench_function("deserialize_json_compressed", |b| {
        b.iter(|| {
            let out = serializer
                .deserialize(black_box("User"), black_box(&serialized))
                .unwrap();
            black_box(out);
        });
    });
}

criterion_group!(benches, bench_serialize_plain, bench_serialize_compressed);
criterion_main!(benches);
