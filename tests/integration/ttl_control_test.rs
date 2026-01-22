#![allow(deprecated)]
// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// TTL 控制集成测试
//
// 测试 TTL 查询、刷新和 touch 操作。

#[cfg(feature = "l2-redis")]
mod tests {
    use oxcache::backend::l2::L2Backend;
    use oxcache::client::l2::L2Client;
    use oxcache::client::ttl_control::TtlControl;
    use oxcache::client::two_level::TwoLevelClient;
    use oxcache::config::L2Config;
    use oxcache::{CacheExt, CacheOps};
    use secrecy::SecretString;
    use std::sync::Arc;

    // 获取测试 L2Client
    async fn get_test_l2_client() -> Arc<L2Client> {
        let config = L2Config {
            connection_string: SecretString::new("redis://127.0.0.1:6379".to_string()),
            mode: oxcache::config::RedisMode::Standalone,
            default_ttl: Some(300),
            ..Default::default()
        };

        let backend = Arc::new(
            L2Backend::new(&config)
                .await
                .expect("Failed to create L2 backend"),
        );

        Arc::new(
            L2Client::new(
                "test_ttl_service".to_string(),
                backend,
                oxcache::serialization::SerializerEnum::Json(
                    oxcache::serialization::JsonSerializer::new(),
                ),
            )
            .await
            .expect("Failed to create L2 client"),
        )
    }

    // 获取测试 TwoLevelClient（用于 L1 TTL 测试）
    async fn get_test_two_level_client() -> Arc<TwoLevelClient> {
        use oxcache::backend::l1::L1Backend;
        use oxcache::config::{L1Config, TwoLevelConfig};

        let l1_config = L1Config {
            max_capacity: 10000, // 增加容量以容纳多个键
            ..Default::default()
        };

        let l1 = Arc::new(L1Backend::new(l1_config.max_capacity));

        let l2_config = L2Config {
            connection_string: SecretString::new("redis://127.0.0.1:6379".to_string()),
            mode: oxcache::config::RedisMode::Standalone,
            default_ttl: Some(300),
            ..Default::default()
        };

        let l2_backend = Arc::new(
            L2Backend::new(&l2_config)
                .await
                .expect("Failed to create L2 backend"),
        );

        let two_level_config = TwoLevelConfig {
            ..Default::default()
        };

        Arc::new(
            TwoLevelClient::new(
                "test_ttl_service".to_string(),
                two_level_config,
                l1,
                l2_backend,
                oxcache::serialization::SerializerEnum::Json(
                    oxcache::serialization::JsonSerializer::new(),
                ),
            )
            .await
            .expect("Failed to create TwoLevel client"),
        )
    }

    // ============================================================================
    // L1 TTL 测试
    // ============================================================================

    #[tokio::test]
    async fn test_get_l1_ttl_basic() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:l1:ttl:get";

        // 设置带 TTL 的值
        client.set(test_key, &"value", Some(300)).await.unwrap();

        // 获取 L1 TTL
        let ttl = client.get_l1_ttl(test_key).await.unwrap();
        assert!(ttl.is_some());
        assert!(ttl.unwrap() > 0);
        assert!(ttl.unwrap() <= 300);

        // 不存在的键
        let ttl = client.get_l1_ttl("nonexistent_key").await.unwrap();
        assert!(ttl.is_none());

