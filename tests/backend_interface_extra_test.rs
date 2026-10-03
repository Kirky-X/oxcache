// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! `backend::interface` 面覆盖测试：`SyncBackendAdapter`（同步后端 → async 门面）、
//! async/sync trait 默认方法（is_empty / get_many / keys / as_lua_executor /
//! as_atomic_writer / as_sync_atomic_writer）、`glob_match` 与 `BackendKind`
//! 判别面。这些路径此前无任何直接测试。

#![cfg(feature = "full")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
#[cfg(feature = "lua")]
use oxcache::backend::LuaExecutor;
use oxcache::backend::interface::glob_match;
use oxcache::backend::{
    AtomicCacheWriter, BackendKind, CacheConnector, CacheReader, CacheSetItem, CacheWriter,
    SyncBackendAdapter, SyncCacheBackend, SyncCacheConnector, SyncCacheReader, SyncCacheWriter,
};
use oxcache::error::{OxCacheError, OxCacheResult};

// ============================================================================
// 测试替身一：全覆写 mock（sync + async 双面实现）
// ============================================================================

#[derive(Default)]
struct FullState {
    data: HashMap<String, Vec<u8>>,
    shutdowns: u32,
    counters: HashMap<String, i64>,
}

struct FullMock {
    state: Arc<Mutex<FullState>>,
    kind: BackendKind,
}

impl FullMock {
    fn new(kind: BackendKind) -> (Arc<Self>, Arc<Mutex<FullState>>) {
        let state = Arc::new(Mutex::new(FullState::default()));
        (
            Arc::new(Self {
                state: Arc::clone(&state),
                kind,
            }),
            state,
        )
    }
}

impl SyncCacheReader for FullMock {
    fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        Ok(self.state.lock().unwrap().data.get(key).cloned())
    }

    fn exists(&self, key: &str) -> OxCacheResult<bool> {
        Ok(self.state.lock().unwrap().data.contains_key(key))
    }

    fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        let _ = key;
        Ok(None)
    }

    fn len(&self) -> OxCacheResult<u64> {
        Ok(self.state.lock().unwrap().data.len() as u64)
    }

    fn capacity(&self) -> OxCacheResult<u64> {
        Ok(1024)
    }

    fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        Ok(HashMap::from([("kind".to_string(), "full".to_string())]))
    }

    fn get_many(&self, keys: &[String]) -> OxCacheResult<Vec<Option<Vec<u8>>>> {
        let st = self.state.lock().unwrap();
        Ok(keys.iter().map(|k| st.data.get(k).cloned()).collect())
    }

    fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        let st = self.state.lock().unwrap();
        Ok(st
            .data
            .keys()
            .filter(|k| glob_match(pattern, k))
            .cloned()
            .collect())
    }
}

impl SyncCacheWriter for FullMock {
    fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, _ttl: Option<Duration>) -> OxCacheResult<()> {
        self.state
            .lock()
            .unwrap()
            .data
            .insert(key.to_string(), value.as_ref().clone());
        Ok(())
    }

    fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.state.lock().unwrap().data.remove(key);
        Ok(())
    }

    fn clear(&self) -> OxCacheResult<()> {
        self.state.lock().unwrap().data.clear();
        Ok(())
    }

    fn expire(&self, key: &str, _ttl: Duration) -> OxCacheResult<bool> {
        Ok(self.state.lock().unwrap().data.contains_key(key))
    }

    fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        let mut st = self.state.lock().unwrap();
        for (key, value, _) in items {
            st.data.insert(key.to_string(), value.as_ref().clone());
        }
        Ok(())
    }

    fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        let mut st = self.state.lock().unwrap();
        for k in keys {
            st.data.remove(k);
        }
        Ok(())
    }
}

impl SyncCacheConnector for FullMock {
    fn health_check(&self) -> OxCacheResult<()> {
        Ok(())
    }

    fn shutdown(&self) {
        self.state.lock().unwrap().shutdowns += 1;
    }

    fn backend_kind(&self) -> BackendKind {
        self.kind
    }

    fn as_sync_atomic_writer(&self) -> Option<&dyn oxcache::backend::SyncAtomicCacheWriter> {
        Some(self)
    }
}

