// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// 窄特性验证：minimal feature only（L1 memory cache + metrics + serialization）
// 迁移自 examples/feature_matrix/minimal_feature
// 防止 0.3.2 回归：pub mod metrics/serialization 未 cfg-gate

#![cfg(feature = "minimal")]

use oxcache::Cache;

#[tokio::test]
async fn minimal_feature_compiles() {
    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
    cache.set(&"k".into(), &"v".into()).await.unwrap();
    assert_eq!(cache.get(&"k".into()).await.unwrap().unwrap(), "v");

    // 验证 metrics 模块在 minimal feature 下可访问
    let _stats = oxcache::CacheStats::default();
}
