// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Pipeline batch operation tests for RedisBackend.

use super::*;
use crate::backend::CacheReader;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_many_pipeline_and_get_many_pipeline() {
    let backend = make_backend().await;
    let k1 = unique_key("p1");
    let k2 = unique_key("p2");
    let items: Vec<(&str, Vec<u8>)> = vec![(k1.as_str(), b"pv1".to_vec()), (k2.as_str(), b"pv2".to_vec())];
    backend
        .set_many_pipeline(&items, None)
        .await
        .expect("set_many_pipeline failed");

    let keys: Vec<&str> = vec![k1.as_str(), k2.as_str()];
    let values = backend
        .get_many_pipeline(&keys)
        .await
        .expect("get_many_pipeline failed");
    assert_eq!(values.len(), 2);
    assert_eq!(values[0], Some(b"pv1".to_vec()));
    assert_eq!(values[1], Some(b"pv2".to_vec()));

    backend
        .delete_many_pipeline(&keys)
        .await
        .expect("delete_many_pipeline failed");
    for k in &keys {
        assert!(!backend.exists(k).await.unwrap());
    }
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_many_pipeline_empty_is_ok() {
    let backend = make_backend().await;
    backend
        .set_many_pipeline(&[], None)
        .await
        .expect("set_many_pipeline empty should be ok");
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_get_many_pipeline_empty_returns_empty() {
    let backend = make_backend().await;
    let result = backend
        .get_many_pipeline(&[])
        .await
        .expect("get_many_pipeline empty failed");
    assert!(result.is_empty());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_delete_many_pipeline_empty_is_ok() {
    let backend = make_backend().await;
    backend
        .delete_many_pipeline(&[])
        .await
        .expect("delete_many_pipeline empty should be ok");
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_many_pipeline_with_ttl() {
    let backend = make_backend().await;
    let k1 = unique_key("pttl1");
    let items: Vec<(&str, Vec<u8>)> = vec![(k1.as_str(), b"v".to_vec())];
    backend
        .set_many_pipeline(&items, Some(Duration::from_secs(80)))
        .await
        .expect("set_many_pipeline failed");
    let ttl = backend.ttl(&k1).await.unwrap();
    assert!(ttl.is_some() && ttl.unwrap().as_secs() > 70);
    cleanup(&backend, &k1).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_many_pipeline_with_invalid_key_rejected() {
    let backend = make_backend().await;
    let items: Vec<(&str, Vec<u8>)> = vec![("bad;key", b"v".to_vec())];
    let result = backend.set_many_pipeline(&items, None).await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_get_many_pipeline_with_invalid_key_rejected() {
    let backend = make_backend().await;
    let keys: Vec<&str> = vec!["bad;key"];
    let result = backend.get_many_pipeline(&keys).await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_delete_many_pipeline_with_invalid_key_rejected() {
    let backend = make_backend().await;
    let keys: Vec<&str> = vec!["bad;key"];
    let result = backend.delete_many_pipeline(&keys).await;
    assert!(result.is_err());
}