        // 清理
        let _ = client.delete(test_key).await;
    }

    #[tokio::test]
    async fn test_get_l1_ttl_not_exists() {
        let client = get_test_two_level_client().await;

        // 不存在的键应该返回 None
        let ttl = client
            .get_l1_ttl("definitely_not_exists_key_12345")
            .await
            .unwrap();
        assert!(ttl.is_none());
    }

    #[tokio::test]
    async fn test_refresh_l1_ttl() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:l1:ttl:refresh";

        // 设置初始值，TTL 为 60 秒
        client.set(test_key, &"value", Some(60)).await.unwrap();

        // 刷新 L1 TTL 到 600 秒
        let result = client.refresh_l1_ttl(test_key, 600).await.unwrap();
        assert!(result);

        // 验证新 TTL
        let ttl = client.get_l1_ttl(test_key).await.unwrap();
        assert!(ttl.is_some());
        assert!(ttl.unwrap() > 500); // 至少还有 500 秒以上

        // 清理
        let _ = client.delete(test_key).await;
    }

    #[tokio::test]
    async fn test_refresh_l1_ttl_not_exists() {
        let client = get_test_two_level_client().await;

        // 刷新不存在的键应该返回 false
        let result = client.refresh_l1_ttl("nonexistent_key", 600).await.unwrap();
        assert!(!result);
    }

    // ============================================================================
    // L2 TTL 测试
    // ============================================================================

    #[tokio::test]
    async fn test_get_l2_ttl_basic() {
        let client = get_test_l2_client().await;
        let test_key = "oxcache:test:ttl:get";

        // 设置带 TTL 的值
        client.set(test_key, &"value", Some(300)).await.unwrap();

        // 获取 TTL
        let ttl = client.get_l2_ttl(test_key).await.unwrap();
        assert!(ttl.is_some());
        assert!(ttl.unwrap() > 0);
        assert!(ttl.unwrap() <= 300);

        // 不存在的键
        let ttl = client.get_l2_ttl("nonexistent_key").await.unwrap();
        assert!(ttl.is_none());

        // 清理
        let _ = client.delete(test_key).await;
    }

    #[tokio::test]
    async fn test_get_l2_ttl_not_exists() {
        let client = get_test_l2_client().await;

        // 不存在的键应该返回 None
        let ttl = client
            .get_l2_ttl("definitely_not_exists_key_12345")
            .await
            .unwrap();
        assert!(ttl.is_none());
    }

    // ============================================================================
    // L2 TTL 刷新测试
    // ============================================================================

    #[tokio::test]
    async fn test_refresh_l2_ttl() {
        let client = get_test_l2_client().await;
        let test_key = "oxcache:test:ttl:refresh";

        // 设置初始值，TTL 为 60 秒
        client.set(test_key, &"value", Some(60)).await.unwrap();

        // 刷新 TTL 到 600 秒
        let result = client.refresh_l2_ttl(test_key, 600).await.unwrap();
        assert!(result);

        // 验证新 TTL
        let ttl = client.get_l2_ttl(test_key).await.unwrap();
        assert!(ttl.is_some());
        assert!(ttl.unwrap() > 500); // 至少还有 500 秒以上

        // 清理
        let _ = client.delete(test_key).await;
    }

    #[tokio::test]
    async fn test_refresh_l2_ttl_not_exists() {
        let client = get_test_l2_client().await;

        // 刷新不存在的键应该返回 false
        let result = client.refresh_l2_ttl("nonexistent_key", 600).await.unwrap();
        assert!(!result);
    }

    // ============================================================================
    // Touch 操作测试
    // ============================================================================

    #[tokio::test]
    async fn test_touch_basic() {
        let client = get_test_l2_client().await;
        let test_key = "oxcache:test:touch:basic";

        // 设置值，TTL 为 120 秒
        client.set(test_key, &"value", Some(120)).await.unwrap();

        // 获取初始 TTL
        let initial_ttl = client.get_l2_ttl(test_key).await.unwrap();
        assert!(initial_ttl.is_some());

        // Touch 操作
        let result = client.touch(test_key).await.unwrap();
        assert!(result);

        // TTL 应该被刷新（接近原始值）
        let new_ttl = client.get_l2_ttl(test_key).await.unwrap();
        assert!(new_ttl.is_some());

        // 清理
        let _ = client.delete(test_key).await;
    }

    #[tokio::test]
    async fn test_touch_not_exists() {
        let client = get_test_l2_client().await;

        // Touch 不存在的键应该返回 false
        let result = client.touch("nonexistent_touch_key").await.unwrap();
        assert!(!result);
    }

    // ============================================================================
    // 通用 TTL 测试
    // ============================================================================

    #[tokio::test]
    async fn test_get_ttl_l2_only() {
        let client = get_test_l2_client().await;
        let test_key = "oxcache:test:ttl:getttl";

        // 设置带 TTL 的值
        client.set(test_key, &"value", Some(180)).await.unwrap();

        // get_ttl 应该返回 L2 TTL
        let ttl = client.get_ttl(test_key).await.unwrap();
        assert!(ttl.is_some());
        assert!(ttl.unwrap() > 0);
        assert!(ttl.unwrap() <= 180);

        // 清理
        let _ = client.delete(test_key).await;
    }

    #[tokio::test]
    async fn test_refresh_ttl() {
        let client = get_test_l2_client().await;
        let test_key = "oxcache:test:ttl:refresttl";

        // 设置初始值
        client.set(test_key, &"value", Some(60)).await.unwrap();

        // 刷新 TTL（同时刷新 L1 和 L2）
        let result = client.refresh_ttl(test_key, 600).await.unwrap();
        assert!(result);

        // 验证 TTL 已更新
        let ttl = client.get_ttl(test_key).await.unwrap();
        assert!(ttl.is_some());
        assert!(ttl.unwrap() > 500);

        // 清理
        let _ = client.delete(test_key).await;
    }

    // ============================================================================
    // TTL 边界测试
    // ============================================================================

    #[tokio::test]
    async fn test_ttl_expiration() {
        let client = get_test_l2_client().await;
        let test_key = "oxcache:test:ttl:expire";

        // 设置很短的 TTL
        client.set(test_key, &"temporary", Some(1)).await.unwrap();

        // 立即查询应该能获取到
        let value: Option<String> = client.get(test_key).await.unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap(), "temporary");

        // 等待过期
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // 过期后应该获取不到
        let value: Option<String> = client.get(test_key).await.unwrap();
        assert!(value.is_none());

        // TTL 应该返回 None
        let ttl = client.get_l2_ttl(test_key).await.unwrap();
        assert!(ttl.is_none());
    }

    // ============================================================================
    // 批量 TTL 操作测试
    // ============================================================================

    #[tokio::test]
    async fn test_multiple_keys_ttl() {
        let client = get_test_l2_client().await;
        let test_prefix = "oxcache:test:ttl:multi";

        // 设置多个带不同 TTL 的键
        for i in 1..=5 {
            let key = format!("{}:{}", test_prefix, i);
            let ttl = i * 60; // 60, 120, 180, 240, 300
            client
                .set(&key, &format!("value{}", i), Some(ttl))
                .await
                .unwrap();
        }

        // 验证所有键的 TTL
        for i in 1..=5 {
            let key = format!("{}:{}", test_prefix, i);
            let ttl = client.get_l2_ttl(&key).await.unwrap();
            assert!(ttl.is_some());
            let expected_min = (i * 60) - 10; // 允许一些误差
            assert!(ttl.unwrap() >= expected_min);

            // 清理
            let _ = client.delete(&key).await;
        }
    }

    // ============================================================================
    // L1 TTL 边界测试
    // ============================================================================

    #[tokio::test]
    async fn test_l1_ttl_expiration() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:l1:ttl:expire";

        // 设置很短的 L1 TTL
        client.set(test_key, &"temporary", Some(1)).await.unwrap();

        // 立即查询应该能获取到
        let value: Option<String> = client.get(test_key).await.unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap(), "temporary");

        // 等待过期
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // 过期后应该获取不到
        let value: Option<String> = client.get(test_key).await.unwrap();
        assert!(value.is_none());

        // L1 TTL 应该返回 None
        let ttl = client.get_l1_ttl(test_key).await.unwrap();
        assert!(ttl.is_none());
    }

    #[tokio::test]
    async fn test_l1_multiple_keys_ttl() {
        let client = get_test_two_level_client().await;
        let test_prefix = "oxcache:test:l1:ttl:multi";

        // 设置多个带不同 TTL 的键
        for i in 1..=5 {
            let key = format!("{}:{}", test_prefix, i);
            let ttl = i * 60; // 60, 120, 180, 240, 300
            client
                .set(&key, &format!("value{}", i), Some(ttl))
                .await
                .unwrap();
        }

        // 验证所有键的 L1 TTL
        for i in 1..=5 {
            let key = format!("{}:{}", test_prefix, i);
            let ttl = client.get_l1_ttl(&key).await.unwrap();
            assert!(ttl.is_some());
            let expected_min = (i * 60) - 10; // 允许一些误差
            assert!(ttl.unwrap() >= expected_min);

            // 清理
            let _ = client.delete(&key).await;
        }
    }

    // ============================================================================
    // L1 和 L2 TTL 一致性测试
    // ============================================================================

    #[tokio::test]
    async fn test_l1_l2_ttl_consistency() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:ttl:consistency";

        // 设置值，TTL 为 300 秒
        client.set(test_key, &"value", Some(300)).await.unwrap();

        // 获取 L1 和 L2 TTL
        let l1_ttl = client.get_l1_ttl(test_key).await.unwrap();
        let l2_ttl = client.get_l2_ttl(test_key).await.unwrap();

        // 两个 TTL 都应该存在
        assert!(l1_ttl.is_some());
        assert!(l2_ttl.is_some());

        // L1 和 L2 TTL 应该相近（允许一些差异）
        let l1_value = l1_ttl.unwrap();
        let l2_value = l2_ttl.unwrap();
        let diff = (l1_value as i64 - l2_value as i64).abs();
        assert!(diff < 10, "L1 and L2 TTL should be close");

        // 清理
        let _ = client.delete(test_key).await;
    }

    #[tokio::test]
    async fn test_refresh_both_ttls() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:ttl:refresh_both";

        // 设置初始值，TTL 为 60 秒
        client.set(test_key, &"value", Some(60)).await.unwrap();

        // 刷新 TTL（同时刷新 L1 和 L2）
        let result = client.refresh_ttl(test_key, 600).await.unwrap();
        assert!(result);

        // 验证 L1 TTL 已更新
        let l1_ttl = client.get_l1_ttl(test_key).await.unwrap();
        assert!(l1_ttl.is_some());
        assert!(l1_ttl.unwrap() > 500);

        // 验证 L2 TTL 已更新
        let l2_ttl = client.get_l2_ttl(test_key).await.unwrap();
        assert!(l2_ttl.is_some());
        assert!(l2_ttl.unwrap() > 500);

        // 清理
        let _ = client.delete(test_key).await;
    }
}
