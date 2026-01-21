#![allow(deprecated)]
//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 分层缓存细粒度控制集成测试
//!
//! 测试 L1/L2 直接操作和跨层移动功能。

#[cfg(feature = "l2-redis")]
mod tests {
    use oxcache::backend::l1::L1Backend;
    use oxcache::backend::l2::L2Backend;
    use oxcache::client::tiered_cache::TieredCacheControl;
    use oxcache::client::two_level::TwoLevelClient;
    use oxcache::config::{L2Config, RedisMode, TwoLevelConfig};
    use oxcache::serialization::{JsonSerializer, SerializerEnum};
    use secrecy::SecretString;
    use std::sync::Arc;

    /// 获取测试 TwoLevelClient
    async fn get_test_two_level_client() -> Arc<TwoLevelClient> {
        let l2_config = L2Config {
            connection_string: SecretString::new("redis://127.0.0.1:6379".to_string()),
            mode: RedisMode::Standalone,
            default_ttl: Some(300),
            ..Default::default()
        };

        let l2_backend = Arc::new(
            L2Backend::new(&l2_config)
                .await
                .expect("Failed to create L2 backend"),
        );

        let l1_backend = Arc::new(L1Backend::new(1000));

        let two_level_config = TwoLevelConfig {
            promote_on_hit: false, // 禁用自动提升以便测试手动操作
            enable_batch_write: false,
            ..Default::default()
        };

        Arc::new(
            TwoLevelClient::new(
                "test_tiered_service".to_string(),
                two_level_config,
                l1_backend,
                l2_backend,
                SerializerEnum::Json(JsonSerializer::new()),
            )
            .await
            .expect("Failed to create TwoLevel client"),
        )
    }

    /// 清理测试数据
    async fn cleanup_test_keys(client: &TwoLevelClient, pattern: &str) {
        let _ = client.del_pattern(pattern).await;
    }

    // ============================================================================
    // L1 直接操作测试
    // ============================================================================

    #[tokio::test]
    async fn test_get_l1_direct() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:tiered:l1direct";

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;

        // 直接设置 L1
        client
            .set_l1_direct(test_key, b"l1_value".to_vec(), Some(300))
            .await
            .unwrap();

