// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 临时执行探针（E2E 场景 6）：TLS 强制门控 —— `redis://` 拒绝、`rediss://`
//! 前缀跳过门控、`OXCACHE_ALLOW_INSECURE_REDIS` 豁免变量对真实非 TLS 服务放行。
//!
//! 依据：`src/backend/memory/redis/builder.rs:225-244`（门控实现），
//! `docs/USER_GUIDE.md:535-537`、`docs/API_REFERENCE.md:413-415`（契约描述）。

#![cfg(feature = "redis")]

#[path = "common/mod.rs"]
pub mod common;

use serial_test::serial;

use oxcache::OxCacheError;
use oxcache::backend::{CacheConnector, CacheReader, CacheWriter, RedisBackend};

const TLS_REJECTION_MSG: &str = "must use TLS";
const EXEMPT_KEY: &str = "OXCACHE_ALLOW_INSECURE_REDIS";
const EXEMPT_VALUE: &str = "I_UNDERSTAND_THE_RISKS";

// SAFETY: edition 2024 set_var 为 unsafe；env 测试以 #[serial] 串行，进程级 env 无并发读写
#[allow(unsafe_code)]
fn set_env(key: &str, value: &str) {
    unsafe { std::env::set_var(key, value) };
}

// SAFETY: edition 2024 remove_var 为 unsafe；env 测试以 #[serial] 串行，进程级 env 无并发读写
#[allow(unsafe_code)]
fn remove_env(key: &str) {
    unsafe { std::env::remove_var(key) };
}

/// 豁免变量守卫：测试结束时恢复环境（unset）
struct EnvGuard(&'static str);
impl Drop for EnvGuard {
    fn drop(&mut self) {
        remove_env(self.0);
    }
}

/// redis://（无豁免变量）→ 门控拒绝，`InvalidInput` 附 TLS 要求文案。
/// 门控在连接建立前执行（builder.rs:225），无需真实服务。
#[tokio::test]
#[serial]
async fn probe6_non_tls_rejected_without_exemption() {
    remove_env(EXEMPT_KEY);
    let _guard = EnvGuard(EXEMPT_KEY);

    let err = match RedisBackend::new("redis://127.0.0.1:6379").await {
        Ok(_) => panic!("non-TLS URL must be rejected when the exemption is unset"),
        Err(e) => e,
    };
    match &err {
        OxCacheError::InvalidInput(msg) => {
            assert!(
                msg.contains(TLS_REJECTION_MSG),
                "rejection must carry the TLS requirement, got: {msg}"
            );
            assert!(
                msg.contains(EXEMPT_KEY),
                "rejection must name the exemption variable, got: {msg}"
            );
        }
        other => panic!("expected InvalidInput(TLS rejection), got {other:?}"),
    }
}

/// rediss:// 前缀跳过门控：错误不得是 TLS 拒绝文案。本 crate 的 redis 依赖
/// 未启用 TLS feature，连接自身无法成立；断言语义为「失败点在门控之后
/// （连接/URL 层），`rediss://` 已越过 TLS 检查」。
#[tokio::test]
#[serial]
async fn probe6_rediss_prefix_passes_the_tls_gate() {
    remove_env(EXEMPT_KEY);
    let _guard = EnvGuard(EXEMPT_KEY);

    let err = match RedisBackend::new("rediss://127.0.0.1:6379").await {
        Ok(_) => panic!("no TLS-capable server at this endpoint"),
        Err(e) => e,
    };
    let debug = format!("{err:?}");
    assert!(
        !debug.contains(TLS_REJECTION_MSG),
        "rediss:// must not be stopped by the TLS gate, got: {debug}"
    );
    println!("[probe6] rediss:// failure (post-gate): {err:?}");
}

/// 豁免变量置位 → 非 TLS redis:// 对真实 Redis 服务放行：构造成功、
/// 健康检查与读写可用（testcontainers 自起真实 redis:7-alpine）。
#[tokio::test]
#[serial]
async fn probe6_exemption_allows_non_tls_against_real_redis() {
    let Some((container, url)) = common::test_containers::start_redis_container().await.ok() else {
        println!("[probe6] container unavailable; exemption-pass case not executed");
        return;
    };

    set_env(EXEMPT_KEY, EXEMPT_VALUE);
    let _guard = EnvGuard(EXEMPT_KEY);

    let backend = RedisBackend::new(&url)
        .await
        .expect("exemption variable must allow a non-TLS redis:// URL");
    backend
        .health_check()
        .await
        .expect("backend must be healthy after construction");

    CacheWriter::set(
        &backend,
        "probe6-key".into(),
        "v1".as_bytes().to_vec().into(),
        None,
    )
    .await
    .expect("set must work through the exempted backend");
    let got = CacheReader::get(&backend, "probe6-key").await.expect("get");
    assert_eq!(got.as_deref(), Some(b"v1".as_slice()));
    drop(container);
}
