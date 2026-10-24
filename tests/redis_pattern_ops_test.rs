// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 真实 Redis 容器上的模式操作验证。
//!
//! `pattern_delete` 的计数口径是「实际被删除的键数」（按 DEL 回执统计），
//! 而不是按 SCAN 批长度盲计。该改动此前只在源码层核对过，没有真实服务端
//! 验证过 —— 本文件补上这一层。
//!
//! 计数语义里「键在 SCAN 与 DEL 之间被并发删除」那一支无法用确定性测试
//! 钉住（需要在两个调用之间注入时序），因此这里钉的是可观测契约：
//! 返回的条数等于真正消失的匹配键数、不匹配键不受影响、重复调用归零。

#![cfg(feature = "redis")]

#[path = "common/mod.rs"]
mod common;

use common::test_containers::{RedisContainer, container_or_skip};
use oxcache::backend::{CacheBackend, CacheConnector, CacheReader, CacheSetItem, CacheWriter};
use std::sync::Arc;

fn item(key: &str) -> CacheSetItem {
    (Arc::from(key), Arc::new(vec![1u8]), None)
}

/// 模式删除在真实 Redis 上按「实际被删除的键数」计数。
#[tokio::test]
async fn redis_pattern_delete_reports_actually_deleted_keys() {
    let Some(container) = container_or_skip("redis", RedisContainer::start).await else {
        return;
    };
    let backend = oxcache::backend::RedisBackend::new(&container.url())
        .await
        .expect("redis backend");

    let items = vec![item("ns:a"), item("ns:b"), item("ns:c"), item("other:x")];
    CacheWriter::set_many(&backend, &items)
        .await
        .expect("set_many");

    let deleted = CacheBackend::delete_by_pattern(&backend, "ns:*")
        .await
        .expect("delete_by_pattern");
    assert_eq!(deleted, 3, "三个匹配键必须被计入删除条数");
    assert_eq!(
        CacheReader::len(&backend).await.expect("len"),
        1,
        "只有不匹配的键应当留下"
    );
    assert!(
        CacheReader::get(&backend, "other:x")
            .await
            .expect("get")
            .is_some(),
        "不匹配的键不得被模式删除波及"
    );

    // 键已不存在时再删一次必须归零（按 DEL 回执计数才不会把空批算成已删）。
    let again = CacheBackend::delete_by_pattern(&backend, "ns:*")
        .await
        .expect("delete_by_pattern again");
    assert_eq!(again, 0, "没有匹配键时删除条数必须为 0");

    CacheConnector::shutdown(&backend).await;
}

/// 模式计数在真实 Redis 上与实际键数一致。
#[tokio::test]
async fn redis_pattern_count_matches_actual_keys() {
    let Some(container) = container_or_skip("redis", RedisContainer::start).await else {
        return;
    };
    let backend = oxcache::backend::RedisBackend::new(&container.url())
        .await
        .expect("redis backend");

    let items = vec![item("ns:1"), item("ns:2"), item("other:1")];
    CacheWriter::set_many(&backend, &items)
        .await
        .expect("set_many");

    let counted = CacheBackend::count_by_pattern(&backend, "ns:*")
        .await
        .expect("count_by_pattern");
    assert_eq!(counted, 2, "只统计匹配模式的键");

    CacheConnector::shutdown(&backend).await;
}
