// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis客户端综合测试
// 合并自 redis_client_test.rs 和 redis_client_comprehensive_test.rs
// 测试覆盖：连接管理、基本操作、连接池、Lua脚本、批量操作、Pipeline、错误处理、边缘情况

#[cfg(test)]
#[cfg(feature = "redis")]
mod redis_client_tests {
    use crate::common::{get_redis_url, is_redis_available};
    use oxcache::backend::interface::LuaExecutor;
    use oxcache::backend::memory::redis::{RedisBackend, RedisBackendBuilder, RedisMode};
    use oxcache::backend::score::BackendScore;
    use oxcache::validate_lua_script;
    use serial_test::serial;
    use std::time::Duration;

    // ============================================================================
    // 测试辅助函数
    // ============================================================================

    async fn create_backend() -> RedisBackend {
        unsafe {
            std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        }
        let url = get_redis_url();
        let result = RedisBackend::new(&url).await;
        result.expect("Redis should be running on localhost:6380")
    }

    async fn skip_if_redis_unavailable() -> bool {
        if !is_redis_available().await {
            println!("Skipping test - Redis not available");
            false
        } else {
            true
        }
    }

    // ============================================================================
    // Builder 测试
    // ============================================================================

    mod builder_tests {
        use super::*;

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
            assert!(backend.is_ok());
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_builder_modes() {
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
                .expect("Standalone should work");
            assert_eq!(backend.mode(), RedisMode::Standalone);

            let backend = RedisBackendBuilder::default()
                .connection_string(&url)
                .mode(RedisMode::Sentinel)
                .build()
                .await
                .expect("Sentinel should work");
            assert_eq!(backend.mode(), RedisMode::Sentinel);

            let backend = RedisBackendBuilder::default()
                .connection_string(&url)
                .mode(RedisMode::Cluster)
                .build()
                .await
                .expect("Cluster should work");
            assert_eq!(backend.mode(), RedisMode::Cluster);
        }

