// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! CacheConfig 构建路径长尾覆盖：
//!
//! - Disk 后端完整构建（open-or-create + 默认 TTL）
//! - Moka/DashMap 槽位 TTL/TTI 注入与容量传递
//! - metrics_enabled=false 注入 NoOp、serialization_format 透传
//! - backend_kind 枚举 → 原始串映射与校验拒绝分支

#![cfg(feature = "full")]

use std::time::Duration;

use oxcache::config::CacheConfig;

#[tokio::test]
async fn disk_backend_builds_with_default_ttl() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cfg-disk.redb");

    {
        let config = CacheConfig::builder()
            .backend("disk")
            .disk_path(path.to_str().unwrap())
            .ttl(Duration::from_secs(120))
            .build();
        let backend = config.build_backend().await.expect("build disk backend");
        assert!(backend.is_some());
        // redb 文件锁：显式释放后才能二次打开
        drop(backend);
    }

    // 同路径二次构建：open 分支（文件已存在）
    let reopen = CacheConfig::builder()
        .backend("disk")
        .disk_path(path.to_str().unwrap())
        .build();
    let backend2 = reopen.build_backend().await.expect("reopen disk backend");
    assert!(backend2.is_some());
}

#[tokio::test]
async fn disk_backend_requires_path_and_reports_open_create_failure() {
    let config = CacheConfig::builder().backend("disk").build();
    let err = match config.build_backend().await {
        Err(e) => e,
        Ok(_) => panic!("missing disk_path must fail"),
    };
    assert!(!err.to_string().is_empty());

    // 不可创建的路径（目录不存在）→ open+create 双失败复合错误
    let config = CacheConfig::builder()
        .backend("disk")
        .disk_path("/nonexistent-dir-ox/cache.redb")
        .build();
    let err = match config.build_backend().await {
        Err(e) => e,
        Ok(_) => panic!("uncreatable path must fail"),
    };
    assert!(
        err.to_string().contains("open failed") || err.to_string().contains("create failed"),
        "got: {err}"
    );
}

#[tokio::test]
async fn moka_slot_applies_ttl_tti_and_capacity() {
    let config = CacheConfig::builder()
        .backend("moka")
        .capacity(1234)
        .ttl(Duration::from_secs(30))
        .tti(Duration::from_secs(10))
        .build();
    let backend = config.build_backend().await.expect("build moka");
    assert!(backend.is_some());
}

#[tokio::test]
async fn dashmap_slot_applies_capacity() {
    let config = CacheConfig::builder()
        .backend("dashmap")
        .capacity(2048)
        .build();
    let backend = config.build_backend().await.expect("build dashmap");
    assert!(backend.is_some());
}

#[tokio::test]
async fn metrics_disabled_and_serialization_format_flow_through() {
    let config = CacheConfig::builder()
        .backend("moka")
        .metrics_enabled(false)
        .serialization_format("json")
        .build();
    let backend = config
        .build_backend()
        .await
        .expect("build with noop metrics");
    assert!(backend.is_some());
}

#[test]
fn validate_rejects_unknown_and_unbuildable_backends() {
    // Chain / Unknown 不能经单后端配置构建
    let err = CacheConfig::builder()
        .backend("chain")
        .build()
        .validate()
        .unwrap_err();
    assert!(err.to_string().contains("chain"), "got: {err}");

    let err = CacheConfig::builder()
        .backend("nope")
        .build()
        .validate()
        .unwrap_err();
    assert!(
        !err.to_string().is_empty(),
        "unknown backend must be rejected"
    );
}

#[test]
fn backend_kind_setter_maps_all_enum_variants() {
    let cfg = CacheConfig::builder()
        .backend_kind(oxcache::backend::BackendKind::Moka)
        .build();
    assert_eq!(cfg.backend.as_deref(), Some("moka"));
    let cfg = CacheConfig::builder()
        .backend_kind(oxcache::backend::BackendKind::DashMap)
        .build();
    assert_eq!(cfg.backend.as_deref(), Some("dashmap"));
}

