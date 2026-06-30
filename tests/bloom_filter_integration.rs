// tests/bloom_filter_integration.rs
//
// BloomFilter + BloomFilterBackend 端到端集成测试
//
// 验证 BloomFilter 独立类型与 BloomFilterBackend 装饰器在 Moka 后端上的
// 端到端行为：BF 过滤负查询、set 更新 BF 与 inner、TTL 透传、sync API。
//
// 注意：BloomFilterBackend 同时实现 CacheReader/CacheWriter（async）和
// SyncCacheReader/SyncCacheWriter（sync），方法名相同（get/set/clear 等）。
// 直接 `.await` 调用无法消歧，必须使用 UFCS（`CacheReader::get(&backend, ...).await`）。

#![cfg(feature = "bloom-filter")]

use std::time::Duration;

use oxcache::backend::interface::{CacheReader, CacheWriter, SyncCacheReader, SyncCacheWriter};
use oxcache::backend::MokaMemoryBackend;
use oxcache::features::bloom_filter::{BloomFilter, BloomFilterBackend};

// ============================================================================
// BloomFilter 独立类型
// ============================================================================

#[test]
fn test_bloom_filter_basic_lifecycle() {
    let bf = BloomFilter::new(10_000, 0.01);

    // 空时 contains 返回 false
    assert!(!bf.contains("missing"));

    // insert 后 contains 返回 true
    bf.insert("key1");
    bf.insert("key2");
    assert!(bf.contains("key1"));
    assert!(bf.contains("key2"));

    // clear 重置
    bf.clear();
    assert!(!bf.contains("key1"));
    assert!(!bf.contains("key2"));
}

#[test]
fn test_bloom_filter_no_false_negatives() {
    // 插入 1000 个 key，所有已插入的 key 都必须 contains=true（无假阴性）
    let bf = BloomFilter::new(10_000, 0.01);
    for i in 0..1000 {
        bf.insert(&format!("key_{}", i));
    }
    for i in 0..1000 {
        assert!(
            bf.contains(&format!("key_{}", i)),
            "BF must not have false negatives for key_{}",
            i
        );
    }
}

// ============================================================================
// BloomFilterBackend 装饰器（async + Moka）
// ============================================================================

#[tokio::test]
async fn test_bf_backend_get_miss_skips_inner() {
    // BF 未命中时，inner 不被调用（返回 None）
    let inner = MokaMemoryBackend::new();
    let inner_ref = inner.clone();
    let backend = BloomFilterBackend::new(inner);

    // 查询不存在的 key：BF 未命中，返回 None
    let value = CacheReader::get(&backend, "missing").await.unwrap();
    assert_eq!(value, None);

    // inner 也不应有该 key
    assert_eq!(CacheReader::get(&inner_ref, "missing").await.unwrap(), None);
}

#[tokio::test]
async fn test_bf_backend_set_updates_bloom_and_inner() {
    let inner = MokaMemoryBackend::new();
    let inner_ref = inner.clone();
    let backend = BloomFilterBackend::new(inner);

    // set 后 BF 和 inner 都应反映该 key
    CacheWriter::set(&backend, "k", b"v".to_vec(), None).await.unwrap();

    // BF 应包含该 key
    assert!(backend.bloom().contains("k"));

    // inner 应有该值
    assert_eq!(CacheReader::get(&inner_ref, "k").await.unwrap(), Some(b"v".to_vec()));

    // 装饰器 get 应返回值
    assert_eq!(CacheReader::get(&backend, "k").await.unwrap(), Some(b"v".to_vec()));
}

#[tokio::test]
async fn test_bf_backend_delete_does_not_modify_bloom() {
    let inner = MokaMemoryBackend::new();
    let backend = BloomFilterBackend::new(inner);

    CacheWriter::set(&backend, "k", b"v".to_vec(), None).await.unwrap();
    assert!(backend.bloom().contains("k"));

    // delete 不修改 BF（BF 不支持删除）
    CacheWriter::delete(&backend, "k").await.unwrap();
    assert!(
        backend.bloom().contains("k"),
        "BF should still contain key after delete"
    );

    // 但 inner 已删除，装饰器 get 返回 None（BF 命中但 inner miss）
    assert_eq!(CacheReader::get(&backend, "k").await.unwrap(), None);
}

#[tokio::test]
async fn test_bf_backend_set_with_ttl_passes_through() {
    let inner = MokaMemoryBackend::new();
    let inner_ref = inner.clone();
    let backend = BloomFilterBackend::new(inner);

    // set with 50ms TTL
    CacheWriter::set(&backend, "k", b"v".to_vec(), Some(Duration::from_millis(50)))
        .await
        .unwrap();

    // 立即查询应返回 Some
    assert_eq!(CacheReader::get(&backend, "k").await.unwrap(), Some(b"v".to_vec()));

    // 等 100ms 让 TTL 过期
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Moka 异步清理可能略有延迟，循环等待
    let mut expired = false;
    for _ in 0..10 {
        if CacheReader::get(&backend, "k").await.unwrap().is_none() {
            expired = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(expired, "BF backend should return None after TTL expires");

    // inner 也应过期
    let mut inner_expired = false;
    for _ in 0..10 {
        if CacheReader::get(&inner_ref, "k").await.unwrap().is_none() {
            inner_expired = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(inner_expired, "inner backend should also expire after TTL");
}

#[tokio::test]
async fn test_bf_backend_clear_clears_both() {
    let inner = MokaMemoryBackend::new();
    let inner_ref = inner.clone();
    let backend = BloomFilterBackend::new(inner);

    CacheWriter::set(&backend, "k1", b"v1".to_vec(), None).await.unwrap();
    CacheWriter::set(&backend, "k2", b"v2".to_vec(), None).await.unwrap();

    // clear 应清空 inner 和 BF
    CacheWriter::clear(&backend).await.unwrap();

    assert!(!backend.bloom().contains("k1"));
    assert!(!backend.bloom().contains("k2"));
    assert_eq!(CacheReader::get(&inner_ref, "k1").await.unwrap(), None);
    assert_eq!(CacheReader::get(&inner_ref, "k2").await.unwrap(), None);
}

// ============================================================================
// BloomFilterBackend sync API（TG14）
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_bf_backend_sync_get_set_basic() {
    let inner = MokaMemoryBackend::new();
    let backend = BloomFilterBackend::new(inner);

    // sync set + get
    SyncCacheWriter::set(&backend, "k", b"v".to_vec(), None).unwrap();
    let value = SyncCacheReader::get(&backend, "k").unwrap();
    assert_eq!(value, Some(b"v".to_vec()));

    // BF 应包含该 key
    assert!(backend.bloom().contains("k"));

    // sync get miss
    let value = SyncCacheReader::get(&backend, "missing").unwrap();
    assert_eq!(value, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_bf_backend_sync_set_with_ttl_passes_through() {
    let inner = MokaMemoryBackend::new();
    let backend = BloomFilterBackend::new(inner);

    // sync set with 50ms TTL
    SyncCacheWriter::set(&backend, "k", b"v".to_vec(), Some(Duration::from_millis(50))).unwrap();

    // 立即查询应返回 Some
    let value = SyncCacheReader::get(&backend, "k").unwrap();
    assert_eq!(value, Some(b"v".to_vec()));

    // 等 100ms 让 TTL 过期
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 循环等待 Moka 清理
    let mut expired = false;
    for _ in 0..10 {
        if SyncCacheReader::get(&backend, "k").unwrap().is_none() {
            expired = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(expired, "BF backend sync get should return None after TTL expires");
}
