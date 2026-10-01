// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 批量 API 与 UnifiedSerializer 口径一致性测试（审计 F13 回归）。
//!
//! 钉住的行为：`set_many`/`get_many` 必须经 `UnifiedSerializer`，与单条
//! `set`/`get` 同口径；配置二进制格式（bincode）后批量与单条互相可读。

#![cfg(feature = "serde-bincode")]
#![allow(missing_docs)]

use oxcache::Cache;
use oxcache::infra::serialization::SerializationFormat;
use std::collections::HashMap;
use std::time::Duration;

async fn bincode_cache() -> Cache<String, String> {
    Cache::builder()
        .serialization_format(SerializationFormat::Bincode)
        .build()
        .await
        .unwrap()
}

#[tokio::test]
async fn set_many_then_single_get_across_binary_format() {
    let cache = bincode_cache().await;

    let k1 = "batch-a".to_string();
    let v1 = "value-a".to_string();
    let k2 = "batch-b".to_string();
    let v2 = "value-b".to_string();
    let items = vec![(&k1, &v1), (&k2, &v2)];
    cache.set_many(items).await.unwrap();

    // 单条 get 必须能读回批量写入的数据（修复前：set_many 硬编码 JSON，
    // bincode 单条 get 反序列化失败）
    let a: Option<String> = cache.get(&"batch-a".to_string()).await.unwrap();
    assert_eq!(a, Some("value-a".to_string()));
    let b: Option<String> = cache.get(&"batch-b".to_string()).await.unwrap();
    assert_eq!(b, Some("value-b".to_string()));
}

#[tokio::test]
async fn single_set_then_get_many_across_binary_format() {
    let cache = bincode_cache().await;

    cache
        .set(&"solo".to_string(), &"solo-value".to_string())
        .await
        .unwrap();

    let solo_key = "solo".to_string();
    let keys = vec![&solo_key];
    let got: HashMap<String, String> = cache.get_many(keys).await.unwrap();
    assert_eq!(got.get("solo").map(String::as_str), Some("solo-value"));
}

#[tokio::test]
async fn set_many_with_ttl_applies_jittered_ttl() {
    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

    let ttl_key = "ttl-key".to_string();
    let ttl_value = "ttl-value".to_string();
    let items = vec![(&ttl_key, &ttl_value)];
    cache
        .set_many_with_ttl(items, Some(Duration::from_secs(60)))
        .await
        .unwrap();

    let ttl = cache
        .ttl(&"ttl-key".to_string())
        .await
        .unwrap()
        .expect("ttl 应存在");
    // 默认抖动 ±10%：60s → [54s, 66s)
    assert!(
        ttl >= Duration::from_secs(54) && ttl < Duration::from_secs(66),
        "ttl {ttl:?} 应在抖动区间 [54s, 66s)"
    );
}
