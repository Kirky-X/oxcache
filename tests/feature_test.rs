// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// 窄特性验证：确保各 tier 预设（minimal / core）可正常编译运行。
// 迁移自 examples/feature_matrix/，防止回归。

// ============================================================================
// minimal feature 验证（L1 memory cache + metrics + serialization）
// ============================================================================

#[cfg(feature = "minimal")]
mod minimal_tests {
    use oxcache::Cache;

    #[tokio::test]
    async fn minimal_feature_compiles() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set(&"k".into(), &"v".into()).await.unwrap();
        assert_eq!(cache.get(&"k".into()).await.unwrap().unwrap(), "v");

        // 验证 metrics 模块在 minimal feature 下可访问
        let _stats = oxcache::CacheStats::default();
    }
}

// ============================================================================
// core feature 验证（L1 memory + L2 Redis，不含 macros/compression/cli）
// ============================================================================

#[cfg(feature = "core")]
mod core_tests {
    use oxcache::Cache;

    #[tokio::test]
    async fn core_feature_compiles() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set(&"k".into(), &"v".into()).await.unwrap();
        assert_eq!(cache.get(&"k".into()).await.unwrap().unwrap(), "v");
    }
}
