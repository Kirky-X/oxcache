//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache builder tests

use oxcache::backend::MokaMemoryBackend as MemoryBackend;
use oxcache::cache::builder::CacheBuilder;
use oxcache::cache::Cache;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TestValue {
    id: u64,
    name: String,
}

#[tokio::test]
async fn test_cache_builder_default() {
    let cache: Cache<String, TestValue> = CacheBuilder::default().build().await.unwrap();
    cache.health_check().await.unwrap();
}

#[tokio::test]
async fn test_cache_builder_with_capacity() {
    let cache: Cache<String, TestValue> = CacheBuilder::default().capacity(1000).build().await.unwrap();
    cache.health_check().await.unwrap();
}

#[tokio::test]
async fn test_cache_builder_with_ttl() {
    let cache: Cache<String, TestValue> = CacheBuilder::default()
        .ttl(Duration::from_secs(3600))
        .build()
        .await
        .unwrap();
    cache.health_check().await.unwrap();
}

#[tokio::test]
async fn test_cache_builder_with_backend() {
    let backend = MemoryBackend::builder().capacity(5000).build();
    let cache: Cache<String, TestValue> = CacheBuilder::default()
        .backend_arc(Arc::new(backend))
        .build()
        .await
        .unwrap();
    cache.health_check().await.unwrap();
}
