// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis客户端覆盖率补充测试
// 目标: 提升源文件 src/backend/client/redis/client.rs 的测试覆盖率
// 当前覆盖率: 52.50% (247行未覆盖)

#[cfg(test)]
#[cfg(feature = "redis")]
mod redis_client_tests {
    use crate::common::{get_redis_url, is_redis_available};
    use oxcache::backend::interface::LuaExecutor;
    use oxcache::backend::memory::redis::{RedisBackend, RedisBackendBuilder, RedisMode};
    use oxcache::backend::score::BackendScore;
    use oxcache::backend::{CacheConnector, CacheReader, CacheWriter};
    use serial_test::serial;
    use std::time::Duration;

    // ============================================================================
    // 测试辅助函数
    // ============================================================================

    /// 创建 Redis 客户端的辅助函数
    async fn create_backend() -> RedisBackend {
        // 设置允许非 TLS 连接
        unsafe {
            std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        }
        let url = get_redis_url();
        let result = RedisBackend::new(&url).await;
        unsafe {
            std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
        }
        result.expect("Redis should be running on localhost:6380")
    }

    /// 检查 Redis 是否可用,如果不可用则跳过测试
    async fn skip_if_redis_unavailable() -> bool {
        if !is_redis_available().await {
            println!("⚠️  Skipping test - Redis not available");
            false
        } else {
            true
        }
    }

    // ============================================================================
    // RedisBackendBuilder 测试
    // ============================================================================

    mod builder_tests {
        use super::*;

        /// 测试 builder 的 connection_string 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_builder_connection_string() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            let url = get_redis_url();
            let backend = RedisBackendBuilder::default().connection_string(&url).build().await;

            unsafe {
                std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            }

            assert!(backend.is_ok(), "Builder should create backend successfully");
            println!("✅ Builder connection_string test passed");
        }

        /// 测试 builder 的 mode 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_builder_mode_standalone() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            let url = get_redis_url();
            let backend = RedisBackendBuilder::default()
                .connection_string(&url)
                .mode(RedisMode::Standalone)
                .build()
                .await
                .expect("Backend creation should succeed");

            unsafe {
                std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            }