        // 直接获取 L1（应该能获取到）
        let value = client.get_l1_direct(test_key).await.unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap(), b"l1_value");

        // 通过常规 get 应该也能获取（因为数据在 L1）
        // 注意：由于设置了原始字节，需要使用 get_l1_direct 获取
        let value = client.get_l1_direct(test_key).await.unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap(), b"l1_value");

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;
    }

    #[tokio::test]
    async fn test_get_l1_direct_not_exists() {
        let client = get_test_two_level_client().await;

        // L1 中不存在应该返回 None
        let value = client.get_l1_direct("nonexistent_l1_key").await.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_set_l1_direct() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:tiered:setl1";

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;

        // 直接设置 L1
        client
            .set_l1_direct(test_key, b"only_in_l1".to_vec(), Some(300))
            .await
            .unwrap();

        // 验证 L1 有数据
        let l1_value = client.get_l1_direct(test_key).await.unwrap();
        assert!(l1_value.is_some());

        // 验证 L2 没有数据（因为是直接设置 L1）
        let l2_value = client.get_l2_direct(test_key).await.unwrap();
        assert!(l2_value.is_none());

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;
    }

    #[tokio::test]
    async fn test_delete_l1_direct() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:tiered:deletel1";

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;

        // 先设置数据到 L1
        client
            .set_l1_direct(test_key, b"to_delete".to_vec(), Some(300))
            .await
            .unwrap();

        // 验证存在
        assert!(client.get_l1_direct(test_key).await.unwrap().is_some());

        // 直接删除 L1
        let result = client.delete_l1_direct(test_key).await.unwrap();
        assert!(result);

        // 验证已删除
        assert!(client.get_l1_direct(test_key).await.unwrap().is_none());

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;
    }

    // ============================================================================
    // L2 直接操作测试
    // ============================================================================

    #[tokio::test]
    async fn test_get_l2_direct() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:tiered:l2direct";

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;

        // 直接设置 L2
        client
            .set_l2_direct(test_key, b"l2_value".to_vec(), Some(300))
            .await
            .unwrap();

        // 直接获取 L2
        let value = client.get_l2_direct(test_key).await.unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap(), b"l2_value".to_vec());

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;
    }

    #[tokio::test]
    async fn test_set_l2_direct() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:tiered:setl2";

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;

        // 直接设置 L2
        client
            .set_l2_direct(test_key, b"only_in_l2".to_vec(), Some(300))
            .await
            .unwrap();

        // 验证 L2 有数据
        let l2_value = client.get_l2_direct(test_key).await.unwrap();
        assert!(l2_value.is_some());
        assert_eq!(l2_value.unwrap(), b"only_in_l2");

        // 验证 L1 没有数据（因为是直接设置 L2）
        let l1_value = client.get_l1_direct(test_key).await.unwrap();
        assert!(l1_value.is_none());

        // 常规 get_bytes 应该能获取到（因为数据在 L2，原始字节）
        let value = client.get_l2_direct(test_key).await.unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap(), b"only_in_l2");

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;
    }

    #[tokio::test]
    async fn test_delete_l2_direct() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:tiered:deletel2";

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;

        // 先设置数据到 L2
        client
            .set_l2_direct(test_key, b"to_delete_from_l2".to_vec(), Some(300))
            .await
            .unwrap();

        // 直接删除 L2
        let result = client.delete_l2_direct(test_key).await.unwrap();
        assert!(result);

        // 验证已删除
        assert!(client.get_l2_direct(test_key).await.unwrap().is_none());

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;
    }

    // ============================================================================
    // 跨层移动测试
    // ============================================================================

    #[tokio::test]
    async fn test_promote_to_l1() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:tiered:promote";

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;

        // 先设置数据到 L2
        client
            .set_l2_direct(test_key, b"promote_this".to_vec(), Some(300))
            .await
            .unwrap();

        // 验证 L1 没有数据
        assert!(client.get_l1_direct(test_key).await.unwrap().is_none());

        // 提升到 L1
        let result = client.promote_to_l1(test_key).await.unwrap();
        assert!(result);

        // 验证 L1 有数据
        let l1_value = client.get_l1_direct(test_key).await.unwrap();
        assert!(l1_value.is_some());
        assert_eq!(l1_value.unwrap(), b"promote_this".to_vec());

        // L2 仍然有数据
        let l2_value = client.get_l2_direct(test_key).await.unwrap();
        assert!(l2_value.is_some());

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;
    }

    #[tokio::test]
    async fn test_promote_to_l1_not_exists() {
        let client = get_test_two_level_client().await;

        // 提升不存在的键应该返回 false
        let result = client
            .promote_to_l1("nonexistent_promote_key")
            .await
            .unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_demote_to_l2() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:tiered:demote";

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;

        // 先设置数据到 L1
        client
            .set_l1_direct(test_key, b"demote_this".to_vec(), Some(300))
            .await
            .unwrap();

        // 降级到 L2（同时可以指定新的 TTL）
        let result = client.demote_to_l2(test_key, Some(600)).await.unwrap();
        assert!(result);

        // L1 应该还有数据（promote_on_hit 已禁用，所以不会自动提升回来）
        let l1_value = client.get_l1_direct(test_key).await.unwrap();
        assert!(l1_value.is_some());

        // L2 也有数据
        let l2_value = client.get_l2_direct(test_key).await.unwrap();
        assert!(l2_value.is_some());
        assert_eq!(l2_value.unwrap(), b"demote_this".to_vec());

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;
    }

    // ============================================================================
    // 完整清除测试
    // ============================================================================

    #[tokio::test]
    async fn test_evict_all() {
        let client = get_test_two_level_client().await;
        let test_key = "oxcache:test:tiered:evict";

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;

        // 在 L1 和 L2 都设置数据
        client
            .set_l1_direct(test_key, b"l1_data".to_vec(), Some(300))
            .await
            .unwrap();
        client
            .set_l2_direct(test_key, b"l2_data".to_vec(), Some(300))
            .await
            .unwrap();

        // 验证两边都有数据
        assert!(client.get_l1_direct(test_key).await.unwrap().is_some());
        assert!(client.get_l2_direct(test_key).await.unwrap().is_some());

        // 完整清除
        let result = client.evict_all(test_key).await.unwrap();
        assert!(result);

        // 验证两边数据都被清除
        assert!(client.get_l1_direct(test_key).await.unwrap().is_none());
        assert!(client.get_l2_direct(test_key).await.unwrap().is_none());

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;
    }

    // ============================================================================
    // 层隔离测试
    // ============================================================================

    #[tokio::test]
    async fn test_layer_isolation() {
        let client = get_test_two_level_client().await;
        let l1_key = "oxcache:test:tiered:isolation:l1";
        let l2_key = "oxcache:test:tiered:isolation:l2";
        let shared_key = "oxcache:test:tiered:isolation:shared";

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;

        // 等待清理完成
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // 设置不同的值到不同层
        client
            .set_l1_direct(l1_key, b"l1_only".to_vec(), Some(300))
            .await
            .unwrap();
        client
            .set_l2_direct(l2_key, b"l2_only".to_vec(), Some(300))
            .await
            .unwrap();

        // 对于共享键，两边都设置
        client
            .set_l1_direct(shared_key, b"shared_in_l1".to_vec(), Some(300))
            .await
            .unwrap();
        client
            .set_l2_direct(shared_key, b"shared_in_l2".to_vec(), Some(300))
            .await
            .unwrap();

        // 验证层隔离
        // L1 键只能从 L1 获取
        assert!(client.get_l1_direct(l1_key).await.unwrap().is_some());
        assert!(client.get_l2_direct(l1_key).await.unwrap().is_none());

        // L2 键只能从 L2 获取
        assert!(client.get_l1_direct(l2_key).await.unwrap().is_none());
        let l2_result = client.get_l2_direct(l2_key).await.unwrap();
        assert!(l2_result.is_some(), "L2 key should exist in L2");

        // 共享键两边都能获取
        assert_eq!(
            client.get_l1_direct(shared_key).await.unwrap().unwrap(),
            b"shared_in_l1".to_vec()
        );
        assert_eq!(
            client.get_l2_direct(shared_key).await.unwrap().unwrap(),
            b"shared_in_l2".to_vec()
        );

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;
    }

    // ============================================================================
    // 批量操作测试
    // ============================================================================

    #[tokio::test]
    async fn test_multiple_direct_operations() {
        let client = get_test_two_level_client().await;
        let test_prefix = "oxcache:test:tiered:multi";

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;

        // 并发设置多个键
        let mut handles = Vec::new();
        for i in 1..=10 {
            let client = client.clone();
            let key = format!("{}:{}", test_prefix, i);
            handles.push(tokio::spawn(async move {
                client
                    .set_l1_direct(&key, format!("value{}", i).into_bytes(), Some(300))
                    .await
            }));
        }

        for handle in handles {
            let _ = handle.await.unwrap();
        }

        // 验证所有键都在 L1
        for i in 1..=10 {
            let key = format!("{}:{}", test_prefix, i);
            let value = client.get_l1_direct(&key).await.unwrap();
            assert!(value.is_some());
        }

        cleanup_test_keys(&client, "oxcache:test:tiered:*").await;
    }
}
