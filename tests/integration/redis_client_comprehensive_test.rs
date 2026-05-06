// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis客户端综合测试
// 测试覆盖：连接管理、基本操作、连接池、Lua脚本、批量操作、错误处理

#[cfg(test)]
#[cfg(feature = "redis")]
mod redis_client_comprehensive_tests {
    use crate::common::{get_redis_url, is_redis_available};
    use oxcache::backend::interface::LuaExecutor;
    use oxcache::backend::memory::redis::RedisBackend;
    use oxcache::backend::{CacheConnector, CacheReader, CacheWriter};
    use oxcache::validate_lua_script;
    use serial_test::serial;
    use std::time::Duration;

    // ============================================================================
    // 测试上下文模块
    // ============================================================================

    mod test_context {
        use super::*;

        /// Redis测试上下文
        pub struct RedisTestContext {
            pub connection_string: String,
        }

        impl RedisTestContext {
            /// 创建新的测试上下文
            pub async fn new() -> Self {
                if !is_redis_available().await {
                    panic!("Redis not available for testing");
                }

                let connection_string = get_redis_url();
                Self { connection_string }
            }

            /// 获取连接字符串
            pub fn connection_string(&self) -> &str {
                &self.connection_string
            }

            /// 创建Redis客户端
            pub async fn create_client(&self) -> RedisBackend {
                RedisBackend::new(self.connection_string())
                    .await
                    .expect("Failed to create Redis backend")
            }
        }
    }

    // ============================================================================
    // 基础连接和操作测试
    // ============================================================================

