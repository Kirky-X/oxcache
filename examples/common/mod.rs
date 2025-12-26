//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! Common utilities for examples

use oxcache::config::{
    CacheType, Config, L1Config, L2Config, RedisMode, ServiceConfig, TwoLevelConfig,
};
use std::collections::HashMap;

/// Create a default two-level cache configuration for examples
pub fn create_default_config(service_name: &str, max_capacity: usize) -> Config {
    let mut services = HashMap::new();
    services.insert(
        service_name.to_string(),
        ServiceConfig {
            cache_type: CacheType::TwoLevel,
            ttl: Some(300),
            serialization: None,
            l1: Some(L1Config {
                max_capacity: max_capacity as u64,
            }),
            l2: Some(L2Config {
                mode: RedisMode::Standalone,
                connection_string: "redis://127.0.0.1:6379".to_string().into(),
                ..Default::default()
            }),
            two_level: Some(TwoLevelConfig {
                promote_on_hit: true,
                enable_batch_write: true,
                batch_size: 10,
                batch_interval_ms: 100,
                invalidation_channel: None,
                bloom_filter: None,
                warmup: None,
            }),
        },
    );

    Config {
        services,
        ..Default::default()
    }
}
