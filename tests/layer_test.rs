//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 分层缓存测试

use oxcache::config::{CacheType, Config, L1Config, L2Config, RedisMode, ServiceConfig};
use std::collections::HashMap;

mod common;

#[tokio::test]
async fn test_l1_only_mode() {
    common::setup_logging();

    let service_name = common::generate_unique_service_name("l1_only_test");

    let config = Config {
        config_version: Some(1),
        global: Default::default(),
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::L1,
                    ttl: Some(60),
                    serialization: None,
                    two_level: None,
                    l1: Some(L1Config {
                        max_capacity: 100,
                        cleanup_interval_secs: 0,
                        ..Default::default()
                    }),
                    l2: None,
                },
            );
            map
        },
    };

    common::setup_cache(config).await;

    let client = oxcache::get_client(&service_name).expect("Client should be available");

    // Test Set
    client
        .set_bytes("key1", "value1".as_bytes().to_vec(), None)
        .await
        .expect("Set should succeed");

    // Test Get
    let value_bytes = client.get_bytes("key1").await.expect("Get should succeed");
    let value = value_bytes.map(|v| String::from_utf8(v).expect("Should be valid utf8"));
    assert_eq!(value, Some("value1".to_string()));

    // Test Delete
    client.delete("key1").await.expect("Delete should succeed");

    let value_bytes = client.get_bytes("key1").await.expect("Get should succeed");
    assert_eq!(value_bytes, None);
}

#[tokio::test]
async fn test_l2_only_mode() {
    common::setup_logging();

    if !common::is_redis_available().await {
        println!("Skipping L2-only test because Redis is not available");
        return;
    }

    let service_name = common::generate_unique_service_name("l2_only_test");

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let config = Config {
        config_version: Some(1),
        global: Default::default(),
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::L2,
                    ttl: Some(60),
                    serialization: None,
                    two_level: None,
                    l1: None,
                    l2: Some(L2Config {
                        mode: RedisMode::Standalone,
                        connection_string: redis_url.into(),
                        connection_timeout_ms: 2000,
                        command_timeout_ms: 1000,
                        sentinel: None,
                        default_ttl: None,
                        cluster: None,
                        password: None,
                        enable_tls: false,
                        ..Default::default()
                    }),
                },
            );
            map
        },
    };

    common::setup_cache(config).await;

    let client = oxcache::get_client(&service_name).expect("Client should be available");

    // Test Set
    client
        .set_bytes("key2", "value2".as_bytes().to_vec(), None)
        .await
        .expect("Set should succeed");

    // Test Get
    let value_bytes = client.get_bytes("key2").await.expect("Get should succeed");
    let value = value_bytes.map(|v| String::from_utf8(v).expect("Should be valid utf8"));
    assert_eq!(value, Some("value2".to_string()));

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let redis_client = redis::Client::open(redis_url).unwrap();
    let mut conn = match redis_client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            println!("Skipping direct Redis verification (connect error): {}", e);
            return;
        }
    };
    let redis_val: redis::RedisResult<Option<Vec<u8>>> =
        redis::cmd("GET").arg("key2").query_async(&mut conn).await;
    if let Ok(val) = redis_val {
        assert!(val.is_some());
    } else if let Err(e) = redis_val {
        println!("Skipping direct Redis verification (command error): {}", e);
        return;
    }

    // Test Delete
    client.delete("key2").await.expect("Delete should succeed");

    let value_bytes = client.get_bytes("key2").await.expect("Get should succeed");
    assert_eq!(value_bytes, None);

    let redis_val: redis::RedisResult<Option<Vec<u8>>> =
        redis::cmd("GET").arg("key2").query_async(&mut conn).await;
    if let Ok(val) = redis_val {
        assert!(val.is_none());
    } else if let Err(e) = redis_val {
        println!(
            "Skipping direct Redis deletion verification (command error): {}",
            e
        );
    }
}
