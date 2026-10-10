// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 临时执行探针（E2E 场景 172）：`ttl_jitter` 参数边界 —— NaN 视为 0（无抖动）、
//! 越界值钳制到 1.0（不得放大抖动）。

#![cfg(all(feature = "memory", feature = "serialization"))]

use oxcache::Cache;
use std::time::Duration;

const BASE: Duration = Duration::from_secs(60);

#[tokio::test]
async fn ttl_jitter_nan_means_no_jitter() {
    let cache: Cache<String, String> = Cache::builder().ttl_jitter(f64::NAN).build().await.unwrap();
    let k = "nan-key".to_string();
    cache
        .set_with_ttl(&k, &"v".to_string(), Some(BASE))
        .await
        .unwrap();
    let ttl = cache.ttl(&k).await.unwrap().unwrap();
    assert!(
        ttl <= BASE && ttl >= BASE - Duration::from_millis(100),
        "NaN jitter must be treated as 0.0 (exact TTL), got {ttl:?}"
    );
}

#[tokio::test]
async fn ttl_jitter_zero_is_exact() {
    let cache: Cache<String, String> = Cache::builder().ttl_jitter(0.0).build().await.unwrap();
    let k = "zero-key".to_string();
    cache
        .set_with_ttl(&k, &"v".to_string(), Some(BASE))
        .await
        .unwrap();
    let ttl = cache.ttl(&k).await.unwrap().unwrap();
    assert!(
        ttl <= BASE && ttl >= BASE - Duration::from_millis(100),
        "zero jitter must be exact, got {ttl:?}"
    );
}

#[tokio::test]
async fn ttl_jitter_out_of_range_clamped_to_one() {
    let cache: Cache<String, String> = Cache::builder().ttl_jitter(5.0).build().await.unwrap();
    let limit = BASE * 2 + Duration::from_millis(100);
    let mut distinct = std::collections::HashSet::new();
    for i in 0..64 {
        let k = format!("clamp-key-{i}");
        cache
            .set_with_ttl(&k, &"v".to_string(), Some(BASE))
            .await
            .unwrap();
        let ttl = cache.ttl(&k).await.unwrap().unwrap();
        assert!(
            ttl <= limit,
            "clamped factor must stay within 2x base (clamp to 1.0), got {ttl:?}"
        );
        distinct.insert(ttl.as_millis());
    }
    assert!(
        distinct.len() > 1,
        "clamped factor 1.0 must still produce jitter, got {distinct:?}"
    );
}
