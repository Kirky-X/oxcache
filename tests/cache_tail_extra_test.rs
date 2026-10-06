// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 长尾面覆盖：后端工厂注册表、redb 磁盘后端（open/create 失败、懒过期、
//! max_entries 清扫、set_many/统计面）、压缩装饰器（阈值两侧 + 批量 +
//! 全委托）、batch 批量读写、sync 原子操作（适配器注入 + NotSupported 兜
//! 底）、moka/dashmap 同步面 runtime 分支、统一序列化器构造面。

#![cfg(feature = "full")]

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::memory::dashmap::DashMapMemoryBackend;
use oxcache::backend::{
    BackendKind, CacheConnector, CacheReader, CacheSetItem, CacheWriter, SyncBackendAdapter,
    SyncCacheConnector, SyncCacheReader, SyncCacheWriter,
};
use oxcache::cache::CacheBuilder;
use oxcache::error::{OxCacheError, OxCacheResult};
use oxcache::features::compression::CompressingBackend;

fn key(k: &str) -> Arc<str> {
    Arc::from(k)
}

fn value(v: &[u8]) -> Arc<Vec<u8>> {
    Arc::new(v.to_vec())
}

// ============================================================================
// 后端工厂注册表
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn registry_builtin_factories_and_unknown_kind() {
    use oxcache::backend::{BackendRegistry, BackendSpec};

    let registry = BackendRegistry::global();
    let names = registry.registered();
    assert!(names.contains(&"moka".to_string()));
    assert!(names.contains(&"dashmap".to_string()));
    assert!(names.contains(&"memory".to_string()));
    assert!(names.contains(&"redis".to_string()));

    // moka 工厂（带容量与 TTL）
    let spec = BackendSpec::new("moka")
        .with_capacity(128)
        .with_capacity(128); // 重复设置幂等
    let backend = registry.build(&spec).await.unwrap();
    CacheWriter::set(backend.as_ref(), key("f"), value(b"1"), None)
        .await
        .unwrap();

    // dashmap 工厂（容量 + default_ttl）
    let spec = BackendSpec::new("dashmap").with_capacity(64);
    let backend = registry.build(&spec).await.unwrap();
    assert_eq!(CacheReader::len(backend.as_ref()).await.unwrap(), 0);

    // "memory" 别名工厂（default_ttl_ms > 0 分支经 spec 字段驱动，
    // 这里走默认 0 TTL 分支）
    let spec = BackendSpec::new("memory").with_capacity(32);
    let backend = registry.build(&spec).await.unwrap();
    let _ = CacheReader::capacity(backend.as_ref()).await.unwrap();

    // redis 工厂：不可达端点 → Connection 错误
    let spec = BackendSpec::new("redis").with_url("redis://127.0.0.1:1");
    assert!(registry.build(&spec).await.is_err());

    // 未知 kind：错误附带可用列表
    let spec = BackendSpec::new("no-such-backend");
    let refused = registry.build(&spec).await;
    let err = match refused {
        Err(e) => e,
        Ok(_) => panic!("未知 backend kind 必须显性报错"),
    };
    assert!(err.to_string().contains("unknown backend kind"));
    assert!(err.to_string().contains("moka"));
}

// ============================================================================
// redb 磁盘后端
// ============================================================================

#[tokio::test]
async fn disk_open_missing_and_create_in_missing_dir_fail() {
    use oxcache::backend::disk::RedbDiskBackend;

    // open：库文件不存在 → 错误
    let missing = std::env::temp_dir().join(format!("ox-tail-{}-missing.redb", std::process::id()));
    let _ = std::fs::remove_file(&missing);
    assert!(RedbDiskBackend::open(&missing).is_err());

    // create：父目录不存在 → 错误
    let nested = std::env::temp_dir()
        .join(format!("ox-tail-{}-no-dir", std::process::id()))
        .join("x.redb");
    assert!(RedbDiskBackend::create(&nested).is_err());
    let _ = std::fs::remove_file(&missing);
}