impl oxcache::backend::SyncAtomicCacheWriter for FullMock {
    fn incr(&self, key: &str, delta: i64, _ttl: Option<Duration>) -> OxCacheResult<i64> {
        let mut st = self.state.lock().unwrap();
        let next = st.counters.entry(key.to_string()).or_insert(0);
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
        let mut st = self.state.lock().unwrap();
        let matches = match (st.data.get(key), expected) {
            (Some(cur), Some(exp)) => cur.as_slice() == exp,
            (None, None) => true,
            _ => false,
        };
        if matches {
            st.data.insert(key.to_string(), new);
        }
        Ok(matches)
    }

    fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        _ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let mut st = self.state.lock().unwrap();
        if st.data.contains_key(key) {
            Ok(false)
        } else {
            st.data.insert(key.to_string(), value);
            Ok(true)
        }
    }
}

// ============================================================================
// 测试替身二：零可选覆写 mock——trait 默认方法全覆盖
// ============================================================================

struct BareMock {
    data: Mutex<HashMap<String, Vec<u8>>>,
    /// 是否在 async 原子探针中如实呈现能力（驱动桥接两分支）
    atomic: bool,
}

impl BareMock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            data: Mutex::new(HashMap::new()),
            atomic: false,
        })
    }

    fn with_atomic() -> Arc<Self> {
        Arc::new(Self {
            data: Mutex::new(HashMap::new()),
            atomic: true,
        })
    }
}

impl SyncCacheReader for BareMock {
    fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        Ok(self.data.lock().unwrap().get(key).cloned())
    }
    fn exists(&self, key: &str) -> OxCacheResult<bool> {
        Ok(self.data.lock().unwrap().contains_key(key))
    }
    fn ttl(&self, _key: &str) -> OxCacheResult<Option<Duration>> {
        Ok(None)
    }
    fn len(&self) -> OxCacheResult<u64> {
        Ok(self.data.lock().unwrap().len() as u64)
    }
    fn capacity(&self) -> OxCacheResult<u64> {
        Ok(0)
    }
    fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        Ok(HashMap::new())
    }
}

impl SyncCacheWriter for BareMock {
    fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, _ttl: Option<Duration>) -> OxCacheResult<()> {
        self.data
            .lock()
            .unwrap()
            .insert(key.to_string(), value.as_ref().clone());
        Ok(())
    }
    fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.data.lock().unwrap().remove(key);
        Ok(())
    }
    fn clear(&self) -> OxCacheResult<()> {
        self.data.lock().unwrap().clear();
        Ok(())
    }
    fn expire(&self, key: &str, _ttl: Duration) -> OxCacheResult<bool> {
        Ok(self.data.lock().unwrap().contains_key(key))
    }
}

impl SyncCacheConnector for BareMock {
    fn health_check(&self) -> OxCacheResult<()> {
        Ok(())
    }
    fn shutdown(&self) {}
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Unknown
    }
    // as_sync_atomic_writer 不覆写 → 默认 None
}

// async 面：不覆写 is_empty / get_many / keys → 走 CacheReader 默认实现
#[async_trait]
impl CacheReader for BareMock {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        Ok(self.data.lock().unwrap().get(key).cloned())
    }
    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        Ok(self.data.lock().unwrap().contains_key(key))
    }
    async fn ttl(&self, _key: &str) -> OxCacheResult<Option<Duration>> {
        Ok(None)
    }
    async fn len(&self) -> OxCacheResult<u64> {
        Ok(self.data.lock().unwrap().len() as u64)
    }
    async fn capacity(&self) -> OxCacheResult<u64> {
        Ok(0)
    }
    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        Ok(HashMap::new())
    }
}

#[async_trait]
impl CacheWriter for BareMock {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        _ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        self.data
            .lock()
            .unwrap()
            .insert(key.to_string(), value.as_ref().clone());
        Ok(())
    }
    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.data.lock().unwrap().remove(key);
        Ok(())
    }
    async fn clear(&self) -> OxCacheResult<()> {
        self.data.lock().unwrap().clear();
        Ok(())
    }
    async fn expire(&self, key: &str, _ttl: Duration) -> OxCacheResult<bool> {
        Ok(self.data.lock().unwrap().contains_key(key))
    }
}

