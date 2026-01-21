#![allow(deprecated)]
//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis 原生操作集成测试
//!
//! 测试 Redis 有序集合、Lua 脚本、批量操作等原生功能。

#[cfg(feature = "l2-redis")]
mod tests {
    use oxcache::backend::l2::L2Backend;
    use oxcache::client::redis_native::RedisNativeOps;
    use oxcache::config::L2Config;
    use secrecy::SecretString;
    use std::collections::HashMap;
    use std::sync::Arc;

    // 获取测试后端
    async fn get_test_backend() -> Arc<dyn RedisNativeOps> {
        let config = L2Config {
            connection_string: SecretString::new("redis://127.0.0.1:6379".to_string()),
            mode: oxcache::config::RedisMode::Standalone,
            default_ttl: Some(300),
            ..Default::default()
        };

        let backend: L2Backend = L2Backend::new(&config)
            .await
            .expect("Failed to create L2 backend");
        Arc::new(backend)
    }

    /// 清理测试数据
    async fn cleanup_test_keys(backend: &dyn RedisNativeOps, pattern: &str) {
        let _ = backend.del_pattern(pattern).await;
    }

    // ============================================================================
    // 有序集合操作测试
    // ============================================================================

    #[tokio::test]
    async fn test_zadd_basic() {
        let backend = get_test_backend().await;
        let test_key = "oxcache:test:zadd:basic";

        cleanup_test_keys(&*backend, "oxcache:test:zadd:*").await;

        // 添加单个成员
        let result = backend.zadd(test_key, 1.0, "member1", None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        // 验证成员数量
        let count = backend.zcard(test_key).await;
        assert!(count.is_ok());
        assert_eq!(count.unwrap(), 1);

        cleanup_test_keys(&*backend, "oxcache:test:zadd:*").await;
    }

    #[tokio::test]
    async fn test_zrange_by_score() {
        let backend = get_test_backend().await;
        let test_key = "oxcache:test:zrange:test";

        cleanup_test_keys(&*backend, "oxcache:test:zrange:*").await;

        // 添加测试数据
        for i in 1..=10 {
            let _ = backend
                .zadd(test_key, i as f64, &format!("member{}", i), None)
                .await;
        }

        // 获取分数范围内的成员
        let result = backend.zrange_by_score(test_key, 3.0, 7.0, false).await;
        assert!(result.is_ok());
        let members = result.unwrap();
        assert_eq!(members.len(), 5);
        assert!(members.iter().any(|m| m.member == "member3"));
        assert!(members.iter().any(|m| m.member == "member7"));

        cleanup_test_keys(&*backend, "oxcache:test:zrange:*").await;
    }

    #[tokio::test]
    async fn test_zscore() {
        let backend = get_test_backend().await;
        let test_key = "oxcache:test:zscore:test";

        cleanup_test_keys(&*backend, "oxcache:test:zscore:*").await;

        // 添加成员
        let _ = backend.zadd(test_key, 42.5, "special_member", None).await;

        // 获取分数
        let result = backend.zscore(test_key, "special_member").await;
        assert!(result.is_ok());
        let score = result.unwrap();
        assert!(score.is_some());
        assert!((score.unwrap() - 42.5).abs() < 0.001);

        // 获取不存在的成员分数
        let result = backend.zscore(test_key, "nonexistent").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        cleanup_test_keys(&*backend, "oxcache:test:zscore:*").await;
    }

    #[tokio::test]
    async fn test_zrem() {
        let backend = get_test_backend().await;
        let test_key = "oxcache:test:zrem:test";

        cleanup_test_keys(&*backend, "oxcache:test:zrem:*").await;

        // 添加成员
        let _ = backend.zadd(test_key, 1.0, "toremove", None).await;
        let _ = backend.zadd(test_key, 2.0, "tokeep", None).await;

        // 验证成员存在
        let count_before = backend.zcard(test_key).await.unwrap();
        assert_eq!(count_before, 2);

        // 移除成员
        let result = backend.zrem(test_key, &["toremove"]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        // 验证成员已被移除
        let count_after = backend.zcard(test_key).await.unwrap();
        assert_eq!(count_after, 1);

        cleanup_test_keys(&*backend, "oxcache:test:zrem:*").await;
    }

    // ============================================================================
    // Lua 脚本测试
    // ============================================================================

    #[tokio::test]
    async fn test_script_load_and_evalsha() {
        let backend = get_test_backend().await;
        let test_key = "oxcache:test:script:counter";

        cleanup_test_keys(&*backend, "oxcache:test:script:*").await;

        // 加载 Lua 脚本
        let script = r#"
            local key = KEYS[1]
            local current = redis.call('GET', key) or 0
            local increment = tonumber(ARGV[1])
            local new_value = current + increment
            redis.call('SET', key, new_value)
            return new_value
        "#;

        let result = backend.script_load(script).await;
        assert!(result.is_ok());
        let script_sha = result.unwrap();
        assert!(!script_sha.is_empty());

        // 设置初始值
        let mut kvs = HashMap::new();
        kvs.insert(test_key, b"10" as &[u8]);
        let _ = backend.set_many(kvs, None).await;

        // 执行脚本
        let result = backend.evalsha(&script_sha, &[test_key], &["5"]).await;
        assert!(result.is_ok());
        // 返回值应该是 15
        let result_val: i64 = result.unwrap().parse().unwrap();
        assert_eq!(result_val, 15);

        cleanup_test_keys(&*backend, "oxcache:test:script:*").await;
    }

    #[tokio::test]
    async fn test_eval_direct() {
        let backend = get_test_backend().await;
        let test_key = "oxcache:test:eval:test";

        cleanup_test_keys(&*backend, "oxcache:test:eval:*").await;

        // 直接执行 EVAL
        let script = r#"return redis.call('GET', KEYS[1])"#;
        let mut kvs = HashMap::new();
        kvs.insert(test_key, b"hello" as &[u8]);
        let _ = backend.set_many(kvs, None).await;

        let result = backend.eval_readonly(script, &[test_key], &[]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello");

        cleanup_test_keys(&*backend, "oxcache:test:eval:*").await;
    }

    // ============================================================================
    // 批量操作测试
    // ============================================================================

    #[tokio::test]
    async fn test_get_many() {
        let backend = get_test_backend().await;
        let test_prefix = "oxcache:test:getmany";

        cleanup_test_keys(&*backend, "oxcache:test:getmany:*").await;

        // 批量设置
        for i in 1..=5 {
            let key = format!("{}:{}", test_prefix, i);
            let _ = backend
                .set(&key, format!("value{}", i).as_bytes(), None)
                .await;
        }

        // 批量获取
        let keys: Vec<String> = (1..=5).map(|i| format!("{}:{}", test_prefix, i)).collect();
        let keys_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        let result = backend.get_many(&keys_refs).await;
        assert!(result.is_ok());

        let values = result.unwrap();
        assert_eq!(values.len(), 5);
        // 验证值
        for i in 1..=5 {
            let key = format!("{}:{}", test_prefix, i);
            let expected = format!("value{}", i).into_bytes();
            if let Some(value) = values.get(&key) {
                assert_eq!(value, &expected);
            } else {
                panic!("Key {} not found in result", key);
            }
        }

        cleanup_test_keys(&*backend, "oxcache:test:getmany:*").await;
    }

    #[tokio::test]
    async fn test_set_many() {
        let backend = get_test_backend().await;
        let test_prefix = "oxcache:test:setmany";

        cleanup_test_keys(&*backend, "oxcache:test:setmany:*").await;

        // 批量设置
        for i in 1..=10 {
            let key = format!("{}:{}", test_prefix, i);
            let _ = backend
                .set(&key, format!("batch_value{}", i).as_bytes(), Some(300))
                .await;
        }

        // 验证
        for i in 1..=10 {
            let key = format!("{}:{}", test_prefix, i);
            let value = backend.get_bytes(&key).await.unwrap();
            assert!(value.is_some());
            let actual = String::from_utf8(value.unwrap()).unwrap();
            assert_eq!(actual, format!("batch_value{}", i));
        }

        cleanup_test_keys(&*backend, "oxcache:test:setmany:*").await;
    }

    #[tokio::test]
    async fn test_del_pattern() {
        let backend = get_test_backend().await;
        let test_prefix = "oxcache:test:delpattern";

        cleanup_test_keys(&*backend, "oxcache:test:delpattern:*").await;

        // 创建测试数据
        for i in 1..=20 {
            let key = format!("{}:item{}", test_prefix, i);
            let _ = backend.set(&key, b"value", None).await;
        }

        // 使用模式删除
        let result = backend.del_pattern(&format!("{}:*", test_prefix)).await;
        assert!(result.is_ok());
        // 应该删除一些键
        assert!(result.unwrap() >= 10); // 至少删除 10 个

        cleanup_test_keys(&*backend, "oxcache:test:delpattern:*").await;
    }

    // ============================================================================
    // 键扫描测试
    // ============================================================================

    #[tokio::test]
    async fn test_scan_keys() {
        let backend = get_test_backend().await;
        let test_prefix = "oxcache:test:scan";

        cleanup_test_keys(&*backend, "oxcache:test:scan:*").await;

        // 创建测试数据
        for i in 1..=50 {
            let key = format!("{}:{}", test_prefix, i);
            let _ = backend.set(&key, b"value", None).await;
        }

        // 扫描键
        let result = backend.scan_keys(&format!("{}:*", test_prefix), 10).await;
        assert!(result.is_ok());

        let keys = result.unwrap();
        // 应该获取到一些键
        assert!(keys.len() >= 10);

        cleanup_test_keys(&*backend, "oxcache:test:scan:*").await;
    }

    // ============================================================================
    // 计数器操作测试
    // ============================================================================

    #[tokio::test]
    async fn test_increment_decrement() {
        let backend = get_test_backend().await;
        let test_key = "oxcache:test:counter";

        cleanup_test_keys(&*backend, "oxcache:test:counter*").await;

        // 测试 increment
        let result = backend.increment(test_key, 10, Some(300)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 10);

        let result = backend.increment(test_key, 5, Some(300)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 15);

        // 测试 decrement
        let result = backend.decrement(test_key, 3, Some(300)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 12);

        // 测试 get_counter
        let result = backend.get_counter(test_key).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(12));

        cleanup_test_keys(&*backend, "oxcache:test:counter*").await;
    }
}