#[tokio::test(flavor = "multi_thread")]
async fn disk_backend_full_surface_with_lazy_expiry_and_sweep() {
    use oxcache::backend::disk::RedbDiskBackend;

    let dir = std::env::temp_dir().join(format!("ox-tail-disk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("cache.redb");

    // create 既有库文件时等效 open
    let disk = RedbDiskBackend::create(&path).unwrap();
    let disk = disk
        .with_default_ttl(Duration::from_millis(60))
        .with_max_entries(4);

    // 基本读写
    CacheWriter::set(
        &disk,
        key("d:1"),
        value(b"v1"),
        Some(Duration::from_secs(60)),
    )
    .await
    .unwrap();
    assert_eq!(
        CacheReader::get(&disk, "d:1").await.unwrap().as_deref(),
        Some(&b"v1"[..])
    );
    assert!(CacheReader::exists(&disk, "d:1").await.unwrap());

    // set_many（混合 TTL）+ get_many + keys + len + stats + capacity
    let items: Vec<CacheSetItem> = vec![
        (key("d:2"), value(b"v2"), Some(Duration::from_millis(60))),
        (key("d:3"), value(b"v3"), Some(Duration::from_secs(60))),
    ];
    CacheWriter::set_many(&disk, &items).await.unwrap();
    let got = CacheReader::get_many(&disk, &["d:1".to_string(), "d:3".to_string()])
        .await
        .unwrap();
    assert_eq!(got[0].as_deref(), Some(&b"v1"[..]));
    assert_eq!(got[1].as_deref(), Some(&b"v3"[..]));
    let _ = CacheReader::len(&disk).await.unwrap();
    let stats = CacheReader::stats(&disk).await.unwrap();
    assert_eq!(stats.get("backend").map(String::as_str), Some("disk"));
    assert!(stats.contains_key("path") && stats.contains_key("max_entries"));
    let _ = CacheReader::capacity(&disk).await.unwrap();
    let keys = CacheReader::keys(&disk, "d:*").await.unwrap();
    assert_eq!(keys.len(), 3);

    // default_ttl 过期 → 懒过期物理删除（get/exists 双路径）
    CacheWriter::set(&disk, key("d:exp"), value(b"e"), None)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(CacheReader::get(&disk, "d:exp").await.unwrap(), None);
    assert!(!CacheReader::exists(&disk, "d:exp").await.unwrap());
    assert_eq!(CacheReader::ttl(&disk, "d:exp").await.unwrap(), None);

    // expire + delete + clear
    assert!(
        CacheWriter::expire(&disk, "d:1", Duration::from_secs(30))
            .await
            .unwrap()
    );
    CacheWriter::delete(&disk, "d:1").await.unwrap();
    assert!(!CacheReader::exists(&disk, "d:1").await.unwrap());
    CacheWriter::clear(&disk).await.unwrap();
    assert_eq!(CacheReader::len(&disk).await.unwrap(), 0);

    // connector 面
    CacheConnector::health_check(&disk).await.unwrap();
    CacheConnector::shutdown(&disk).await;
    assert_eq!(CacheConnector::backend_kind(&disk), BackendKind::Disk);

    let _ = std::fs::remove_dir_all(&dir);
}

// ============================================================================
// 压缩装饰器
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn compression_threshold_and_full_delegation() {
    let inner = Arc::new(DashMapMemoryBackend::default());
    let compressed =
        CompressingBackend::with_threshold(Arc::clone(&inner) as Arc<_>, 64).with_level(3);

    // 小值：低于阈值不压缩
    CacheWriter::set(&compressed, key("small"), value(b"tiny"), None)
        .await
        .unwrap();
    assert_eq!(
        CacheReader::get(&compressed, "small")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"tiny"[..])
    );
    // inner 中存储的是原文
    assert_eq!(
        CacheReader::get(inner.as_ref(), "small")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"tiny"[..])
    );

    // 大值：达到阈值压缩，读回解压还原
    let big = vec![b'x'; 8_192];
    CacheWriter::set(&compressed, key("big"), value(&big), None)
        .await
        .unwrap();
    assert_eq!(
        CacheReader::get(&compressed, "big")
            .await
            .unwrap()
            .as_deref(),
        Some(big.as_slice())
    );
    // inner 中存储的是压缩字节（体积严格小于原文）
    let stored = CacheReader::get(inner.as_ref(), "big")
        .await
        .unwrap()
        .unwrap();
    assert!(stored.len() < big.len(), "压缩后体积必须更小");

    // 阈值访问器
    assert_eq!(compressed.threshold(), 64);
    let default_backend = CompressingBackend::new(Arc::clone(&inner) as Arc<_>);
    let _ = default_backend.threshold();

    // set_many（混合大小）与委托面
    let items: Vec<CacheSetItem> = vec![
        (key("m:s"), value(b"s"), None),
        (key("m:b"), value(&big), Some(Duration::from_secs(30))),
    ];
    CacheWriter::set_many(&compressed, &items).await.unwrap();
    assert_eq!(
        CacheReader::get(&compressed, "m:b")
            .await
            .unwrap()
            .as_deref(),
        Some(big.as_slice())
    );
    CacheWriter::delete_many(&compressed, &["m:b".to_string()])
        .await
        .unwrap();
    assert!(!CacheReader::exists(&compressed, "m:b").await.unwrap());
    let _ = CacheReader::ttl(&compressed, "m:s").await.unwrap();
    let _ = CacheReader::len(&compressed).await.unwrap();
    let _ = CacheReader::capacity(&compressed).await.unwrap();
    let _ = CacheReader::stats(&compressed).await.unwrap();
    let _ = CacheReader::keys(&compressed, "m:*").await.unwrap();
    CacheWriter::delete(&compressed, "m:s").await.unwrap();
    CacheWriter::expire(&compressed, "small", Duration::from_secs(5))
        .await
        .unwrap();
    CacheConnector::health_check(&compressed).await.unwrap();
    CacheConnector::shutdown(&compressed).await;
    assert_eq!(
        CacheConnector::backend_kind(&compressed),
        BackendKind::DashMap
    );
    CacheWriter::clear(&compressed).await.unwrap();
    assert_eq!(CacheReader::len(&compressed).await.unwrap(), 0);
}