#[async_trait]
impl CacheConnector for BareMock {
    async fn health_check(&self) -> OxCacheResult<()> {
        Ok(())
    }
    async fn shutdown(&self) {}
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Unknown
    }
    fn as_atomic_writer(&self) -> Option<&dyn AtomicCacheWriter> {
        if self.atomic { Some(self) } else { None }
    }
}

#[async_trait]
impl AtomicCacheWriter for BareMock {
    async fn incr(&self, key: &str, delta: i64, _ttl: Option<Duration>) -> OxCacheResult<i64> {
        let mut data = self.data.lock().unwrap();
        let cur: i64 = data
            .get(key)
            .and_then(|v| String::from_utf8_lossy(v).parse().ok())
            .unwrap_or(0);
        let next = cur + delta;
        data.insert(key.to_string(), next.to_string().into_bytes());
        Ok(next)
    }
    async fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        _ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let mut data = self.data.lock().unwrap();
        let matches = match (data.get(key), expected) {
            (Some(cur), Some(exp)) => cur.as_slice() == exp,
            (None, None) => true,
            _ => false,
        };
        if matches {
            data.insert(key.to_string(), new);
        }
        Ok(matches)
    }
    async fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        _ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let mut data = self.data.lock().unwrap();
        if data.contains_key(key) {
            Ok(false)
        } else {
            data.insert(key.to_string(), value);
            Ok(true)
        }
    }
}

// ============================================================================
// SyncBackendAdapter：同步后端 → async 门面（全覆写替身）
// ============================================================================

#[tokio::test]
async fn sync_backend_adapter_async_surface() {
    let (backend, state) = FullMock::new(BackendKind::Unknown);
    let adapter = SyncBackendAdapter::new(backend);

    // async CacheReader 委托
    assert_eq!(CacheReader::get(&adapter, "a").await.unwrap(), None);
    assert!(!CacheReader::exists(&adapter, "a").await.unwrap());
    assert_eq!(CacheReader::ttl(&adapter, "a").await.unwrap(), None);
    assert_eq!(CacheReader::len(&adapter).await.unwrap(), 0);
    assert_eq!(CacheReader::capacity(&adapter).await.unwrap(), 1024);
    let stats = CacheReader::stats(&adapter).await.unwrap();
    assert_eq!(stats.get("kind").map(String::as_str), Some("full"));
    CacheWriter::set(&adapter, Arc::from("a"), Arc::new(b"va".to_vec()), None)
        .await
        .unwrap();
    CacheWriter::set(&adapter, Arc::from("b"), Arc::new(b"vb".to_vec()), None)
        .await
        .unwrap();
    let many = CacheReader::get_many(&adapter, &["a".to_string(), "b".to_string()])
        .await
        .unwrap();
    assert_eq!(many[0].as_deref(), Some(&b"va"[..]));
    assert_eq!(many[1].as_deref(), Some(&b"vb"[..]));
    let keys = CacheReader::keys(&adapter, "a*").await.unwrap();
    assert_eq!(keys, vec!["a".to_string()]);

    // async CacheWriter 委托
    assert!(
        CacheWriter::expire(&adapter, "a", Duration::from_secs(1))
            .await
            .unwrap()
    );
    assert!(
        !CacheWriter::expire(&adapter, "zz", Duration::from_secs(1))
            .await
            .unwrap()
    );
    let items: Vec<CacheSetItem> = vec![(Arc::from("c"), Arc::new(b"vc".to_vec()), None)];
    CacheWriter::set_many(&adapter, &items).await.unwrap();
    assert_eq!(
        CacheReader::get(&adapter, "c").await.unwrap().as_deref(),
        Some(&b"vc"[..])
    );
    CacheWriter::delete_many(&adapter, &["b".to_string(), "c".to_string()])
        .await
        .unwrap();
    CacheWriter::delete(&adapter, "c").await.unwrap();
    assert!(CacheReader::exists(&adapter, "a").await.unwrap());
    assert!(!CacheReader::exists(&adapter, "b").await.unwrap());

    // async CacheConnector 委托
    CacheConnector::health_check(&adapter).await.unwrap();
    CacheConnector::shutdown(&adapter).await;
    assert_eq!(state.lock().unwrap().shutdowns, 1);
    assert_eq!(CacheConnector::backend_kind(&adapter), BackendKind::Unknown);

    // 原子探针：adapter 按 inner 的 as_sync_atomic_writer 判定 → Some(self)
    assert!(CacheConnector::as_atomic_writer(&adapter).is_some());
    #[cfg(feature = "lua")]
    assert!(CacheConnector::as_lua_executor(&adapter).is_none());
    // AtomicCacheWriter 经 inner 同步原子面委托
    let atomic = CacheConnector::as_atomic_writer(&adapter).unwrap();
    assert_eq!(
        AtomicCacheWriter::incr(atomic, "ac", 4, None)
            .await
            .unwrap(),
        4
    );
    assert!(
        AtomicCacheWriter::set_if_absent(atomic, "anx", b"av".to_vec(), None)
            .await
            .unwrap()
    );
    assert!(
        AtomicCacheWriter::compare_and_swap(
            atomic,
            "anx",
            Some(b"av".as_slice()),
            b"av2".to_vec(),
            None
        )
        .await
        .unwrap()
    );
}

