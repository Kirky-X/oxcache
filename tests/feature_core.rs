// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// 窄特性验证：core feature only（L1 memory + L2 Redis，不含 macros/compression/cli）
// 迁移自 examples/feature_matrix/core_feature

#![cfg(feature = "core")]

use oxcache::Cache;

#[tokio::test]
async fn core_feature_compiles() {
    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
    cache.set(&"k".into(), &"v".into()).await.unwrap();
    assert_eq!(cache.get(&"k".into()).await.unwrap().unwrap(), "v");
}