// ============================================================================
// Cache 批量读写面
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn cache_batch_set_many_and_get_many() {
    let cache: oxcache::Cache<String, String> = CacheBuilder::default().build().await.unwrap();

    let items = [
        ("b:1".to_string(), "v1".to_string()),
        ("b:2".to_string(), "v2".to_string()),
    ];
    cache
        .set_many(items.iter().map(|(k, v)| (k, v)))
        .await
        .unwrap();
    let got = cache
        .get_many(
            [
                "b:1".to_string(),
                "b:missing".to_string(),
                "b:2".to_string(),
            ]
            .iter(),
        )
        .await
        .unwrap();
    assert_eq!(got.get("b:1").map(String::as_str), Some("v1"));
    assert_eq!(got.get("b:2").map(String::as_str), Some("v2"));
    assert!(!got.contains_key("b:missing"));

    // 带 TTL 的批量写
    cache
        .set_many_with_ttl(
            [("b:3".to_string(), "v3".to_string())]
                .iter()
                .map(|(k, v)| (k, v)),
            Some(Duration::from_secs(30)),
        )
        .await
        .unwrap();
    assert_eq!(
        cache.get(&"b:3".to_string()).await.unwrap().as_deref(),
        Some("v3")
    );
}

// ============================================================================
// sync 原子操作：适配器注入（happy）与无原子能力兜底（NotSupported）
// ============================================================================

struct AtomicMock {
    data: std::sync::Mutex<std::collections::HashMap<String, i64>>,
}

impl AtomicMock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            data: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
    }
}

impl SyncCacheReader for AtomicMock {
    fn get(&self, _key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        Ok(None)
    }
    fn exists(&self, _key: &str) -> OxCacheResult<bool> {
        Ok(false)
    }
    fn ttl(&self, _key: &str) -> OxCacheResult<Option<Duration>> {
        Ok(None)
    }
    fn len(&self) -> OxCacheResult<u64> {
        Ok(0)
    }
    fn capacity(&self) -> OxCacheResult<u64> {
        Ok(0)
    }
    fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
        Ok(Default::default())
    }
}

impl SyncCacheWriter for AtomicMock {
    fn set(
        &self,
        _key: Arc<str>,
        _value: Arc<Vec<u8>>,
        _ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        Ok(())
    }
    fn delete(&self, _key: &str) -> OxCacheResult<()> {
        Ok(())
    }
    fn clear(&self) -> OxCacheResult<()> {
        Ok(())
    }
    fn expire(&self, _key: &str, _ttl: Duration) -> OxCacheResult<bool> {
        Ok(false)
    }
}

impl SyncCacheConnector for AtomicMock {
    fn health_check(&self) -> OxCacheResult<()> {
        Ok(())
    }
    fn shutdown(&self) {}
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Unknown
    }
    fn as_sync_atomic_writer(&self) -> Option<&dyn oxcache::backend::SyncAtomicCacheWriter> {
        Some(self)
    }
}