        #[tokio::test]
        async fn test_builder_missing_connection_string() {
            let result = RedisBackendBuilder::default().build().await;
            assert!(result.is_err());
        }

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
            assert!(backend.is_ok());
        }
    }

    // ============================================================================
    // 基本操作测试
    // ============================================================================

    mod basic_operations {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_connection_establishment() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            backend.health_check().await.expect("Health check failed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_basic_set_get() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            backend.set("test_key", b"test_value".to_vec(), None).await.unwrap();
            let value = backend.get("test_key").await.unwrap();
            assert_eq!(value, Some(b"test_value".to_vec()));
            backend.delete("test_key").await.ok();
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_delete() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            backend.set("key_to_delete", b"value".to_vec(), None).await.unwrap();
            backend.delete("key_to_delete").await.unwrap();
            assert!(backend.get("key_to_delete").await.unwrap().is_none());
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_ttl() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            backend
                .set("key_with_ttl", b"value".to_vec(), Some(Duration::from_secs(1)))
                .await
                .unwrap();
            assert!(backend.get("key_with_ttl").await.unwrap().is_some());
            tokio::time::sleep(Duration::from_millis(1100)).await;
            assert!(backend.get("key_with_ttl").await.unwrap().is_none());
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_nonexistent_key() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            assert!(backend.get("nonexistent_key").await.unwrap().is_none());
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_overwrite_key() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            backend.set("key", b"value1".to_vec(), None).await.unwrap();
            backend.set("key", b"value2".to_vec(), None).await.unwrap();
            assert_eq!(backend.get("key").await.unwrap(), Some(b"value2".to_vec()));
            backend.delete("key").await.ok();
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_exists() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            assert!(!backend.exists("test_exists_key").await.unwrap());
            backend.set("test_exists_key", b"value".to_vec(), None).await.unwrap();
            assert!(backend.exists("test_exists_key").await.unwrap());
            backend.delete("test_exists_key").await.ok();
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_expire_command() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            backend.set("expire_test_key", b"value".to_vec(), None).await.unwrap();
            let result = backend.expire("expire_test_key", Duration::from_secs(1)).await.unwrap();
            assert!(result);
            assert!(backend.ttl("expire_test_key").await.unwrap().is_some());
            tokio::time::sleep(Duration::from_millis(1100)).await;
            assert!(backend.get("expire_test_key").await.unwrap().is_none());
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_empty_and_large_values() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;

            backend.set("empty_key", b"".to_vec(), None).await.unwrap();
            assert_eq!(backend.get("empty_key").await.unwrap(), Some(b"".to_vec()));
            backend.delete("empty_key").await.ok();

            let large_value = vec![0u8; 1024 * 1024];
            backend.set("large_key", large_value.clone(), None).await.unwrap();
            assert_eq!(backend.get("large_key").await.unwrap(), Some(large_value));
            backend.delete("large_key").await.ok();
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_special_characters_in_key() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            let special_keys = [
                "test:key:with:colons",
                "test_key_with_underscores",
                "test-key-with-dashes",
                "test.key.with.dots",
            ];
            for key in &special_keys {
                backend.set(key, b"value".to_vec(), None).await.unwrap();
                assert_eq!(backend.get(key).await.unwrap(), Some(b"value".to_vec()));
                backend.delete(key).await.ok();
            }
        }
    }

    // ============================================================================
    // Pipeline 批量操作测试
    // ============================================================================

    mod pipeline_tests {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_set_many_pipeline() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            let items = vec![
                ("pipeline_key1", b"value1".to_vec()),
                ("pipeline_key2", b"value2".to_vec()),
                ("pipeline_key3", b"value3".to_vec()),
            ];
            backend.set_many_pipeline(&items, None).await.unwrap();
            for (key, value) in &items {
                assert_eq!(backend.get(key).await.unwrap(), Some(value.clone()));
            }
            for (key, _) in &items {
                backend.delete(key).await.ok();
            }
        }

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
            backend
                .set_many_pipeline(&items, Some(Duration::from_secs(2)))
                .await
                .unwrap();
            for (key, _) in &items {
                let ttl = backend.ttl(key).await.unwrap();
                assert!(ttl.is_some() && ttl.unwrap().as_secs() <= 2);
            }
            tokio::time::sleep(Duration::from_millis(2100)).await;
            for (key, _) in &items {
                assert!(backend.get(key).await.unwrap().is_none());
            }
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_get_many_pipeline() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            backend
                .set("get_pipeline_key1", b"value1".to_vec(), None)
                .await
                .unwrap();
            backend
                .set("get_pipeline_key2", b"value2".to_vec(), None)
                .await
                .unwrap();
            let keys = vec!["get_pipeline_key1", "get_pipeline_key2", "nonexistent"];
            let values = backend.get_many_pipeline(&keys).await.unwrap();
            assert_eq!(values.len(), 3);
            assert_eq!(values[0], Some(b"value1".to_vec()));
            assert_eq!(values[1], Some(b"value2".to_vec()));
            assert_eq!(values[2], None);
            for key in &keys {
                backend.delete(key).await.ok();
            }
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_delete_many_pipeline() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            let keys = vec!["del_pipeline_key1", "del_pipeline_key2", "del_pipeline_key3"];
            for key in &keys {
                backend.set(key, b"value".to_vec(), None).await.unwrap();
            }
            backend.delete_many_pipeline(&keys).await.unwrap();
            for key in &keys {
                assert!(!backend.exists(key).await.unwrap());
            }
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_empty_pipeline_operations() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            assert!(backend
                .set_many_pipeline(&Vec::<(&str, Vec<u8>)>::new(), None)
                .await
                .is_ok());
            assert!(backend.get_many_pipeline(&Vec::<&str>::new()).await.unwrap().is_empty());
            assert!(backend.delete_many_pipeline(&Vec::<&str>::new()).await.is_ok());
        }
    }

    // ============================================================================
    // CacheBackend 批量操作测试
    // ============================================================================

    mod batch_operations {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_set_many_get_many_delete_many() {
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
            backend.set_many(&items).await.unwrap();
            for (key, value, _) in &items {
                assert_eq!(backend.get(key).await.unwrap(), Some(value.clone()));
            }

            let keys = vec![
                "set_many_key1".to_string(),
                "set_many_key2".to_string(),
                "nonexistent".to_string(),
            ];
            let values = backend.get_many(&keys).await.unwrap();
            assert_eq!(values.len(), 3);
            assert_eq!(values[0], Some(b"value1".to_vec()));
            assert_eq!(values[1], Some(b"value2".to_vec()));
            assert_eq!(values[2], None);

            backend.delete_many(&keys).await.unwrap();
            assert!(!backend.exists("set_many_key1").await.unwrap());
            assert!(!backend.exists("set_many_key2").await.unwrap());
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_empty_batch_operations() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            assert!(backend
                .set_many(&Vec::<(String, Vec<u8>, Option<Duration>)>::new())
                .await
                .is_ok());
            assert!(backend.get_many(&Vec::<String>::new()).await.unwrap().is_empty());
            assert!(backend.delete_many(&Vec::<String>::new()).await.is_ok());
        }
    }

    // ============================================================================
    // BackendScore 和 Redis 特有方法测试
    // ============================================================================

    mod backend_methods {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_mode_client_ping() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            assert_eq!(backend.mode(), RedisMode::Standalone);
            let _client = backend.client();
            assert_eq!(backend.ping().await.unwrap(), "PONG");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_score_and_metadata() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            assert!(backend.score() > 0);
            assert!(backend.is_persistent());
            assert_eq!(backend.backend_name(), "redis");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_capacity_and_len() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            assert_eq!(backend.capacity().await.unwrap(), 0);
            let _len = backend.len().await.unwrap();
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_stats() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            let stats = backend.stats().await.unwrap();
            assert!(!stats.is_empty());
            assert!(stats.contains_key("memory_info"));
        }
    }

    // ============================================================================
    // Lua 脚本测试
    // ============================================================================

    mod lua_scripts {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_basic_lua_script() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            backend.set("counter", b"0".to_vec(), None).await.unwrap();
            let script = r#"local current = redis.call('GET', KEYS[1]); current = tonumber(current); redis.call('SET', KEYS[1], current + 1); return current + 1"#;
            backend.eval_lua(script, &["counter"], &[]).await.unwrap();
            assert_eq!(backend.get("counter").await.unwrap(), Some(b"1".to_vec()));
            backend.delete("counter").await.ok();
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_lua_script_conditional_set() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            let script = r#"if redis.call('EXISTS', KEYS[1]) == 0 then redis.call('SET', KEYS[1], ARGV[1]); return 1 else return 0 end"#;
            let result = backend.eval_lua(script, &["unique_key"], &["value1"]).await.unwrap();
            match result {
                redis::Value::Int(i) => assert_eq!(i, 1),
                redis::Value::BulkString(data) => assert_eq!(data.as_slice(), b"1"),
                _ => panic!("Unexpected result"),
            }
            let result = backend.eval_lua(script, &["unique_key"], &["value2"]).await.unwrap();
            match result {
                redis::Value::Int(i) => assert_eq!(i, 0),
                redis::Value::BulkString(data) => assert_eq!(data.as_slice(), b"0"),
                _ => panic!("Unexpected result"),
            }
            assert_eq!(backend.get("unique_key").await.unwrap(), Some(b"value1".to_vec()));
            backend.delete("unique_key").await.ok();
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_lua_script_error() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            assert!(backend.eval_lua("invalid lua syntax", &["key"], &[]).await.is_err());
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_lua_script_with_multiple_args() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            backend.set("arg_test_1", b"0".to_vec(), None).await.unwrap();
            backend.set("arg_test_2", b"0".to_vec(), None).await.unwrap();
            let script = r#"local sum = 0; for i, key in ipairs(KEYS) do local val = redis.call('GET', key); val = tonumber(val) or 0; local add = tonumber(ARGV[i]) or 0; sum = sum + val + add end; return sum"#;
            let result = backend
                .eval_lua(script, &["arg_test_1", "arg_test_2"], &["5", "10"])
                .await
                .unwrap();
            match result {
                redis::Value::Int(i) => assert_eq!(i, 15),
                redis::Value::BulkString(data) => assert_eq!(data.as_slice(), b"15"),
                _ => panic!("Unexpected result"),
            }
            backend.delete("arg_test_1").await.ok();
            backend.delete("arg_test_2").await.ok();
        }

        #[test]
        fn test_lua_script_validation() {
            let valid_script = r#"return redis.call('GET', KEYS[1])"#;
            assert!(validate_lua_script(valid_script, 1).is_ok());
            let dangerous_script = r#"return redis.call('FLUSHALL')"#;
            assert!(validate_lua_script(dangerous_script, 0).is_err());
            let long_script = "local x = 1\n".repeat(10000);
            assert!(validate_lua_script(&long_script, 1).is_err());
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_sorted_set_via_lua() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            let test_key = "test:sorted:set1";
            let zadd_script = r#"redis.call('ZADD', KEYS[1], tonumber(ARGV[1]), ARGV[2]); return 1"#;
            backend
                .eval_lua(zadd_script, &[test_key], &["1.0", "member1"])
                .await
                .unwrap();
            let zrange_script = r#"return redis.call('ZRANGE', KEYS[1], tonumber(ARGV[1]), tonumber(ARGV[2]))"#;
            backend
                .eval_lua(zrange_script, &[test_key], &["0", "-1"])
                .await
                .unwrap();
            let _ = backend.delete(test_key).await;
        }
    }

    // ============================================================================
    // 错误处理和边缘情况测试
    // ============================================================================

    mod error_handling {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_invalid_keys() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            assert!(backend
                .set("key\nwith\nnewlines", b"value".to_vec(), None)
                .await
                .is_err());
            assert!(backend.set("", b"value".to_vec(), None).await.is_err());
            assert!(backend.set("key\0withnull", b"value".to_vec(), None).await.is_err());
        }

        #[tokio::test]
        async fn test_connection_to_invalid_host() {
            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }
            assert!(RedisBackend::new("redis://nonexistent-host-test:6379").await.is_err());
        }
    }

    mod edge_cases {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_ttl_operations() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            assert!(backend.ttl("nonexistent_ttl_key").await.unwrap().is_none());
            assert!(!backend
                .expire("nonexistent_expire_key", Duration::from_secs(60))
                .await
                .unwrap());

            backend
                .set("expire_quick_key", b"value".to_vec(), Some(Duration::from_secs(2)))
                .await
                .unwrap();
            assert!(backend.ttl("expire_quick_key").await.unwrap().is_some());
            tokio::time::sleep(Duration::from_millis(2100)).await;
            assert!(backend.ttl("expire_quick_key").await.unwrap().is_none());
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_binary_and_unicode_values() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;

            let binary_value: Vec<u8> = (0u8..=255).collect();
            backend.set("binary_key", binary_value.clone(), None).await.unwrap();
            assert_eq!(backend.get("binary_key").await.unwrap(), Some(binary_value));
            backend.delete("binary_key").await.ok();

            let unicode_values = ["Hello 世界", "こんにちは世界", "🎉🎊🎈"];
            for value in &unicode_values {
                let value_bytes = value.as_bytes().to_vec();
                backend.set("unicode_key", value_bytes.clone(), None).await.unwrap();
                assert_eq!(backend.get("unicode_key").await.unwrap(), Some(value_bytes));
            }
            backend.delete("unicode_key").await.ok();
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_very_long_key() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            let long_key = format!("long_key:{}", "a".repeat(1000));
            backend.set(&long_key, b"value".to_vec(), None).await.unwrap();
            assert_eq!(backend.get(&long_key).await.unwrap(), Some(b"value".to_vec()));
            backend.delete(&long_key).await.ok();
        }
    }

    // ============================================================================
    // 连接管理和清理测试
    // ============================================================================

    mod connection_management {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_connection_pool() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            let mut handles = vec![];
            for i in 0..20 {
                let backend_clone = backend.clone();
                handles.push(tokio::spawn(async move {
                    let key = format!("concurrent_key_{}", i);
                    let value = format!("value_{}", i).into_bytes();
                    backend_clone.set(&key, value.clone(), None).await.unwrap();
                    assert_eq!(backend_clone.get(&key).await.unwrap(), Some(value));
                }));
            }
            for (i, handle) in handles.into_iter().enumerate() {
                handle.await.unwrap();
                backend.delete(&format!("concurrent_key_{}", i)).await.ok();
            }
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_client_clone() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            let backend_clone = backend.clone();
            backend.set("clone_test", b"original".to_vec(), None).await.unwrap();
            assert_eq!(
                backend_clone.get("clone_test").await.unwrap(),
                Some(b"original".to_vec())
            );
            backend_clone.set("clone_test", b"cloned".to_vec(), None).await.unwrap();
            assert_eq!(backend.get("clone_test").await.unwrap(), Some(b"cloned".to_vec()));
            backend.delete("clone_test").await.ok();
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_shutdown() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            backend.shutdown().await;
        }
    }

    // ============================================================================
    // 并发测试
    // ============================================================================

    mod concurrency {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_concurrent_pipeline_operations() {
            if !skip_if_redis_unavailable().await {
                return;
            }
            let backend = create_backend().await;
            let mut handles = vec![];
            for i in 0..5 {
                let backend_clone = backend.clone();
                handles.push(tokio::spawn(async move {
                    let key1 = format!("concurrent_key_{}_1", i);
                    let key2 = format!("concurrent_key_{}_2", i);
                    let val1 = format!("value_{}_1", i).into_bytes();
                    let val2 = format!("value_{}_2", i).into_bytes();
                    let items = vec![(key1.as_str(), val1.clone()), (key2.as_str(), val2.clone())];
                    backend_clone.set_many_pipeline(&items, None).await.unwrap();
                    assert_eq!(backend_clone.get(&key1).await.unwrap(), Some(val1));
                    assert_eq!(backend_clone.get(&key2).await.unwrap(), Some(val2));
                    backend_clone.delete(&key1).await.ok();
                    backend_clone.delete(&key2).await.ok();
                }));
            }
            for handle in handles {
                handle.await.unwrap();
            }
        }
    }
}
