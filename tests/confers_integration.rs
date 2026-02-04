// Copyright (c) 2025-2026, Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! confers DI集成测试
//!
//! 测试oxcache的with_confers()依赖注入功能

#[cfg(feature = "confers")]
mod tests {
    use oxcache::Cache;
    use serde_json::json;

    /// 创建测试用的confers配置（JSON格式）
    fn create_test_config(pairs: Vec<(&str, serde_json::Value)>) -> serde_json::Value {
        let mut config_map = serde_json::Map::new();
        for (key, value) in pairs {
            let parts: Vec<&str> = key.split('.').collect();
            if parts.len() == 1 {
                config_map.insert(key.to_string(), value);
            } else {
                // 处理嵌套键
                let mut current = config_map
                    .entry(parts[0].to_string())
                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
                    .as_object_mut()
                    .unwrap();
                for (i, part) in parts.iter().enumerate() {
                    if i == parts.len() - 1 {
                        current.insert(part.to_string(), value.clone());
                    } else {
                        current = current
                            .entry(part.to_string())
                            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
                            .as_object_mut()
                            .unwrap();
                    }
                }
            }
        }
        json!({ "oxcache": config_map })
    }

    #[tokio::test]
    async fn test_cache_with_confers_memory_backend() {
        let config = create_test_config(vec![
            ("backend", json!("memory")),
            ("capacity", json!(5000)),
        ]);

        let cache: Cache<String, String> = Cache::with_confers(&config)
            .await
            .expect("Failed to create cache with confers");

        // 验证缓存功能正常
        let key1 = "key1".to_string();
        let value1 = "value1".to_string();
        cache
            .set(&key1, &value1)
            .await
            .expect("Failed to set value");

        let value = cache.get(&key1).await.expect("Failed to get value");

        assert_eq!(value, Some(value1));
    }

    #[tokio::test]
    async fn test_cache_builder_with_confers() {
        let config = create_test_config(vec![
            ("backend", json!("memory")),
            ("ttl", json!(3600)),
            ("capacity", json!(1000)),
        ]);

        let cache: Cache<String, String> = Cache::builder()
            .with_confers(&config)
            .build()
            .await
            .expect("Failed to build cache with confers");

        // 验证功能正常
        let test_key = "test_key".to_string();
        let test_value = "test_value".to_string();
        cache
            .set(&test_key, &test_value)
            .await
            .expect("Failed to set value");

        let value = cache.get(&test_key).await.expect("Failed to get value");

        assert_eq!(value, Some(test_value));
    }

    #[tokio::test]
    async fn test_cache_builder_manual_override_confers() {
        let config = create_test_config(vec![
            ("backend", json!("memory")),
            ("ttl", json!(3600)), // confers中的TTL
        ]);

        // 手动设置的TTL应该覆盖confers中的TTL
        let _cache: Cache<String, String> = Cache::builder()
            .with_confers(&config)
            .ttl(std::time::Duration::from_secs(7200)) // 覆盖confers的3600
            .build()
            .await
            .expect("Failed to build cache");

        // 验证缓存创建成功（TTL验证需要在实际使用中测试）
    }

    #[tokio::test]
    async fn test_cache_with_confers_default_memory() {
        // 不指定backend，应该默认使用memory
        let config = create_test_config(vec![("capacity", json!(2000))]);

        let cache: Cache<String, String> = Cache::with_confers(&config)
            .await
            .expect("Failed to create cache");

        let default_key = "default_key".to_string();
        let default_value = "default_value".to_string();
        cache
            .set(&default_key, &default_value)
            .await
            .expect("Failed to set value");

        let value = cache.get(&default_key).await.expect("Failed to get value");

        assert_eq!(value, Some(default_value));
    }

    #[tokio::test]
    async fn test_backend_builder_with_confers_memory() {
        let config = create_test_config(vec![
            ("backend", json!("memory")),
            ("capacity", json!(3000)),
            ("ttl", json!(1800)),
        ]);

        let backend_builder = oxcache::builder::BackendBuilder::with_confers(&config);
        let backend = backend_builder
            .build()
            .await
            .expect("Failed to build backend");

        // 验证health check
        let healthy = backend
            .health_check()
            .await
            .expect("Failed to check health");
        assert!(healthy);
    }

    #[tokio::test]
    async fn test_backend_builder_with_confers_tiered() {
        #[cfg(feature = "redis")]
        {
            let config = create_test_config(vec![
                ("backend", json!("tiered")),
                ("tiered.l1_capacity", json!(5000)),
                // Redis URL是必需的，但测试环境可能没有Redis
                // ("redis.url", json!("redis://localhost:6379")),
            ]);

            let backend_builder = oxcache::builder::BackendBuilder::with_confers(&config);

            // 由于没有Redis URL，build()会失败，这是预期的
            let result = backend_builder.build().await;
            assert!(result.is_err(), "Expected error without Redis URL");
        }

        #[cfg(not(feature = "redis"))]
        {
            // 当redis feature未启用时，tiered应该回退到memory
            let config = create_test_config(vec![("backend", json!("tiered"))]);

            let backend_builder = oxcache::builder::BackendBuilder::with_confers(&config);
            let backend = backend_builder
                .build()
                .await
                .expect("Fallback to memory should work");

            let healthy = backend
                .health_check()
                .await
                .expect("Failed to check health");
            assert!(healthy);
        }
    }

    #[tokio::test]
    async fn test_confers_config_priority() {
        // 测试：手动配置优先级高于confers配置
        let config = create_test_config(vec![
            ("backend", json!("memory")),
            ("ttl", json!(1000)),      // confers中是1000秒
            ("capacity", json!(1000)), // confers中是1000
        ]);

        // builder模式：手动设置的capacity应该覆盖confers
        let cache: Cache<String, String> = Cache::builder()
            .with_confers(&config)
            .capacity(5000) // 手动设置为5000，覆盖confers的1000
            .build()
            .await
            .expect("Failed to build cache");

        // 验证缓存创建成功（capacity验证需要在内部API中测试）
        let _ = cache;
    }
}
