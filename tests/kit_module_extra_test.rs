// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! trait-kit `OxcacheModule` 构建面覆盖：config 缺失显性报错、Redis/Chain
//! 配置分支、chain link 三分支（Memory / Redis 缺配置 / 嵌套 Chain 拒绝）、
//! `apply_redis_config` 全 setter、lifecycle 关停钩子。健康/构建 happy path
//! 由内联测试覆盖，此处补外部可驱动的分支。

#![cfg(feature = "kit")]

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::CacheBackend;
use oxcache::integrations::kit::module::{
    BackendType, ChainLinkConfig, OxcacheConfig, OxcacheModule, RedisConfig,
};
use trait_kit::core::lifecycle::AsyncLifecycle;
use trait_kit::prelude::*;

fn memory_link_config() -> ChainLinkConfig {
    ChainLinkConfig {
        backend: BackendType::Memory,
        score: 100,
        redis: None,
    }
}

#[tokio::test]
async fn build_without_config_fails_explicitly() {
    let mut kit = AsyncKit::new();
    kit.register::<OxcacheModule>().expect("register");
    // 未 set_config：AsyncKit::build 阶段模块构建显性失败
    let result = kit.build().await;
    assert!(result.is_err(), "缺 config 的模块构建必须显性失败");
}

#[tokio::test]
async fn build_memory_backend_with_ttl_and_tti() {
    let mut kit = AsyncKit::new();
    kit.set_config(OxcacheConfig {
        capacity: 128,
        ttl: Some(Duration::from_secs(30)),
        tti: Some(Duration::from_secs(15)),
        ..OxcacheConfig::default()
    });
    kit.register::<OxcacheModule>().expect("register");
    let kit = kit.build().await.expect("build");
    let cache: Arc<dyn CacheBackend + Send + Sync> = kit.require::<OxcacheModule>().expect("cap");
    cache
        .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
        .await
        .expect("set");
    assert_eq!(cache.get("k").await.expect("get"), Some(b"v".to_vec()));
    // lifecycle：直接驱动关停钩子
    <OxcacheModule as AsyncLifecycle>::on_shutdown(&cache).await;
}

#[tokio::test]
async fn build_redis_without_redis_config_fails() {
    let mut kit = AsyncKit::new();
    kit.set_config(OxcacheConfig {
        backend: BackendType::Redis,
        ..OxcacheConfig::default()
    });
    kit.register::<OxcacheModule>().expect("register");
    let result = kit.build().await;
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("backend=Redis 缺 redis config 必须显性失败"),
    };
    assert!(err.to_string().contains("redis"));
}

#[tokio::test]
async fn build_redis_with_unreachable_target_applies_full_config() {
    let mut kit = AsyncKit::new();
    kit.set_config(OxcacheConfig {
        backend: BackendType::Redis,
        redis: Some(RedisConfig {
            connection_string: "redis://127.0.0.1:1".to_string(),
            pool_size: 2,
            connection_timeout: Duration::from_millis(300),
            retry_count: 0,
            retry_delay: Duration::from_millis(1),
            circuit_breaker_threshold: 2,
            circuit_breaker_reset_timeout: Duration::from_millis(500),
        }),
        ..OxcacheConfig::default()
    });
    kit.register::<OxcacheModule>().expect("register");
    // apply_redis_config 全 setter 生效后连接不可达 → Connection 错误透传
    let result = kit.build().await;
    assert!(result.is_err(), "不可达 Redis 必须构建失败");
}

#[tokio::test]
async fn build_chain_default_mode_requires_redis_config() {
    let mut kit = AsyncKit::new();
    kit.set_config(OxcacheConfig {
        backend: BackendType::Chain,
        ..OxcacheConfig::default()
    });
    kit.register::<OxcacheModule>().expect("register");
    let result = kit.build().await;
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("backend=Chain 默认模式缺 redis config 必须显性失败"),
    };
    assert!(err.to_string().contains("redis"));
}

#[tokio::test]
async fn build_chain_with_memory_only_link() {
    let mut kit = AsyncKit::new();
    kit.set_config(OxcacheConfig {
        backend: BackendType::Chain,
        capacity: 64,
        ttl: Some(Duration::from_secs(10)),
        tti: Some(Duration::from_secs(5)),
        chain: vec![memory_link_config()],
        redis: None,
    });
    kit.register::<OxcacheModule>().expect("register");
    let kit = kit.build().await.expect("build");
    let cache: Arc<dyn CacheBackend + Send + Sync> = kit.require::<OxcacheModule>().expect("cap");
    cache
        .set(Arc::from("ck"), Arc::new(b"cv".to_vec()), None)
        .await
        .expect("set");
    assert_eq!(cache.get("ck").await.expect("get"), Some(b"cv".to_vec()));
}

#[tokio::test]
async fn chain_link_redis_without_config_fails() {
    let mut kit = AsyncKit::new();
    kit.set_config(OxcacheConfig {
        backend: BackendType::Chain,
        capacity: 64,
        chain: vec![ChainLinkConfig {
            backend: BackendType::Redis,
            score: 50,
            redis: None,
        }],
        redis: None,
        ..OxcacheConfig::default()
    });
    kit.register::<OxcacheModule>().expect("register");
    let result = kit.build().await;
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("chain link backend=Redis 缺 redis config 必须显性失败"),
    };
    assert!(err.to_string().contains("redis"));
}

#[tokio::test]
async fn chain_link_nested_chain_rejected() {
    let mut kit = AsyncKit::new();
    kit.set_config(OxcacheConfig {
        backend: BackendType::Chain,
        capacity: 64,
        chain: vec![ChainLinkConfig {
            backend: BackendType::Chain,
            score: 50,
            redis: None,
        }],
        redis: None,
        ..OxcacheConfig::default()
    });
    kit.register::<OxcacheModule>().expect("register");
    let result = kit.build().await;
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("chain 内嵌套 chain 必须显性失败"),
    };
    assert!(err.to_string().contains("nested"));
}
