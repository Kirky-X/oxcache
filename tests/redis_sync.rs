// Copyright (c) 2025-2026, Kirky.X
//
// MIT License

// tests/redis_sync.rs
//
// SyncCacheBackend 集成测试 for RedisBackend (任务组 8)
//
// 验证 RedisBackend 的同步 API（基于 tokio::task::block_in_place 桥接 async）：
//   - multi-thread runtime 下 sync get/set 正常工作
//   - current-thread runtime 下 sync get 返回 Err(NotSupported)
//   - sync set with TTL 过期生效
//   - sync expire 设置 TTL
//
// 运行需 Redis server：`redis-cli ping` 返回 PONG。Redis 不可用时测试被 #[ignore]。
// 启用方式：`cargo test --features redis --test redis_sync -- --ignored`

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use oxcache::backend::{RedisBackend, SyncCacheReader, SyncCacheWriter};
use oxcache::error::CacheError;

/// Redis 测试连接地址（与本仓库其他 redis 集成测试一致）
const REDIS_URL: &str = "redis://127.0.0.1:6379";
/// 测试 key 前缀，确保测试间互不干扰
const KEY_PREFIX: &str = "test_redis_sync:";

/// 全局唯一 ID 生成器，保证每个测试 key 唯一
static UID: AtomicU64 = AtomicU64::new(0);

/// 生成唯一测试 key
fn unique_key(suffix: &str) -> String {
    let id = UID.fetch_add(1, Ordering::SeqCst);
    format!("{}{}_{}", KEY_PREFIX, id, suffix)
}

/// 设置允许非 TLS 连接的环境变量并创建 RedisBackend
async fn make_backend() -> RedisBackend {
    std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
    RedisBackend::new(REDIS_URL)
        .await
        .expect("Failed to connect to Redis — start a Redis server (e.g. `redis-server`) before running this test")
}

// ============================================================================
// 测试用例
// ============================================================================

/// multi-thread runtime 下 sync get/set 基本流程。
///
/// `Runtime::new()` 创建多线程 runtime，sync 方法内部通过
/// `tokio::task::block_in_place` 桥接到 async 实现。
#[test]
#[ignore = "requires Redis server; run with: cargo test --features redis --test redis_sync -- --ignored"]
fn test_redis_sync_get_set_multi_thread_runtime() {
    let rt = tokio::runtime::Runtime::new().expect("failed to build multi-thread runtime");
    rt.block_on(async {
        let backend = make_backend().await;
        let key = unique_key("sync_get_set");

        // sync set
        SyncCacheWriter::set(&backend, &key, b"hello sync".to_vec(), None).expect("sync set failed");

        // sync get
        let val = SyncCacheReader::get(&backend, &key).expect("sync get failed");
        assert_eq!(val, Some(b"hello sync".to_vec()));

        // sync exists
        assert!(SyncCacheReader::exists(&backend, &key).expect("sync exists failed"));

        // sync delete
        SyncCacheWriter::delete(&backend, &key).expect("sync delete failed");
        assert!(!SyncCacheReader::exists(&backend, &key).expect("sync exists after delete failed"));
    });
}

/// current-thread runtime 下 sync get 必须返回 `Err(NotSupported)`，
/// 而不是 panic（block_in_place 在 current-thread runtime 上会 panic）。
#[test]
#[ignore = "requires Redis server; run with: cargo test --features redis --test redis_sync -- --ignored"]
fn test_redis_sync_get_current_thread_fails() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build current-thread runtime");
    rt.block_on(async {
        let backend = make_backend().await;
        let key = unique_key("sync_current_thread");

        // 在 current-thread runtime 下，sync get 必须返回 Err(NotSupported)
        let result = SyncCacheReader::get(&backend, &key);
        assert!(
            matches!(result, Err(CacheError::NotSupported(_))),
            "expected Err(NotSupported) on current-thread runtime, got {:?}",
            result
        );

        // sync set 同样应返回 Err(NotSupported)
        let result = SyncCacheWriter::set(&backend, &key, b"v".to_vec(), None);
        assert!(
            matches!(result, Err(CacheError::NotSupported(_))),
            "expected Err(NotSupported) for sync set on current-thread runtime, got {:?}",
            result
        );
    });
}

/// sync set 带 TTL，等待过期后 key 不存在。
#[test]
#[ignore = "requires Redis server; run with: cargo test --features redis --test redis_sync -- --ignored"]
fn test_redis_sync_set_with_ttl_expires() {
    let rt = tokio::runtime::Runtime::new().expect("failed to build multi-thread runtime");
    rt.block_on(async {
        let backend = make_backend().await;
        let key = unique_key("sync_ttl_expires");

        // sync set with 1s TTL
        SyncCacheWriter::set(&backend, &key, b"v".to_vec(), Some(Duration::from_secs(1)))
            .expect("sync set with ttl failed");
        assert!(SyncCacheReader::exists(&backend, &key).expect("sync exists failed"));

        // 等待过期
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // 过期后 key 不存在
        assert!(
            !SyncCacheReader::exists(&backend, &key).expect("sync exists after expiry failed"),
            "key should have expired"
        );
    });
}

/// sync expire 为已存在的 key 设置 TTL。
#[test]
#[ignore = "requires Redis server; run with: cargo test --features redis --test redis_sync -- --ignored"]
fn test_redis_sync_expire() {
    let rt = tokio::runtime::Runtime::new().expect("failed to build multi-thread runtime");
    rt.block_on(async {
        let backend = make_backend().await;
        let key = unique_key("sync_expire");

        // 先 set（无 TTL）
        SyncCacheWriter::set(&backend, &key, b"v".to_vec(), None).expect("sync set failed");

        // sync expire 返回 true（key 存在）
        let ok = SyncCacheWriter::expire(&backend, &key, Duration::from_secs(50)).expect("sync expire failed");
        assert!(ok, "expire should return true for existing key");

        // sync ttl 返回剩余时间
        let ttl = SyncCacheReader::ttl(&backend, &key).expect("sync ttl failed");
        assert!(ttl.is_some(), "ttl should be Some after expire");
        let secs = ttl.unwrap().as_secs();
        assert!(secs > 40 && secs <= 50, "ttl secs should be in (40, 50], got {}", secs);

        // sync expire 对不存在的 key 返回 false
        let missing = unique_key("sync_expire_missing");
        let ok = SyncCacheWriter::expire(&backend, &missing, Duration::from_secs(10)).expect("sync expire call failed");
        assert!(!ok, "expire should return false for missing key");

        // cleanup
        SyncCacheWriter::delete(&backend, &key).expect("sync delete failed");
    });
}