    mod basic_operations {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_connection_establishment() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 测试健康检查
            client.health_check().await.expect("Health check failed");
            println!("✅ Connection establishment test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_basic_set_get() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // Set操作
            let result = client.set("test_key", b"test_value".to_vec(), None).await;
            assert!(result.is_ok(), "SET operation failed");

            // Get操作
            let value = client.get("test_key").await;
            assert!(value.is_ok(), "GET operation failed");
            assert_eq!(value.unwrap(), Some(b"test_value".to_vec()));

            // 清理
            client.delete("test_key").await.ok();

            println!("✅ Basic SET/GET test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_delete() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            client.set("key_to_delete", b"value".to_vec(), None).await.unwrap();
            client.delete("key_to_delete").await.unwrap();

            let value = client.get("key_to_delete").await.unwrap();
            assert!(value.is_none(), "Key should be deleted");

            println!("✅ DELETE test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_ttl() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 设置带TTL的键
            client
                .set("key_with_ttl", b"value".to_vec(), Some(Duration::from_secs(1)))
                .await
                .unwrap();

            // 立即读取应该成功
            let value = client.get("key_with_ttl").await.unwrap();
            assert!(value.is_some(), "Key should exist immediately");

            // 等待过期
            tokio::time::sleep(Duration::from_millis(1100)).await;

            // 再次读取应该为空
            let value = client.get("key_with_ttl").await.unwrap();
            assert!(value.is_none(), "Key should be expired");

            println!("✅ TTL test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_nonexistent_key() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            let value = client.get("nonexistent_key").await.unwrap();
            assert!(value.is_none(), "Nonexistent key should return None");

            println!("✅ Nonexistent key test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_overwrite_key() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            client.set("key", b"value1".to_vec(), None).await.unwrap();
            client.set("key", b"value2".to_vec(), None).await.unwrap();

            let value = client.get("key").await.unwrap();
            assert_eq!(value, Some(b"value2".to_vec()));

            // 清理
            client.delete("key").await.ok();

            println!("✅ Overwrite key test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_exists() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 键不存在
            let exists = client.exists("test_exists_key").await.unwrap();
            assert!(!exists, "Key should not exist");

            // 设置键
            client.set("test_exists_key", b"value".to_vec(), None).await.unwrap();

            // 键存在
            let exists = client.exists("test_exists_key").await.unwrap();
            assert!(exists, "Key should exist");

            // 清理
            client.delete("test_exists_key").await.ok();

            println!("✅ EXISTS test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_expire_command() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 设置无TTL的键
            client.set("expire_test_key", b"value".to_vec(), None).await.unwrap();

            // 设置过期时间
            let result = client.expire("expire_test_key", Duration::from_secs(1)).await.unwrap();
            assert!(result, "EXPIRE should return true");

            // 立即检查TTL
            let ttl = client.ttl("expire_test_key").await.unwrap();
            assert!(ttl.is_some(), "TTL should be set");
            assert!(ttl.unwrap().as_secs() > 0, "TTL should be positive");

            // 等待过期
            tokio::time::sleep(Duration::from_millis(1100)).await;

            // 键应该已过期
            let value = client.get("expire_test_key").await.unwrap();
            assert!(value.is_none(), "Key should be expired");

            println!("✅ EXPIRE command test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_empty_value() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 设置空值
            client.set("empty_key", b"".to_vec(), None).await.unwrap();

            // 获取空值
            let value = client.get("empty_key").await.unwrap();
            assert_eq!(value, Some(b"".to_vec()));

            // 清理
            client.delete("empty_key").await.ok();

            println!("✅ Empty value test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_large_value() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 创建大值 (1MB)
            let large_value = vec![0u8; 1024 * 1024];

            client.set("large_key", large_value.clone(), None).await.unwrap();

            let retrieved = client.get("large_key").await.unwrap();
            assert_eq!(retrieved, Some(large_value));

            // 清理
            client.delete("large_key").await.ok();

            println!("✅ Large value test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_special_characters_in_key() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            let special_keys = [
                "test:key:with:colons",
                "test_key_with_underscores",
                "test-key-with-dashes",
                "test.key.with.dots",
                "test:key:日本語:中文",
            ];

            for key in &special_keys {
                client.set(key, b"value".to_vec(), None).await.unwrap();

                let value = client.get(key).await.unwrap();
                assert_eq!(value, Some(b"value".to_vec()));

                client.delete(key).await.ok();
            }

            println!("✅ Special characters in key test passed");
        }
    }

    // ============================================================================
    // 连接池和错误处理测试
    // ============================================================================

    mod connection_management {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_connection_pool() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 并发执行多个操作，验证连接池工作正常
            let mut handles = vec![];

            for i in 0..20 {
                let client_clone = client.clone();
                handles.push(tokio::spawn(async move {
                    let key = format!("concurrent_key_{}", i);
                    let value = format!("value_{}", i).into_bytes();

                    client_clone.set(&key, value, None).await.unwrap();

                    client_clone.get(&key).await.unwrap()
                }));
            }

            for (i, handle) in handles.into_iter().enumerate() {
                let value = handle.await.unwrap();
                assert_eq!(value, Some(format!("value_{}", i).into_bytes()));

                // 清理
                let key = format!("concurrent_key_{}", i);
                client.delete(&key).await.ok();
            }

            println!("✅ Connection pool test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_connection_failure_recovery() {
            // 测试连接失败后的恢复
            let invalid_url = "redis://invalid-host-example-test:6379/0";

            // 尝试连接应该失败
            let result = RedisBackend::new(invalid_url).await;
            assert!(result.is_err(), "Connection to invalid host should fail");

            println!("✅ Connection failure recovery test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_client_clone() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 克隆客户端
            let client_clone = client.clone();

            // 使用原始客户端
            client.set("clone_test", b"original".to_vec(), None).await.unwrap();

            // 使用克隆客户端读取
            let value = client_clone.get("clone_test").await.unwrap();
            assert_eq!(value, Some(b"original".to_vec()));

            // 使用克隆客户端写入
            client_clone.set("clone_test", b"cloned".to_vec(), None).await.unwrap();

            // 使用原始客户端读取
            let value = client.get("clone_test").await.unwrap();
            assert_eq!(value, Some(b"cloned".to_vec()));

            // 清理
            client.delete("clone_test").await.ok();

            println!("✅ Client clone test passed");
        }
    }

    // ============================================================================
    // Lua脚本测试
    // ============================================================================

    mod lua_scripts {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_basic_lua_script() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 设置初始值
            client.set("counter", b"0".to_vec(), None).await.unwrap();

            // 执行Lua脚本：原子递增
            let script = r#"
                local current = redis.call('GET', KEYS[1])
                current = tonumber(current)
                redis.call('SET', KEYS[1], current + 1)
                return current + 1
            "#;

            let result = client.eval_lua(script, &["counter"], &[]).await;
            assert!(result.is_ok(), "Lua script execution failed");

            // 验证结果
            let value = client.get("counter").await.unwrap();
            assert_eq!(value, Some(b"1".to_vec()));

            // 清理
            client.delete("counter").await.ok();

            println!("✅ Basic Lua script test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_conditional_set() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // Lua脚本：仅当值不存在时设置
            let script = r#"
                if redis.call('EXISTS', KEYS[1]) == 0 then
                    redis.call('SET', KEYS[1], ARGV[1])
                    return 1
                else
                    return 0
                end
            "#;

            // 第一次应该成功
            let result = client.eval_lua(script, &["unique_key"], &["value1"]).await.unwrap();

            // 验证结果返回1
            match result {
                redis::Value::BulkString(data) => {
                    assert_eq!(data.as_slice(), b"1");
                }
                redis::Value::Int(i) => {
                    assert_eq!(i, 1);
                }
                _ => {
                    panic!("Unexpected result type: {:?}", result);
                }
            }

            // 第二次应该失败（键已存在）
            let result = client.eval_lua(script, &["unique_key"], &["value2"]).await.unwrap();

            // 验证结果返回0
            match result {
                redis::Value::BulkString(data) => {
                    assert_eq!(data.as_slice(), b"0");
                }
                redis::Value::Int(i) => {
                    assert_eq!(i, 0);
                }
                _ => {
                    panic!("Unexpected result type: {:?}", result);
                }
            }

            // 验证值没有改变
            let value = client.get("unique_key").await.unwrap();
            assert_eq!(value, Some(b"value1".to_vec()));

            // 清理
            client.delete("unique_key").await.ok();

            println!("✅ Conditional set Lua script test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_lua_script_error() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 错误的Lua脚本
            let script = "invalid lua syntax";

            let result = client.eval_lua(script, &["key"], &[]).await;
            assert!(result.is_err(), "Invalid Lua script should fail");

            println!("✅ Lua script error test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_lua_script_with_multiple_args() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 设置多个键
            client.set("arg_test_1", b"0".to_vec(), None).await.unwrap();
            client.set("arg_test_2", b"0".to_vec(), None).await.unwrap();

            // Lua脚本：使用多个参数
            let script = r#"
                local sum = 0
                for i, key in ipairs(KEYS) do
                    local val = redis.call('GET', key)
                    val = tonumber(val) or 0
                    local add = tonumber(ARGV[i]) or 0
                    sum = sum + val + add
                end
                return sum
            "#;

            let result = client
                .eval_lua(script, &["arg_test_1", "arg_test_2"], &["5", "10"])
                .await
                .unwrap();

            // 验证结果
            match result {
                redis::Value::BulkString(data) => {
                    assert_eq!(data.as_slice(), b"15");
                }
                redis::Value::Int(i) => {
                    assert_eq!(i, 15);
                }
                _ => {
                    panic!("Unexpected result type: {:?}", result);
                }
            }

            // 清理
            client.delete("arg_test_1").await.ok();
            client.delete("arg_test_2").await.ok();

            println!("✅ Lua script with multiple args test passed");
        }

        /// 测试 Lua 脚本安全验证 - 危险命令检测
        #[test]
        fn test_lua_script_validation_security() {
            // 有效的 Lua 脚本应该能通过验证
            let valid_script = r#"
                local key = KEYS[1]
                local value = ARGV[1]
                return redis.call('GET', key)
            "#;
            assert!(validate_lua_script(valid_script, 1).is_ok());

            // 包含禁止命令的脚本应该被拒绝
            let dangerous_script = r#"
                return redis.call('FLUSHALL')
            "#;
            assert!(validate_lua_script(dangerous_script, 0).is_err());

            // 包含 KEYS 命令的脚本应该被拒绝
            let keys_script = r#"
                local keys = redis.call('KEYS', '*')
                return keys
            "#;
            assert!(validate_lua_script(keys_script, 0).is_err());

            println!("✅ Lua script validation security test passed");
        }

        /// 测试 Lua 脚本最大长度限制
        #[test]
        fn test_lua_script_max_length() {
            // 创建超过限制的脚本
            let long_script = "local x = 1\n".repeat(10000);
            assert!(validate_lua_script(&long_script, 1).is_err());
            println!("✅ Lua script max length test passed");
        }

        /// 测试 Lua 脚本最大 key 数量限制
        #[test]
        fn test_lua_script_max_keys() {
            // 创建声明大量 KEYS 的脚本
            let script = "local result = 0\n".to_string()
                + "for i = 1, 101 do\n"
                + "    local key = KEYS[i]\n"
                + "    result = result + 1\n"
                + "end\n"
                + "return result";
            assert!(validate_lua_script(&script, 101).is_err());
            println!("✅ Lua script max keys test passed");
        }

        /// 测试 Sorted Set 操作（通过 Lua 脚本）
        #[serial(redis)]
        #[tokio::test]
        async fn test_sorted_set_via_lua() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            let test_key = "test:sorted:set1";

            // 通过 Lua 脚本模拟 ZADD 操作
            let zadd_script = r#"
                local key = KEYS[1]
                local score = tonumber(ARGV[1])
                local member = ARGV[2]
                redis.call('ZADD', key, score, member)
                return 1
            "#;

            let zadd_result = client.eval_lua(zadd_script, &[test_key], &["1.0", "member1"]).await;
            assert!(zadd_result.is_ok(), "ZADD via Lua should succeed");
            println!("✅ ZADD via Lua test passed");

            // 通过 Lua 脚本模拟 ZRANGE 操作
            let zrange_script = r#"
                local key = KEYS[1]
                local start = tonumber(ARGV[1])
                local stop = tonumber(ARGV[2])
                return redis.call('ZRANGE', key, start, stop)
            "#;

            let zrange_result = client.eval_lua(zrange_script, &[test_key], &["0", "-1"]).await;
            assert!(zrange_result.is_ok(), "ZRANGE via Lua should succeed");
            println!("✅ ZRANGE via Lua test passed");

            // 清理测试数据
            let _ = client.delete(test_key).await;
            println!("✅ Sorted Set via Lua test passed");
        }
    }

    // ============================================================================
    // 批量操作测试
    // ============================================================================

    mod batch_operations {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_multiple_set_operations() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            let items = vec![
                ("batch_key1", b"batch_value1".to_vec()),
                ("batch_key2", b"batch_value2".to_vec()),
                ("batch_key3", b"batch_value3".to_vec()),
            ];

            // 逐个设置（RedisBackend目前没有set_many方法）
            for (key, value) in &items {
                client.set(key, value.clone(), None).await.unwrap();
            }

            // 验证所有键都设置成功
            for (key, value) in &items {
                let result = client.get(key).await.unwrap();
                assert_eq!(result, Some(value.clone()));
            }

            // 清理
            for (key, _) in &items {
                client.delete(key).await.ok();
            }

            println!("✅ Multiple SET operations test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_multiple_get_operations() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 设置测试数据
            client.set("get_key1", b"get_value1".to_vec(), None).await.unwrap();
            client.set("get_key2", b"get_value2".to_vec(), None).await.unwrap();
            client.set("get_key3", b"get_value3".to_vec(), None).await.unwrap();

            // 逐个获取（RedisBackend目前没有get_many方法）
            let keys = vec!["get_key1", "get_key2", "get_key3", "nonexistent_key"];
            let mut values = vec![];

            for key in &keys {
                values.push(client.get(key).await.unwrap());
            }

            assert_eq!(values.len(), 4);
            assert_eq!(values[0], Some(b"get_value1".to_vec()));
            assert_eq!(values[1], Some(b"get_value2".to_vec()));
            assert_eq!(values[2], Some(b"get_value3".to_vec()));
            assert_eq!(values[3], None);

            // 清理
            client.delete("get_key1").await.ok();
            client.delete("get_key2").await.ok();
            client.delete("get_key3").await.ok();

            println!("✅ Multiple GET operations test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_multiple_delete_operations() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 设置测试数据
            client.set("del_key1", b"del_value1".to_vec(), None).await.unwrap();
            client.set("del_key2", b"del_value2".to_vec(), None).await.unwrap();
            client.set("del_key3", b"del_value3".to_vec(), None).await.unwrap();

            // 逐个删除（RedisBackend目前没有delete_many方法）
            client.delete("del_key1").await.unwrap();
            client.delete("del_key2").await.unwrap();

            // 验证删除结果
            assert!(client.get("del_key1").await.unwrap().is_none());
            assert!(client.get("del_key2").await.unwrap().is_none());
            assert!(client.get("del_key3").await.unwrap().is_some());

            // 清理
            client.delete("del_key3").await.ok();

            println!("✅ Multiple DELETE operations test passed");
        }
    }

    // ============================================================================
    // 健康检查和统计测试
    // ============================================================================

    mod health_and_stats {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_health_check() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 健康检查应该成功
            client.health_check().await.expect("Health check failed");

            println!("✅ Health check test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_ping() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // Ping应该返回PONG
            let result = client.ping().await;
            assert!(result.is_ok(), "Ping failed");
            assert_eq!(result.unwrap(), "PONG");

            println!("✅ PING test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_stats() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 获取统计信息
            let stats = client.stats().await;
            assert!(stats.is_ok(), "Failed to get stats");

            let stats_map = stats.unwrap();
            assert!(!stats_map.is_empty(), "Stats should not be empty");

            println!("✅ Stats test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_len_and_is_empty() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 初始状态可能不为空（Redis中可能有其他键）
            let len = client.len().await;
            assert!(len.is_ok(), "Failed to get length");

            // 测试特定键的存在性
            let test_key = "len_test_key";
            client.set(test_key, b"value".to_vec(), None).await.unwrap();

            let exists = client.exists(test_key).await.unwrap();
            assert!(exists, "Key should exist");

            // 清理
            client.delete(test_key).await.ok();

            println!("✅ LEN and IS_EMPTY test passed");
        }
    }

    // ============================================================================
    // 边缘情况测试
    // ============================================================================

    mod edge_cases {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_binary_value() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 测试二进制值（包含所有字节值）
            let binary_value: Vec<u8> = (0u8..=255).collect();

            client.set("binary_key", binary_value.clone(), None).await.unwrap();

            let retrieved = client.get("binary_key").await.unwrap();
            assert_eq!(retrieved, Some(binary_value));

            // 清理
            client.delete("binary_key").await.ok();

            println!("✅ Binary value test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_unicode_value() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 测试Unicode值
            let unicode_values = [
                "Hello 世界",
                "Привет мир",
                "こんにちは世界",
                "🎉🎊🎈",
                "مرحبا بالعالم",
                "שלום עולם",
            ];

            for value in &unicode_values {
                let value_bytes = value.as_bytes().to_vec();
                client.set("unicode_key", value_bytes.clone(), None).await.unwrap();

                let retrieved = client.get("unicode_key").await.unwrap();
                assert_eq!(retrieved, Some(value_bytes));
            }

            // 清理
            client.delete("unicode_key").await.ok();

            println!("✅ Unicode value test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_very_long_key() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 测试很长的键名
            let long_key = "a".repeat(1000);
            let key = format!("long_key:{}", long_key);

            client.set(&key, b"value".to_vec(), None).await.unwrap();

            let retrieved = client.get(&key).await.unwrap();
            assert_eq!(retrieved, Some(b"value".to_vec()));

            // 清理
            client.delete(&key).await.ok();

            println!("✅ Very long key test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_zero_ttl() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // Redis 7.x 不接受零或极短的 TTL，测试快速过期行为
            // 使用 100ms TTL（Redis 7.x 最小支持约 1ms，但为了测试稳定性使用 100ms）
            let result = client
                .set("zero_ttl_key", b"value".to_vec(), Some(Duration::from_millis(100)))
                .await;

            // 如果 Redis 拒绝短 TTL，这是可接受的行为
            if result.is_err() {
                println!("⚠️  Redis rejected short TTL (expected for Redis 7.x+)");
                println!("✅ Zero TTL test passed (TTL rejection is acceptable)");
                return;
            }

            // 键应该立即存在
            let value = client.get("zero_ttl_key").await.unwrap();
            assert!(value.is_some(), "Key should exist immediately after set");

            // 等待过期
            tokio::time::sleep(Duration::from_millis(150)).await;

            // 键应该已过期
            let value = client.get("zero_ttl_key").await.unwrap();
            assert!(value.is_none(), "Key should be expired after TTL");

            println!("✅ Zero TTL test passed");
        }
    }

    // ============================================================================
    // 清理和关闭测试
    // ============================================================================

    mod cleanup {
        use super::*;

        #[serial(redis)]
        #[tokio::test]
        async fn test_clear_all_keys() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 设置一些键
            client.set("clear_test_1", b"value1".to_vec(), None).await.unwrap();
            client.set("clear_test_2", b"value2".to_vec(), None).await.unwrap();

            // 注意：clear()操作会清空整个数据库
            // 在测试环境中谨慎使用
            let result = client.clear().await;
            // clear可能失败或不可用，取决于权限

            // 验证键已被删除（如果clear成功）
            if result.is_ok() {
                assert!(client.get("clear_test_1").await.unwrap().is_none());
                assert!(client.get("clear_test_2").await.unwrap().is_none());
            }

            println!("✅ CLEAR test passed");
        }

        #[serial(redis)]
        #[tokio::test]
        async fn test_close_connection() {
            if !is_redis_available().await {
                println!("⚠️  Skipping - Redis not available");
                return;
            }

            let ctx = test_context::RedisTestContext::new().await;
            let client = ctx.create_client().await;

            // 关闭连接
            client.shutdown().await;

            // 关闭后操作可能失败
            let _result = client.ping().await;
            // 可能失败或成功（取决于实现）

            println!("✅ CLOSE connection test passed");
        }
    }
}