#[test]
fn validate_rejects_metrics_without_feature_is_full_agnostic() {
    // full 组合下 metrics 恒可用：正常配置必须通过
    CacheConfig::builder()
        .metrics_enabled(true)
        .build()
        .validate()
        .unwrap();
}

#[test]
fn validate_still_rejects_zero_capacity_and_bad_ttl() {
    let err = CacheConfig::builder()
        .capacity(0)
        .build()
        .validate()
        .unwrap_err();
    assert!(err.to_string().contains("capacity"), "got: {err}");

    let err = CacheConfig::builder()
        .ttl(Duration::ZERO)
        .build()
        .validate()
        .unwrap_err();
    assert!(err.to_string().contains("ttl"), "got: {err}");

    let err = CacheConfig::builder()
        .tti(Duration::ZERO)
        .build()
        .validate()
        .unwrap_err();
    assert!(err.to_string().contains("tti"), "got: {err}");
}

#[test]
fn env_capacity_boundary_is_parsed_and_validated() {
    // env 通路只解析不校验：capacity=0 解析成功，validate 显性拒绝
    unsafe {
        std::env::set_var("OXCACHE_CAPACITY", "0");
    }
    let cfg = CacheConfig::try_from_env().expect("env parse succeeds");
    unsafe {
        std::env::remove_var("OXCACHE_CAPACITY");
    }
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("capacity"), "got: {err}");
}

// ============================================================================
// validate 拒绝分支与枚举映射补全
// ============================================================================

#[test]
fn validate_rejects_zero_threshold_pool_and_empty_service() {
    let err = CacheConfig::builder()
        .circuit_breaker_failure_threshold(0)
        .build()
        .validate()
        .unwrap_err();
    assert!(
        err.to_string().contains("threshold") || err.to_string().contains("circuit"),
        "got: {err}"
    );

    let err = CacheConfig::builder()
        .connection_pool_size(0)
        .build()
        .validate()
        .unwrap_err();
    assert!(err.to_string().contains("pool"), "got: {err}");

    let err = CacheConfig::builder()
        .service_name("")
        .build()
        .validate()
        .unwrap_err();
    assert!(err.to_string().contains("service"), "got: {err}");
}

#[test]
fn validate_rejects_backends_that_need_programmatic_assembly() {
    // aerospike 需要 namespace/set 程序化配置
    let err = CacheConfig::builder()
        .backend("aerospike")
        .build()
        .validate()
        .unwrap_err();
    assert!(err.to_string().contains("aerospike"), "got: {err}");

    // valkey 无后端实现
    let err = CacheConfig::builder()
        .backend("valkey")
        .build()
        .validate()
        .unwrap_err();
    assert!(err.to_string().contains("valkey"), "got: {err}");
}

#[test]
fn backend_kind_setter_covers_all_variants() {
    use oxcache::backend::BackendKind;
    let cases = [
        (BackendKind::Moka, "moka"),
        (BackendKind::DashMap, "dashmap"),
        (BackendKind::Redis, "redis"),
        (BackendKind::Valkey, "valkey"),
        (BackendKind::Dragonfly, "dragonfly"),
        (BackendKind::Aerospike, "aerospike"),
        (BackendKind::Chain, "chain"),
        (BackendKind::Mock, "mock"),
        (BackendKind::Disk, "disk"),
        (BackendKind::Unknown, "unknown"),
    ];
    for (kind, raw) in cases {
        let cfg = CacheConfig::builder().backend_kind(kind).build();
        assert_eq!(cfg.backend.as_deref(), Some(raw), "kind {kind:?}");
    }
}

#[tokio::test]
async fn tti_and_null_cache_ttl_flow_into_builder() {
    let config = CacheConfig::builder()
        .backend("moka")
        .ttl(Duration::from_secs(60))
        .tti(Duration::from_secs(30))
        .null_cache_ttl(Duration::from_secs(5))
        .build();
    let backend = config.build_backend().await.expect("build moka with tti");
    assert!(backend.is_some());
}

#[tokio::test]
async fn sync_mode_with_moka_builds_dual_face() {
    let config = CacheConfig::builder()
        .backend("moka")
        .sync_mode(true)
        .build();
    let backend = config.build_backend().await.expect("build moka sync");
    assert!(backend.is_some());
}