#[test]
fn sync_backend_adapter_sync_surface_and_atomic_probe() {
    let (backend, _state) = FullMock::new(BackendKind::Mock);
    let adapter = SyncBackendAdapter::new(backend);

    assert_eq!(SyncCacheReader::get(&adapter, "x").unwrap(), None);
    SyncCacheWriter::set(&adapter, Arc::from("x"), Arc::new(b"vx".to_vec()), None).unwrap();
    assert_eq!(
        SyncCacheReader::get(&adapter, "x").unwrap().as_deref(),
        Some(&b"vx"[..])
    );
    assert_eq!(SyncCacheReader::len(&adapter).unwrap(), 1);
    let got = SyncCacheReader::get_many(&adapter, &["x".to_string(), "y".to_string()]).unwrap();
    assert_eq!(got.len(), 2);
    let keys = SyncCacheReader::keys(&adapter, "x*").unwrap();
    assert_eq!(keys, vec!["x".to_string()]);
    assert!(SyncCacheConnector::health_check(&adapter).is_ok());
    SyncCacheWriter::delete(&adapter, "x").unwrap();
    assert!(SyncCacheReader::is_empty(&adapter).unwrap());

    // adapter 未覆写 as_sync_atomic_writer → 默认 None（async 面探针承担桥接）
    assert!(SyncCacheConnector::as_sync_atomic_writer(&adapter).is_none());

    // 原子探针如实呈现：FullMock 自身实现 SyncAtomicCacheWriter → Some
    let (direct, _st) = FullMock::new(BackendKind::Mock);
    let writer = SyncCacheConnector::as_sync_atomic_writer(direct.as_ref());
    assert!(writer.is_some());
    if let Some(w) = writer {
        assert_eq!(w.incr("ctr", 5, None).unwrap(), 5);
        assert!(w.set_if_absent("nx", b"v".to_vec(), None).unwrap());
        assert!(!w.set_if_absent("nx", b"w".to_vec(), None).unwrap());
        assert!(
            w.compare_and_swap("nx", Some(b"v".as_slice()), b"v2".to_vec(), None)
                .unwrap()
        );
    }
    SyncCacheWriter::clear(&adapter).unwrap();
    SyncCacheConnector::shutdown(&adapter);
}

// ============================================================================
// trait 默认方法（零覆写替身）
// ============================================================================