impl oxcache::backend::SyncAtomicCacheWriter for AtomicMock {
    fn incr(&self, key: &str, delta: i64, _ttl: Option<Duration>) -> OxCacheResult<i64> {
        let mut data = self.data.lock().unwrap();
        let next = data.entry(key.to_string()).or_insert(0);
        *next += delta;
        Ok(*next)
    }
    fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        _ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let mut data = self.data.lock().unwrap();
        let cur = data.get(key).map(|v| v.to_le_bytes().to_vec());
        let matches = match (cur.as_deref(), expected) {
            (Some(c), Some(e)) => c == e,
            (None, None) => true,
            _ => false,
        };
        if matches {
            let val = i64::from_le_bytes(new.as_slice().try_into().unwrap_or([0; 8]));
            data.insert(key.to_string(), val);
        }
        Ok(matches)
    }
    fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        _ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let mut data = self.data.lock().unwrap();
        if data.contains_key(key) {
            Ok(false)
        } else {
            let val = i64::from_le_bytes(value.as_slice().try_into().unwrap_or([0; 8]));
            data.insert(key.to_string(), val);
            Ok(true)
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_sync_atomic_ops_via_adapter() {
    let adapter = SyncBackendAdapter::new(AtomicMock::new());
    let cache: oxcache::Cache<String, i64> = CacheBuilder::default()
        .backend_arc(Arc::new(adapter))
        .sync_mode(true)
        .build()
        .await
        .unwrap();

    assert_eq!(cache.incr_sync(&"c".to_string(), 5, None).unwrap(), 5);
    assert_eq!(cache.incr_sync(&"c".to_string(), 2, None).unwrap(), 7);
    assert!(
        cache
            .set_if_absent_sync(&"n".to_string(), &42, None)
            .unwrap()
    );
    assert!(
        !cache
            .set_if_absent_sync(&"n".to_string(), &43, None)
            .unwrap()
    );
    assert!(
        cache
            .compare_and_swap_sync(
                &"c".to_string(),
                Some(7i64.to_le_bytes().as_slice()),
                9i64.to_le_bytes().to_vec(),
                None,
            )
            .unwrap()
    );
}

// ============================================================================
// moka / dashmap 同步面 runtime 分支与 incr 异常值
// ============================================================================

#[tokio::test]
async fn moka_sync_surface_in_current_thread_runtime_is_rejected() {
    use oxcache::backend::{MokaMemoryBackend, SyncCacheWriter};

    // current_thread 异步上下文：block_in_place 禁止 → 显性 NotSupported
    let backend = MokaMemoryBackend::builder().build();
    let err = SyncCacheWriter::set(&backend, key("k"), value(b"v"), None).unwrap_err();
    assert!(err.to_string().contains("current-thread"));
}

#[tokio::test(flavor = "multi_thread")]
async fn dashmap_sync_surface_works_on_multi_thread() {
    let backend = DashMapMemoryBackend::default();
    SyncCacheWriter::set(&backend, key("k"), value(b"v"), None).unwrap();
    assert_eq!(
        SyncCacheReader::get(&backend, "k").unwrap().as_deref(),
        Some(&b"v"[..])
    );
    // 过期条目：exists/ttl 的 remove_if 分支
    CacheWriter::set(
        &backend,
        key("exp"),
        value(b"e"),
        Some(Duration::from_millis(40)),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(90)).await;
    assert!(!CacheReader::exists(&backend, "exp").await.unwrap());
    assert_eq!(CacheReader::ttl(&backend, "exp").await.unwrap(), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn moka_incr_with_non_numeric_value_fails_explicitly() {
    use oxcache::backend::AtomicCacheWriter;
    use oxcache::backend::MokaMemoryBackend;

    let backend = MokaMemoryBackend::builder().build();
    CacheWriter::set(&backend, key("ctr"), value(b"not-a-number"), None)
        .await
        .unwrap();
    let err = AtomicCacheWriter::incr(&backend, "ctr", 1, None)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("incr"),
        "incr 对非数字值必须显性报错，实际: {err}"
    );
}

// ============================================================================
// 统一序列化器构造面
// ============================================================================

#[test]
fn unified_serializer_constructor_variants() {
    use oxcache::infra::UnifiedSerializer;
    use oxcache::infra::serialization::SerializationFormat;

    let json = UnifiedSerializer::json();
    assert_eq!(json.format(), SerializationFormat::Json);
    let fmt = UnifiedSerializer::with_format(SerializationFormat::Json);
    assert_eq!(fmt.format(), SerializationFormat::Json);
    let compressed = UnifiedSerializer::with_compression();
    assert_eq!(compressed.format(), SerializationFormat::Json);
    // 往返
    let bytes = compressed.serialize(&"payload".to_string()).unwrap();
    assert_eq!(compressed.deserialize::<String>(&bytes).unwrap(), "payload");
    let _ = OxCacheError::L1Error("keep imports".to_string());
}
