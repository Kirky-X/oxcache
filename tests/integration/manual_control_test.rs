//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 手动控制集成测试

use oxcache::{
    backend::{l1::L1Backend, l2::L2Backend},
    client::two_level::TwoLevelClient,
    config::{L2Config, TwoLevelConfig},
    serialization::SerializerEnum,
};
use std::sync::Arc;

#[path = "../common/mod.rs"]
mod common;

use common::setup_logging;

#[tokio::test]
async fn test_manual_control_api() {
    setup_logging();

    if !common::is_redis_available().await {
        println!("Skipping test_manual_control_api because Redis is not available");
        return;
    }

    let service_name = common::generate_unique_service_name("manual_control");
    let service_name_for_cleanup = service_name.clone();

    let l1 = Arc::new(L1Backend::new(100));
    let l2_config = L2Config {
        connection_string: std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
            .into(),
        ..Default::default()
    };
    let l2 = Arc::new(
        L2Backend::new(&l2_config)
            .await
            .expect("Failed to create L2 backend"),
    );

    // Ensure L2 is clean
    // Need to use service name prefix because L2Backend adds it
    // Manually delete using Redis client to ensure it's gone
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let redis_client = redis::Client::open(redis_url).unwrap();
    let mut conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    let _: () = redis::cmd("DEL")
        .arg(format!("{}:manual_key", service_name))
        .query_async(&mut conn)
        .await
        .unwrap();

    let config = TwoLevelConfig::default();

    let client = TwoLevelClient::new(
        service_name,
        config,
        l1.clone(),
        l2.clone(),
        SerializerEnum::Json(oxcache::serialization::json::JsonSerializer::new()),
    )
    .await
    .expect("Failed to create client");

    // 1. Test set_l1_only
    client
        .set_l1_only("manual_key", &"l1_value".to_string(), None)
        .await
        .expect("Failed to set L1 only");

    // Verify L1 has it
    let l1_val: Option<String> = client
        .get_l1_only("manual_key")
        .await
        .expect("Failed to get L1 only");
    assert_eq!(l1_val, Some("l1_value".to_string()));

    // Verify L2 does not have it
    let l2_val: Option<String> = client
        .get_l2_only("manual_key")
        .await
        .expect("Failed to get L2 only");
    assert!(l2_val.is_none(), "L2 should not have the value");

    // 2. Test set_l2_only
    client
        .set_l2_only("manual_key", &"l2_value".to_string(), None)
        .await
        .expect("Failed to set L2 only");

    // Verify L2 has it
    let l2_val: Option<String> = client
        .get_l2_only("manual_key")
        .await
        .expect("Failed to get L2 only");
    assert_eq!(l2_val, Some("l2_value".to_string()));

    // Verify L1 still has old value (manual control bypasses invalidation if not handled,
    // but here we just want to ensure set_l2_only didn't overwrite L1 implicitly)
    let l1_val: Option<String> = client
        .get_l1_only("manual_key")
        .await
        .expect("Failed to get L1 only");
    assert_eq!(l1_val, Some("l1_value".to_string()));

    // Cleanup
    let _ = l2.delete("manual_key").await;
    common::cleanup_service(&service_name_for_cleanup).await;
}