            assert_eq!(backend.mode(), RedisMode::Standalone);
            println!("✅ Builder mode(Standalone) test passed");
        }

        /// 测试 builder 的 mode 方法 - Sentinel 模式
        #[serial(redis)]
        #[tokio::test]
        async fn test_builder_mode_sentinel() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            let url = get_redis_url();
            let backend = RedisBackendBuilder::default()
                .connection_string(&url)
                .mode(RedisMode::Sentinel)
                .build()
                .await
                .expect("Backend creation should succeed");

            unsafe {
                std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            }

            assert_eq!(backend.mode(), RedisMode::Sentinel);
            println!("✅ Builder mode(Sentinel) test passed");
        }

        /// 测试 builder 的 mode 方法 - Cluster 模式
        #[serial(redis)]
        #[tokio::test]
        async fn test_builder_mode_cluster() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            let url = get_redis_url();
            let backend = RedisBackendBuilder::default()
                .connection_string(&url)
                .mode(RedisMode::Cluster)
                .build()
                .await
                .expect("Backend creation should succeed");

            unsafe {
                std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            }

            assert_eq!(backend.mode(), RedisMode::Cluster);
            println!("✅ Builder mode(Cluster) test passed");
        }

        /// 测试 builder 缺少 connection_string 时的错误
        #[tokio::test]
        async fn test_builder_missing_connection_string() {
            let result = RedisBackendBuilder::default().build().await;

            assert!(result.is_err(), "Builder without connection_string should fail");
            match result {
                Err(e) => {
                    assert!(e.to_string().contains("Connection string is required"));
                }
                Ok(_) => panic!("Expected error"),
            }
            println!("✅ Builder missing connection_string error test passed");
        }

        /// 测试 with_pool 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_with_pool() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            let url = get_redis_url();
            let backend = RedisBackend::with_pool(&url, 10).await;

            unsafe {
                std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            }

            assert!(backend.is_ok(), "with_pool should create backend successfully");
            println!("✅ with_pool test passed");
        }
    }

    // ============================================================================
    // 批量 Pipeline 操作测试
    // ============================================================================

    mod pipeline_tests {
        use super::*;

        /// 测试 set_many_pipeline - 基本
        #[serial(redis)]
        #[tokio::test]
        async fn test_set_many_pipeline_basic() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let items = vec![
                ("pipeline_key1", b"value1".to_vec()),
                ("pipeline_key2", b"value2".to_vec()),
                ("pipeline_key3", b"value3".to_vec()),
            ];

            let result = backend.set_many_pipeline(&items, None).await;
            assert!(result.is_ok(), "set_many_pipeline should succeed");

            // 验证所有键都已设置
            for (key, value) in &items {
                let retrieved = backend.get(key).await.expect("GET should succeed");
                assert_eq!(retrieved, Some(value.clone()), "Value should match for key {}", key);
            }

            // 清理
            for (key, _) in &items {
                backend.delete(key).await.ok();
            }

            println!("✅ set_many_pipeline basic test passed");
        }

        /// 测试 set_many_pipeline - 带 TTL
        #[serial(redis)]
        #[tokio::test]
        async fn test_set_many_pipeline_with_ttl() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let items = vec![
                ("ttl_pipeline_key1", b"value1".to_vec()),
                ("ttl_pipeline_key2", b"value2".to_vec()),
            ];

            let result = backend.set_many_pipeline(&items, Some(Duration::from_secs(2))).await;
            assert!(result.is_ok(), "set_many_pipeline with TTL should succeed");

            // 验证 TTL 已设置
            for (key, _) in &items {
                let ttl = backend.ttl(key).await.expect("TTL should succeed");
                assert!(ttl.is_some(), "TTL should be set for key {}", key);
                let ttl_val = ttl.unwrap();
                assert!(
                    ttl_val.as_secs() > 0 && ttl_val.as_secs() <= 2,
                    "TTL should be <= 2 seconds"
                );
            }

            // 等待过期
            tokio::time::sleep(Duration::from_millis(2100)).await;

            // 验证键已过期
            for (key, _) in &items {
                let retrieved = backend.get(key).await.expect("GET should succeed");
                assert!(retrieved.is_none(), "Key {} should be expired", key);
            }

            println!("✅ set_many_pipeline with TTL test passed");
        }

        /// 测试 set_many_pipeline - 空列表
        #[serial(redis)]
        #[tokio::test]
        async fn test_set_many_pipeline_empty() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let items: Vec<(&str, Vec<u8>)> = vec![];
            let result = backend.set_many_pipeline(&items, None).await;

            assert!(result.is_ok(), "set_many_pipeline with empty list should succeed");
            println!("✅ set_many_pipeline empty list test passed");
        }

        /// 测试 get_many_pipeline - 所有键存在
        #[serial(redis)]
        #[tokio::test]
        async fn test_get_many_pipeline_all_exist() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            // 先设置一些键
            let items = vec![
                ("get_pipeline_key1", b"value1".to_vec()),
                ("get_pipeline_key2", b"value2".to_vec()),
                ("get_pipeline_key3", b"value3".to_vec()),
            ];

            for (key, value) in &items {
                backend.set(key, value.clone(), None).await.unwrap();
            }

            // 批量获取
            let keys = vec!["get_pipeline_key1", "get_pipeline_key2", "get_pipeline_key3"];
            let result = backend.get_many_pipeline(&keys).await;

            assert!(result.is_ok(), "get_many_pipeline should succeed");
            let values = result.unwrap();
            assert_eq!(values.len(), 3, "Should return 3 values");
            assert_eq!(values[0], Some(b"value1".to_vec()));
            assert_eq!(values[1], Some(b"value2".to_vec()));
            assert_eq!(values[2], Some(b"value3".to_vec()));

            // 清理
            for key in &keys {
                backend.delete(key).await.ok();
            }

            println!("✅ get_many_pipeline all exist test passed");
        }

        /// 测试 get_many_pipeline - 部分键不存在
        #[serial(redis)]
        #[tokio::test]
        async fn test_get_many_pipeline_partial_missing() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            // 只设置一个键
            backend.set("partial_key1", b"value1".to_vec(), None).await.unwrap();

            // 批量获取,包含不存在的键
            let keys = vec!["partial_key1", "nonexistent_key1", "nonexistent_key2"];
            let result = backend.get_many_pipeline(&keys).await;

            assert!(result.is_ok(), "get_many_pipeline should succeed");
            let values = result.unwrap();
            assert_eq!(values.len(), 3, "Should return 3 values");
            assert_eq!(values[0], Some(b"value1".to_vec()));
            assert_eq!(values[1], None, "Nonexistent key should return None");
            assert_eq!(values[2], None, "Nonexistent key should return None");

            // 清理
            backend.delete("partial_key1").await.ok();

            println!("✅ get_many_pipeline partial missing test passed");
        }

        /// 测试 get_many_pipeline - 空列表
        #[serial(redis)]
        #[tokio::test]
        async fn test_get_many_pipeline_empty() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let keys: Vec<&str> = vec![];
            let result = backend.get_many_pipeline(&keys).await;

            assert!(result.is_ok(), "get_many_pipeline with empty list should succeed");
            let values = result.unwrap();
            assert!(values.is_empty(), "Should return empty vector");

            println!("✅ get_many_pipeline empty list test passed");
        }

        /// 测试 delete_many_pipeline
        #[serial(redis)]
        #[tokio::test]
        async fn test_delete_many_pipeline() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            // 先设置一些键
            let keys = vec!["del_pipeline_key1", "del_pipeline_key2", "del_pipeline_key3"];
            for key in &keys {
                backend.set(key, b"value".to_vec(), None).await.unwrap();
            }

            // 验证键存在
            for key in &keys {
                assert!(backend.exists(key).await.unwrap(), "Key {} should exist", key);
            }

            // 批量删除
            let result = backend.delete_many_pipeline(&keys).await;
            assert!(result.is_ok(), "delete_many_pipeline should succeed");

            // 验证键已删除
            for key in &keys {
                assert!(!backend.exists(key).await.unwrap(), "Key {} should be deleted", key);
            }

            println!("✅ delete_many_pipeline test passed");
        }

        /// 测试 delete_many_pipeline - 空列表
        #[serial(redis)]
        #[tokio::test]
        async fn test_delete_many_pipeline_empty() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let keys: Vec<&str> = vec![];
            let result = backend.delete_many_pipeline(&keys).await;

            assert!(result.is_ok(), "delete_many_pipeline with empty list should succeed");
            println!("✅ delete_many_pipeline empty list test passed");
        }
    }

    // ============================================================================
    // CacheBackend trait 方法测试
    // ============================================================================

    mod cache_backend_tests {
        use super::*;

        /// 测试 capacity 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_capacity() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let capacity = backend.capacity().await;
            assert!(capacity.is_ok(), "capacity should succeed");
            // Redis capacity 返回 0 (无限制)
            assert_eq!(capacity.unwrap(), 0, "Redis capacity should be 0 (unlimited)");

            println!("✅ capacity test passed");
        }

        /// 测试 set_many 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_set_many() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let items = vec![
                (
                    "set_many_key1".to_string(),
                    b"value1".to_vec(),
                    Some(Duration::from_secs(60)),
                ),
                (
                    "set_many_key2".to_string(),
                    b"value2".to_vec(),
                    Some(Duration::from_secs(60)),
                ),
            ];

            let result = backend.set_many(&items).await;
            assert!(result.is_ok(), "set_many should succeed");

            // 验证键已设置
            for (key, value, _) in &items {
                let retrieved = backend.get(key).await.unwrap();
                assert_eq!(retrieved, Some(value.clone()));
            }

            // 清理
            for (key, _, _) in &items {
                backend.delete(key).await.ok();
            }

            println!("✅ set_many test passed");
        }

        /// 测试 set_many - 空列表
        #[serial(redis)]
        #[tokio::test]
        async fn test_set_many_empty() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let items: Vec<(String, Vec<u8>, Option<Duration>)> = vec![];
            let result = backend.set_many(&items).await;

            assert!(result.is_ok(), "set_many with empty list should succeed");
            println!("✅ set_many empty list test passed");
        }

        /// 测试 get_many 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_get_many() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            // 设置一些键
            backend.set("get_many_key1", b"value1".to_vec(), None).await.unwrap();
            backend.set("get_many_key2", b"value2".to_vec(), None).await.unwrap();

            let keys = vec![
                "get_many_key1".to_string(),
                "get_many_key2".to_string(),
                "nonexistent".to_string(),
            ];

            let result = backend.get_many(&keys).await;
            assert!(result.is_ok(), "get_many should succeed");

            let values = result.unwrap();
            assert_eq!(values.len(), 3);
            assert_eq!(values[0], Some(b"value1".to_vec()));
            assert_eq!(values[1], Some(b"value2".to_vec()));
            assert_eq!(values[2], None);

            // 清理
            backend.delete("get_many_key1").await.ok();
            backend.delete("get_many_key2").await.ok();

            println!("✅ get_many test passed");
        }

        /// 测试 get_many - 空列表
        #[serial(redis)]
        #[tokio::test]
        async fn test_get_many_empty() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let keys: Vec<String> = vec![];
            let result = backend.get_many(&keys).await;

            assert!(result.is_ok(), "get_many with empty list should succeed");
            let values = result.unwrap();
            assert!(values.is_empty());

            println!("✅ get_many empty list test passed");
        }

        /// 测试 delete_many 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_delete_many() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            // 设置一些键
            backend.set("del_many_key1", b"value1".to_vec(), None).await.unwrap();
            backend.set("del_many_key2", b"value2".to_vec(), None).await.unwrap();

            let keys = vec!["del_many_key1".to_string(), "del_many_key2".to_string()];

            let result = backend.delete_many(&keys).await;
            assert!(result.is_ok(), "delete_many should succeed");

            // 验证键已删除
            assert!(!backend.exists("del_many_key1").await.unwrap());
            assert!(!backend.exists("del_many_key2").await.unwrap());

            println!("✅ delete_many test passed");
        }

        /// 测试 delete_many - 空列表
        #[serial(redis)]
        #[tokio::test]
        async fn test_delete_many_empty() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let keys: Vec<String> = vec![];
            let result = backend.delete_many(&keys).await;

            assert!(result.is_ok(), "delete_many with empty list should succeed");
            println!("✅ delete_many empty list test passed");
        }
    }

    // ============================================================================
    // RedisBackend 特有方法测试
    // ============================================================================

    mod redis_backend_methods {
        use super::*;

        /// 测试 mode 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_mode_method() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            let backend = create_backend().await;
            let mode = backend.mode();

            unsafe {
                std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            }

            assert_eq!(mode, RedisMode::Standalone);
            println!("✅ mode method test passed");
        }

        /// 测试 client 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_client_method() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            let backend = create_backend().await;
            let _client = backend.client();

            unsafe {
                std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            }

            println!("✅ client method test passed");
        }

        /// 测试 ping 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_ping_method() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            let backend = create_backend().await;
            let result = backend.ping().await;

            unsafe {
                std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            }

            assert!(result.is_ok(), "ping should succeed");
            assert_eq!(result.unwrap(), "PONG");

            println!("✅ ping method test passed");
        }
    }

    // ============================================================================
    // BackendScore trait 测试
    // ============================================================================

    mod backend_score_tests {
        use super::*;

        /// 测试 score 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_score() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;
            let score = backend.score();

            // Redis 的分数应该是预定义的值
            assert!(score > 0, "Redis score should be positive");
            println!("✅ score test passed (score = {})", score);
        }

        /// 测试 is_persistent 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_is_persistent() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;
            let persistent = backend.is_persistent();

            // Redis 是持久化存储
            assert!(persistent, "Redis should be persistent");
            println!("✅ is_persistent test passed");
        }

        /// 测试 backend_name 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_backend_name() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;
            let name = backend.backend_name();

            assert_eq!(name, "redis");
            println!("✅ backend_name test passed");
        }
    }

    // ============================================================================
    // Lua 脚本 SHA 相关测试 (需要 lua-script feature)
    // ============================================================================

    #[cfg(feature = "lua-script")]
    mod lua_script_sha_tests {
        use super::*;

        /// 测试 script_load 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_script_load() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            // 加载一个简单的脚本
            let script = "return redis.call('GET', KEYS[1])";
            let result = backend.script_load(script).await;

            assert!(result.is_ok(), "script_load should succeed");

            let sha = result.unwrap();
            assert_eq!(sha.len(), 40, "SHA should be 40 characters");
            assert!(sha.chars().all(|c| c.is_ascii_hexdigit()), "SHA should be hexadecimal");

            println!("✅ script_load test passed (SHA = {})", sha);
        }

        /// 测试 eval_sha 方法
        #[serial(redis)]
        #[tokio::test]
        async fn test_eval_sha() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            // 设置一个测试键
            backend
                .set("sha_test_key", b"sha_test_value".to_vec(), None)
                .await
                .unwrap();

            // 加载脚本
            let script = "return redis.call('GET', KEYS[1])";
            let sha = backend.script_load(script).await.expect("script_load should succeed");

            // 使用 SHA 执行脚本
            let result = backend.eval_sha(&sha, &["sha_test_key"], &[]).await;

            assert!(result.is_ok(), "eval_sha should succeed");

            // 清理
            backend.delete("sha_test_key").await.ok();

            println!("✅ eval_sha test passed");
        }

        /// 测试 eval_sha - 无效 SHA 格式
        #[serial(redis)]
        #[tokio::test]
        async fn test_eval_sha_invalid_format() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            // 尝试使用无效的 SHA
            let result = backend.eval_sha("invalid_sha", &["key"], &[]).await;

            assert!(result.is_err(), "eval_sha with invalid SHA should fail");
            let err = result.unwrap_err();
            assert!(err.to_string().contains("Invalid SHA format"));

            println!("✅ eval_sha invalid format test passed");
        }

        /// 测试 eval_sha - SHA 太短
        #[serial(redis)]
        #[tokio::test]
        async fn test_eval_sha_short_sha() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let result = backend.eval_sha("abc123", &["key"], &[]).await;

            assert!(result.is_err(), "eval_sha with short SHA should fail");
            let err = result.unwrap_err();
            assert!(err.to_string().contains("40"));

            println!("✅ eval_sha short SHA test passed");
        }

        /// 测试 eval_sha - SHA 太长
        #[serial(redis)]
        #[tokio::test]
        async fn test_eval_sha_long_sha() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let long_sha = "a".repeat(50);
            let result = backend.eval_sha(&long_sha, &["key"], &[]).await;

            assert!(result.is_err(), "eval_sha with long SHA should fail");

            println!("✅ eval_sha long SHA test passed");
        }

        /// 测试 eval_sha - SHA 包含非十六进制字符
        #[serial(redis)]
        #[tokio::test]
        async fn test_eval_sha_non_hex_chars() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let result = backend
                .eval_sha("ghijklmnopqrstuvwxyz12345678901234567890", &["key"], &[])
                .await;

            assert!(result.is_err(), "eval_sha with non-hex chars should fail");

            println!("✅ eval_sha non-hex chars test passed");
        }
    }

    // ============================================================================
    // 错误处理测试
    // ============================================================================

    mod error_handling_tests {
        use super::*;

        /// 测试无效键名的验证
        #[serial(redis)]
        #[tokio::test]
        async fn test_invalid_key_with_newline() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            // 包含换行符的键应该被拒绝
            let result = backend.set("key\nwith\nnewlines", b"value".to_vec(), None).await;

            assert!(result.is_err(), "Key with newline should be rejected");
            println!("✅ Invalid key with newline test passed");
        }

        /// 测试无效键名 - 空键
        #[serial(redis)]
        #[tokio::test]
        async fn test_empty_key() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let result = backend.set("", b"value".to_vec(), None).await;

            assert!(result.is_err(), "Empty key should be rejected");
            println!("✅ Empty key test passed");
        }

        /// 测试无效键名 - 包含空字符
        #[serial(redis)]
        #[tokio::test]
        async fn test_key_with_null_char() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let result = backend.set("key\0withnull", b"value".to_vec(), None).await;

            assert!(result.is_err(), "Key with null char should be rejected");
            println!("✅ Key with null char test passed");
        }

        /// 测试连接到无效主机
        #[tokio::test]
        async fn test_connection_to_invalid_host() {
            // 设置允许不安全连接
            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            let result = RedisBackend::new("redis://nonexistent-host-example-test:6379").await;

            assert!(result.is_err(), "Connection to invalid host should fail");

            unsafe {
                std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            }
            println!("✅ Connection to invalid host test passed");
        }
    }

    // ============================================================================
    // 边缘情况测试
    // ============================================================================

    mod edge_case_tests {
        use super::*;

        /// 测试 TTL 命令对不存在的键
        #[serial(redis)]
        #[tokio::test]
        async fn test_ttl_nonexistent_key() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let ttl = backend.ttl("nonexistent_ttl_key").await;
            assert!(ttl.is_ok(), "TTL for nonexistent key should succeed");
            assert!(ttl.unwrap().is_none(), "TTL for nonexistent key should be None");

            println!("✅ TTL nonexistent key test passed");
        }

        /// 测试 expire 对不存在的键
        #[serial(redis)]
        #[tokio::test]
        async fn test_expire_nonexistent_key() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let result = backend.expire("nonexistent_expire_key", Duration::from_secs(60)).await;
            assert!(result.is_ok(), "EXPIRE for nonexistent key should succeed");
            assert!(!result.unwrap(), "EXPIRE for nonexistent key should return false");

            println!("✅ EXPIRE nonexistent key test passed");
        }

        /// 测试对已过期键的 TTL
        #[serial(redis)]
        #[tokio::test]
        async fn test_ttl_expired_key() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            let backend = create_backend().await;

            // 设置一个合理的 TTL (2 秒)
            backend
                .set("expire_quick_key", b"value".to_vec(), Some(Duration::from_secs(2)))
                .await
                .unwrap();

            // 验证 TTL 已设置
            let ttl = backend.ttl("expire_quick_key").await;
            assert!(ttl.is_ok());
            let ttl_val = ttl.unwrap();
            assert!(ttl_val.is_some(), "TTL should be set");
            assert!(ttl_val.unwrap().as_secs() <= 2, "TTL should be <= 2 seconds");

            // 等待过期
            tokio::time::sleep(Duration::from_millis(2100)).await;

            // TTL 应该返回 None (键已过期或不存在)
            let ttl = backend.ttl("expire_quick_key").await;
            assert!(ttl.is_ok());
            assert!(ttl.unwrap().is_none(), "TTL for expired key should be None");

            unsafe {
                std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            }
            println!("✅ TTL expired key test passed");
        }

        /// 测试 is_empty 对空数据库
        #[serial(redis)]
        #[tokio::test]
        async fn test_is_empty_behavior() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            // is_empty 检查数据库是否有键
            let is_empty = backend.is_empty().await;
            assert!(is_empty.is_ok(), "is_empty should succeed");

            println!("✅ is_empty behavior test passed");
        }

        /// 测试 stats 返回内存信息
        #[serial(redis)]
        #[tokio::test]
        async fn test_stats_returns_memory_info() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            let stats = backend.stats().await.expect("stats should succeed");

            // 应该包含 memory_info 键
            assert!(stats.contains_key("memory_info"), "stats should contain memory_info");

            println!("✅ stats returns memory info test passed");
        }
    }

    // ============================================================================
    // 并发测试
    // ============================================================================

    mod concurrency_tests {
        use super::*;

        /// 测试并发 set_many_pipeline 操作
        #[serial(redis)]
        #[tokio::test]
        async fn test_concurrent_pipeline_operations() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;
            let mut handles = vec![];

            // 启动多个并发任务
            for i in 0..5 {
                let backend_clone = backend.clone();
                handles.push(tokio::spawn(async move {
                    let key1 = format!("concurrent_key_{}_1", i);
                    let key2 = format!("concurrent_key_{}_2", i);
                    let val1 = format!("value_{}_1", i).into_bytes();
                    let val2 = format!("value_{}_2", i).into_bytes();

                    let items = vec![(key1.as_str(), val1.clone()), (key2.as_str(), val2.clone())];
                    backend_clone.set_many_pipeline(&items, None).await.unwrap();

                    // 验证
                    let retrieved1 = backend_clone.get(&key1).await.unwrap();
                    let retrieved2 = backend_clone.get(&key2).await.unwrap();
                    assert_eq!(retrieved1, Some(val1));
                    assert_eq!(retrieved2, Some(val2));

                    // 清理
                    backend_clone.delete(&key1).await.ok();
                    backend_clone.delete(&key2).await.ok();
                }));
            }

            // 等待所有任务完成
            for handle in handles {
                handle.await.unwrap();
            }

            println!("✅ Concurrent pipeline operations test passed");
        }

        /// 测试并发 get_many_pipeline 操作
        #[serial(redis)]
        #[tokio::test]
        async fn test_concurrent_get_many_pipeline() {
            if !skip_if_redis_unavailable().await {
                return;
            }

            let backend = create_backend().await;

            // 预先设置一些键
            let keys: Vec<String> = (0..10).map(|i| format!("concurrent_get_{}", i)).collect();
            for (i, key) in keys.iter().enumerate() {
                backend
                    .set(key, format!("value_{}", i).into_bytes(), None)
                    .await
                    .unwrap();
            }

            let mut handles = vec![];

            // 启动多个并发读取任务
            for _ in 0..5 {
                let backend_clone = backend.clone();
                let keys_ref: Vec<String> = keys.clone();
                handles.push(tokio::spawn(async move {
                    let keys_slice: Vec<&str> = keys_ref.iter().map(|s| s.as_str()).collect();
                    let result = backend_clone.get_many_pipeline(&keys_slice).await.unwrap();
                    assert_eq!(result.len(), 10);
                }));
            }

            for handle in handles {
                handle.await.unwrap();
            }

            // 清理
            for key in &keys {
                backend.delete(key).await.ok();
            }

            println!("✅ Concurrent get_many_pipeline test passed");
        }
    }
}