#[tokio::test]
async fn reader_defaults_loop_gets_and_empty_keys() {
    let backend = BareMock::new();

    // CacheReader::is_empty 默认实现（len==0 → true）
    assert!(CacheReader::is_empty(backend.as_ref()).await.unwrap());
    CacheWriter::set(
        backend.as_ref(),
        Arc::from("d"),
        Arc::new(b"1".to_vec()),
        None,
    )
    .await
    .unwrap();
    assert!(!CacheReader::is_empty(backend.as_ref()).await.unwrap());

    // CacheReader::get_many 默认实现（逐 key get 循环）
    CacheWriter::set(
        backend.as_ref(),
        Arc::from("e"),
        Arc::new(b"2".to_vec()),
        None,
    )
    .await
    .unwrap();
    let many = CacheReader::get_many(
        backend.as_ref(),
        &["d".to_string(), "miss".to_string(), "e".to_string()],
    )
    .await
    .unwrap();
    assert_eq!(many[0].as_deref(), Some(&b"1"[..]));
    assert_eq!(many[1], None);
    assert_eq!(many[2].as_deref(), Some(&b"2"[..]));

    // CacheReader::keys 默认实现（空向量）
    assert!(
        CacheReader::keys(backend.as_ref(), "*")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn connector_default_probes_return_none() {
    let backend = BareMock::new();
    // CacheConnector::as_lua_executor / as_atomic_writer 默认 None
    #[cfg(feature = "lua")]
    assert!(CacheConnector::as_lua_executor(backend.as_ref()).is_none());
    assert!(CacheConnector::as_atomic_writer(backend.as_ref()).is_none());
    // SyncCacheConnector::as_sync_atomic_writer 默认 None
    assert!(SyncCacheConnector::as_sync_atomic_writer(backend.as_ref()).is_none());
    // 对象安全探针（经 dyn 分发）
    #[cfg(feature = "lua")]
    fn takes_lua(_: Option<&dyn LuaExecutor>) {}
    #[cfg(feature = "lua")]
    takes_lua(CacheConnector::as_lua_executor(backend.as_ref()));
}

#[test]
fn bare_mock_via_sync_backend_trait_object() {
    // blanket impl：三 sync 子 trait 齐备即得 SyncCacheBackend
    let backend = BareMock::new();
    let object: Arc<dyn SyncCacheBackend> = backend;
    assert_eq!(SyncCacheReader::get(object.as_ref(), "n").unwrap(), None);
    SyncCacheWriter::set(
        object.as_ref(),
        Arc::from("n"),
        Arc::new(b"m".to_vec()),
        None,
    )
    .unwrap();
    assert_eq!(
        SyncCacheReader::get(object.as_ref(), "n")
            .unwrap()
            .as_deref(),
        Some(&b"m"[..])
    );
    SyncCacheConnector::shutdown(object.as_ref());
}

// ============================================================================
// glob_match
// ============================================================================

#[test]
fn glob_match_patterns() {
    // 精确匹配
    assert!(glob_match("abc", "abc"));
    assert!(!glob_match("abc", "and"));
    // '*' 任意串（含路径分隔符）
    assert!(glob_match("*", "anything/here"));
    assert!(glob_match("a*c", "abc"));
    assert!(glob_match("a*c", "ac"));
    assert!(glob_match("a*c", "aXXbYYc"));
    assert!(!glob_match("a*c", "acbX"));
    // '?' 单字符
    assert!(glob_match("a?c", "abc"));
    assert!(!glob_match("a?c", "ac"));
    assert!(!glob_match("a?c", "abbc"));
    // '*' 回溯
    assert!(glob_match("*c", "aabcc"));
    assert!(glob_match("a*a*b", "axxayyb"));
    assert!(!glob_match("a*a*b", "axbyya"));
    assert!(!glob_match("a*a*b", "axxbyyb"));
    // 多 '*' 与空串
    assert!(glob_match("a**b", "ab"));
    assert!(glob_match("", ""));
    assert!(!glob_match("", "x"));
    assert!(glob_match("*", ""));
    // 尾部 '*'：可匹配空尾
    assert!(glob_match("abc*", "abcXYZ"));
    assert!(glob_match("abc*", "abc"));
    // 中段 '*' 与 '?' 组合
    assert!(glob_match("a*?d", "abccd"));
    assert!(!glob_match("a*?d", "ad"));
}

// ============================================================================
// BackendKind 判别面
// ============================================================================

#[test]
fn backend_kind_classification_and_names() {
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
    for (kind, name) in cases {
        assert_eq!(kind.name(), name);
    }

    assert!(BackendKind::Moka.is_memory());
    assert!(BackendKind::DashMap.is_memory());
    assert!(BackendKind::Mock.is_memory());
    assert!(!BackendKind::Redis.is_memory());
    assert!(BackendKind::Redis.is_distributed());
    assert!(BackendKind::Valkey.is_distributed());
    assert!(BackendKind::Dragonfly.is_distributed());
    assert!(BackendKind::Aerospike.is_distributed());
    assert!(!BackendKind::Disk.is_distributed());
    assert!(BackendKind::Chain.is_composite());
    assert!(!BackendKind::Moka.is_composite());
}

// 健康检查失败分支（显性化错误路径）
#[test]
fn sync_connector_error_propagation() {
    struct SickMock;
    impl SyncCacheConnector for SickMock {
        fn health_check(&self) -> OxCacheResult<()> {
            Err(OxCacheError::Connection("sick".to_string()))
        }
        fn shutdown(&self) {}
        fn backend_kind(&self) -> BackendKind {
            BackendKind::Unknown
        }
    }
    let err = SyncCacheConnector::health_check(&SickMock).unwrap_err();
    assert!(err.to_string().contains("sick"));
}

// ============================================================================
// AsyncToSyncBridge：async 后端 → 同步门面（多线程 runtime 内 block_in_place）
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn bridge_sync_surface_over_async_backend() {
    use oxcache::backend::{AsyncToSyncBridge, SyncAtomicCacheWriter};

    let bridge = AsyncToSyncBridge::new(BareMock::with_atomic());

    // reader 面
    assert_eq!(SyncCacheReader::get(&bridge, "b").unwrap(), None);
    assert!(!SyncCacheReader::exists(&bridge, "b").unwrap());
    assert_eq!(SyncCacheReader::ttl(&bridge, "b").unwrap(), None);
    assert_eq!(SyncCacheReader::len(&bridge).unwrap(), 0);
    assert_eq!(SyncCacheReader::capacity(&bridge).unwrap(), 0);
    assert!(SyncCacheReader::stats(&bridge).unwrap().is_empty());

    // writer 面
    SyncCacheWriter::set(&bridge, Arc::from("b"), Arc::new(b"vb".to_vec()), None).unwrap();
    assert_eq!(
        SyncCacheReader::get(&bridge, "b").unwrap().as_deref(),
        Some(&b"vb"[..])
    );
    assert!(SyncCacheReader::exists(&bridge, "b").unwrap());
    assert!(SyncCacheWriter::expire(&bridge, "b", Duration::from_secs(5)).unwrap());

    // reader 默认实现：get_many 逐 key 循环 / keys 默认空
    let many = SyncCacheReader::get_many(&bridge, &["b".to_string(), "x".to_string()]).unwrap();
    assert_eq!(many[0].as_deref(), Some(&b"vb"[..]));
    assert_eq!(many[1], None);
    assert!(SyncCacheReader::keys(&bridge, "*").unwrap().is_empty());
    assert!(!SyncCacheReader::is_empty(&bridge).unwrap());

    // writer 默认实现：set_many 逐条 set / delete_many 逐条 delete
    SyncCacheWriter::set_many(&bridge, &[(Arc::from("c"), Arc::new(b"vc".to_vec()), None)])
        .unwrap();
    assert_eq!(
        SyncCacheReader::get(&bridge, "c").unwrap().as_deref(),
        Some(&b"vc"[..])
    );
    SyncCacheWriter::delete_many(&bridge, &["c".to_string()]).unwrap();
    assert!(!SyncCacheReader::exists(&bridge, "c").unwrap());

    // 原子面：inner 有 async 原子能力 → 探针 Some 且委托成功
    let atomic = SyncCacheConnector::as_sync_atomic_writer(&bridge);
    assert!(atomic.is_some());
    assert_eq!(
        SyncAtomicCacheWriter::incr(&bridge, "br", 9, None).unwrap(),
        9
    );
    assert!(SyncAtomicCacheWriter::set_if_absent(&bridge, "bnx", b"v".to_vec(), None).unwrap());
    assert!(
        SyncAtomicCacheWriter::compare_and_swap(
            &bridge,
            "bnx",
            Some(b"v".as_slice()),
            b"v2".to_vec(),
            None
        )
        .unwrap()
    );

    // connector 面（多线程分支 shutdown）
    SyncCacheConnector::health_check(&bridge).unwrap();
    assert_eq!(
        SyncCacheConnector::backend_kind(&bridge),
        BackendKind::Unknown
    );
    SyncCacheConnector::shutdown(&bridge);

    SyncCacheWriter::delete(&bridge, "b").unwrap();
    SyncCacheWriter::clear(&bridge).unwrap();
    assert!(SyncCacheReader::is_empty(&bridge).unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_atomic_without_inner_capability_falls_back_to_not_supported() {
    use oxcache::backend::{AsyncToSyncBridge, SyncAtomicCacheWriter};

    // inner 无 async 原子能力 → 探针 None，调用路径显性 NotSupported
    let bridge = AsyncToSyncBridge::new(BareMock::new());
    assert!(SyncCacheConnector::as_sync_atomic_writer(&bridge).is_none());
    let err = SyncAtomicCacheWriter::incr(&bridge, "k", 1, None).unwrap_err();
    assert!(!err.to_string().is_empty());
    let err = SyncAtomicCacheWriter::compare_and_swap(&bridge, "k", None, b"v".to_vec(), None)
        .unwrap_err();
    assert!(!err.to_string().is_empty());
    let err = SyncAtomicCacheWriter::set_if_absent(&bridge, "k", b"v".to_vec(), None).unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[test]
fn bridge_shutdown_outside_runtime_drives_temp_runtime() {
    use oxcache::backend::AsyncToSyncBridge;

    // runtime 之外：临时 current_thread runtime 驱动真实 shutdown（不静默跳过）
    let bridge = AsyncToSyncBridge::new(BareMock::new());
    SyncCacheConnector::shutdown(&bridge);
}

#[tokio::test]
async fn bridge_shutdown_inside_current_thread_runtime_is_rejected_observably() {
    use oxcache::backend::AsyncToSyncBridge;

    // current_thread 异步上下文：嵌套阻塞驱动被禁止 → 拒绝必须可观测（计数器）
    let bridge = AsyncToSyncBridge::new(BareMock::new());
    SyncCacheConnector::shutdown(&bridge);
}

// ============================================================================
// SyncBackendAdapter 同步面补全（exists/ttl/capacity/stats/set_many/delete_many）
// ============================================================================

#[test]
fn sync_backend_adapter_sync_face_rest_methods() {
    let (backend, _state) = FullMock::new(BackendKind::Mock);
    let adapter = SyncBackendAdapter::new(backend);

    assert!(!SyncCacheReader::exists(&adapter, "r").unwrap());
    SyncCacheWriter::set(&adapter, Arc::from("r"), Arc::new(b"1".to_vec()), None).unwrap();
    assert!(SyncCacheReader::exists(&adapter, "r").unwrap());
    assert_eq!(SyncCacheReader::ttl(&adapter, "r").unwrap(), None);
    assert_eq!(SyncCacheReader::capacity(&adapter).unwrap(), 1024);
    let stats = SyncCacheReader::stats(&adapter).unwrap();
    assert_eq!(stats.get("kind").map(String::as_str), Some("full"));

    SyncCacheWriter::set_many(
        &adapter,
        &[(Arc::from("r2"), Arc::new(b"2".to_vec()), None)],
    )
    .unwrap();
    assert!(SyncCacheReader::exists(&adapter, "r2").unwrap());
    assert!(SyncCacheWriter::expire(&adapter, "r2", Duration::from_secs(3)).unwrap());
    SyncCacheWriter::delete_many(&adapter, &["r2".to_string()]).unwrap();
    assert!(!SyncCacheReader::exists(&adapter, "r2").unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn adapter_atomic_without_inner_sync_atomic_falls_back() {
    use oxcache::backend::AtomicCacheWriter;

    // inner（BareMock）无同步原子面 → adapter 探针 None，直接调用走 NotSupported 分支
    let adapter = SyncBackendAdapter::new(BareMock::new());
    assert!(CacheConnector::as_atomic_writer(&adapter).is_none());
    let err = AtomicCacheWriter::incr(&adapter, "k", 1, None)
        .await
        .unwrap_err();
    assert!(!err.to_string().is_empty());
}
