// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Builder 模式测试
//
// 测试 BackendBuilder 和 TieredCacheBuilder 的基本功能。

#[cfg(feature = "l2-redis")]
mod tests {
    use oxcache::builder::{BackendBuilder, TieredCacheBuilder};

    // ============================================================================
    // BackendBuilder 测试
    // ============================================================================

    #[test]
    fn test_backend_builder_memory() {
        let builder = BackendBuilder::memory();
        assert!(matches!(builder, BackendBuilder::Memory { .. }));
    }

    #[test]
    fn test_backend_builder_redis() {
        let builder = BackendBuilder::redis();
        assert!(matches!(builder, BackendBuilder::Redis { .. }));
    }

    #[test]
    fn test_backend_builder_tiered() {
        let builder = BackendBuilder::tiered();
        assert!(matches!(builder, BackendBuilder::Tiered { .. }));
    }

    #[test]
    fn test_backend_builder_memory_with_capacity() {
        let builder = BackendBuilder::memory().capacity(5000);
        assert!(matches!(builder, BackendBuilder::Memory { capacity, .. } if capacity == 5000));
    }

    #[test]
    fn test_backend_builder_redis_with_connection() {
        let builder = BackendBuilder::redis().connection_string("redis://127.0.0.1:6379");
        assert!(matches!(builder, BackendBuilder::Redis { connection_string, .. } if connection_string.as_ref().map(|s| s.as_str()) == Some("redis://127.0.0.1:6379")));
    }

    #[test]
    fn test_backend_builder_tiered_with_l1_capacity() {
        let builder = BackendBuilder::tiered().l1_capacity(10000);
        assert!(matches!(builder, BackendBuilder::Tiered { l1_capacity, .. } if l1_capacity == 10000));
    }

    #[test]
    fn test_backend_builder_tiered_with_l2_connection() {
        let builder = BackendBuilder::tiered().l2_connection_string("redis://127.0.0.1:6379");
        assert!(matches!(builder, BackendBuilder::Tiered { l2_connection_string, .. } if l2_connection_string.as_ref().map(|s| s.as_str()) == Some("redis://127.0.0.1:6379")));
    }

    #[test]
    fn test_backend_builder_tiered_with_auto_promote() {
        let builder = BackendBuilder::tiered().auto_promote(true);
        assert!(matches!(builder, BackendBuilder::Tiered { auto_promote, .. } if auto_promote));
    }

    // ============================================================================
    // TieredCacheBuilder 测试
    // ============================================================================

    #[test]
    fn test_tiered_cache_builder_default() {
        let builder = TieredCacheBuilder::new();
        // 验证 builder 可以被创建
        let _ = builder;
    }

    #[test]
    fn test_tiered_cache_builder_with_l1_capacity() {
        let builder = TieredCacheBuilder::new().l1_capacity(20000);
        // 验证 builder 可以被创建
        let _ = builder;
    }

    #[test]
    fn test_tiered_cache_builder_with_l2_connection() {
        let builder = TieredCacheBuilder::new().l2_connection_string("redis://127.0.0.1:6379");
        // 验证 builder 可以被创建
        let _ = builder;
    }

    #[test]
    fn test_tiered_cache_builder_with_auto_promote() {
        let builder = TieredCacheBuilder::new().auto_promote(false);
        // 验证 builder 可以被创建
        let _ = builder;
    }

    #[test]
    fn test_tiered_cache_builder_full_config() {
        let builder = TieredCacheBuilder::new()
            .l1_capacity(15000)
            .l2_connection_string("redis://127.0.0.1:6379")
            .auto_promote(true);
        // 验证 builder 可以被创建
        let _ = builder;
    }

    // ============================================================================
    // Builder 链式调用测试
    // ============================================================================

    #[test]
    fn test_backend_builder_chain() {
        let builder = BackendBuilder::tiered()
            .l1_capacity(10000)
            .l2_connection_string("redis://127.0.0.1:6379")
            .auto_promote(true);

        assert!(matches!(builder, BackendBuilder::Tiered { l1_capacity, auto_promote, .. } 
            if l1_capacity == 10000 && auto_promote));
    }

    #[test]
    fn test_tiered_cache_builder_chain() {
        let builder = TieredCacheBuilder::new()
            .l1_capacity(5000)
            .l2_connection_string("redis://127.0.0.1:6379")
            .auto_promote(true);
        // 验证 builder 可以被创建
        let _ = builder;
    }
}