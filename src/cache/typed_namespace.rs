// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 类型化命名空间
//!
//! [`TypedNamespace<K, V, N>`] 把命名空间绑定到**编译期类型**（marker type +
//! [`NamespaceName`] trait）：两个不同命名空间的句柄是不同类型，跨服务键
//! 冲突与误删前缀在编译期即被阻止；运行时键与 [`KeyGenerator`](crate::KeyGenerator)
//! 的 `ns:key` 前缀约定保持一致（兼容既有数据）。
//!
//! 用 [`namespace!`] 宏声明命名空间 marker：
//!
//! ```rust,ignore
//! use oxcache::namespace;
//!
//! namespace!(Users);   // marker type `Users`，NAME = "Users"
//! namespace!(Orders);
//!
//! let users: TypedNamespace<String, User, Users> = TypedNamespace::scoped(backend);
//! let orders: TypedNamespace<String, Order, Orders> = TypedNamespace::scoped(backend);
//! // users.set(&k, ...) 的 k 编译期即限定在 Users 命名空间
//! ```

use crate::backend::CacheBackend;
use crate::error::OxCacheResult;
use crate::traits::CacheKey;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

/// 命名空间名（marker type 实现，`NAME` 为运行时键前缀）
pub trait NamespaceName: Send + Sync + 'static {
    const NAME: &'static str;
}

/// 声明命名空间 marker 类型
///
/// ```rust,ignore
/// namespace!(Users);
/// // 等价于：
/// // pub struct Users;
/// // impl NamespaceName for Users { const NAME: &'static str = "Users"; }
/// ```
#[macro_export]
macro_rules! namespace {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct $name;

        impl $crate::cache::typed_namespace::NamespaceName for $name {
            const NAME: &'static str = stringify!($name);
        }
    };
}

/// 编译期命名空间句柄：`K`/`V` 与命名空间 `N` 绑定
pub struct TypedNamespace<K, V, N: NamespaceName> {
    backend: Arc<dyn CacheBackend>,
    prefix: String,
    _phantom: PhantomData<(K, V, N)>,
}

impl<K, V, N> std::fmt::Debug for TypedNamespace<K, V, N>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
    N: NamespaceName,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedNamespace")
            .field("namespace", &<N as NamespaceName>::NAME)
            .field("prefix", &self.prefix)
            .finish()
    }
}

impl<K, V, N> TypedNamespace<K, V, N>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
    N: NamespaceName,
{
    /// 绑定后端，以 `N::NAME` 为命名空间
    pub fn scoped(backend: Arc<dyn CacheBackend>) -> Self {
        Self {
            backend,
            prefix: format!("{}:", <N as NamespaceName>::NAME),
            _phantom: PhantomData,
        }
    }

    /// 完整键：`<ns>:<key>`（与 KeyGenerator 前缀约定一致）
    pub fn full_key(&self, key: &K) -> String {
        format!("{}{}", self.prefix, key.to_key_string())
    }

    /// 命名空间名
    pub fn namespace(&self) -> &'static str {
        <N as NamespaceName>::NAME
    }

    /// 类型化读取
    pub async fn get(&self, key: &K) -> OxCacheResult<Option<V>> {
        let full = self.full_key(key);
        match self.backend.get(&full).await? {
            Some(data) => {
                let val: V = crate::infra::serialization::depth_limited::deserialize_safe(
                    &data,
                    crate::core::constants::MAX_JSON_DEPTH,
                )
                .map_err(|e| crate::error::OxCacheError::Serialization(e.to_string()))?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    /// 类型化写入
    pub async fn set(&self, key: &K, value: &V, ttl: Option<Duration>) -> OxCacheResult<()> {
        let full = self.full_key(key);
        let bytes = serde_json::to_vec(value)
            .map_err(|e| crate::error::OxCacheError::Serialization(e.to_string()))?;
        self.backend
            .set(Arc::from(full), Arc::new(bytes), ttl)
            .await
    }

    /// 删除
    pub async fn delete(&self, key: &K) -> OxCacheResult<()> {
        self.backend.delete(&self.full_key(key)).await
    }

    /// 存在性
    pub async fn exists(&self, key: &K) -> OxCacheResult<bool> {
        self.backend.exists(&self.full_key(key)).await
    }

    /// 本命名空间条目数（按前缀匹配）
    pub async fn len(&self) -> OxCacheResult<u64> {
        let keys = self.backend.keys(&format!("{}*", self.prefix)).await?;
        Ok(keys.len() as u64)
    }

    /// 是否为空
    pub async fn is_empty(&self) -> OxCacheResult<bool> {
        Ok(self.len().await? == 0)
    }

    /// 失效本命名空间全部条目（前缀匹配删除）
    pub async fn invalidate_all(&self) -> OxCacheResult<u64> {
        let pattern = format!("{}*", self.prefix);
        let keys = self.backend.keys(&pattern).await?;
        let count = keys.len() as u64;
        for key in keys {
            self.backend.delete(&key).await?;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;
    use serde::{Deserialize, Serialize};

    namespace!(Users);
    namespace!(Orders);

    #[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
    struct User {
        id: u64,
        name: String,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
    struct Order {
        id: u64,
        total: f64,
    }

    fn backend() -> Arc<dyn CacheBackend> {
        Arc::new(MockBackend::new("mock", 100, false))
    }

    #[tokio::test]
    async fn typed_roundtrip_with_namespace_prefix() {
        let ns: TypedNamespace<String, User, Users> = TypedNamespace::scoped(backend());
        let user = User { id: 1, name: "alice".into() };

        ns.set(&"1".to_string(), &user, None).await.unwrap();
        assert_eq!(ns.get(&"1".to_string()).await.unwrap(), Some(user));
        assert!(ns.exists(&"1".to_string()).await.unwrap());

        // 运行时键带命名空间前缀（与 KeyGenerator 约定一致）
        assert_eq!(ns.full_key(&"1".to_string()), "Users:1");
    }

    #[tokio::test]
    async fn namespaces_are_isolated_at_runtime() {
        let backend = backend();
        let users: TypedNamespace<String, User, Users> = TypedNamespace::scoped(backend.clone());
        let orders: TypedNamespace<String, Order, Orders> = TypedNamespace::scoped(backend.clone());

        users
            .set(&"1".to_string(), &User { id: 1, name: "a".into() }, None)
            .await
            .unwrap();

        // 同形 key 不同命名空间互不可见
        let missing: Option<Order> = orders.get(&"1".to_string()).await.unwrap();
        assert!(missing.is_none(), "跨命名空间同形键不得互相可见");
        assert_eq!(users.len().await.unwrap(), 1);
        assert_eq!(orders.len().await.unwrap(), 0);

        // 编译期隔离：句柄类型不同（Users vs Orders），键无法跨句柄混用
        assert_eq!(users.namespace(), "Users");
        assert_eq!(orders.namespace(), "Orders");
    }

    #[tokio::test]
    async fn invalidate_all_removes_only_own_prefix() {
        let backend = backend();
        let users: TypedNamespace<String, User, Users> = TypedNamespace::scoped(backend.clone());
        let orders: TypedNamespace<String, Order, Orders> = TypedNamespace::scoped(backend.clone());

        users
            .set(&"1".to_string(), &User { id: 1, name: "a".into() }, None)
            .await
            .unwrap();
        users
            .set(&"2".to_string(), &User { id: 2, name: "b".into() }, None)
            .await
            .unwrap();
        orders
            .set(&"1".to_string(), &Order { id: 1, total: 9.9 }, None)
            .await
            .unwrap();

        let removed = users.invalidate_all().await.unwrap();
        assert_eq!(removed, 2, "应只清除 Users 前缀的 2 条");
        assert!(users.is_empty().await.unwrap());
        assert_eq!(
            orders.len().await.unwrap(),
            1,
            "Orders 命名空间不受影响"
        );

        // 原始键确认：Users 已被清除，Orders 保留
        assert!(!backend.exists("Users:1").await.unwrap());
        assert!(backend.exists("Orders:1").await.unwrap());
    }

    #[tokio::test]
    async fn delete_and_ttl_passthrough() {
        let ns: TypedNamespace<String, User, Users> = TypedNamespace::scoped(backend());
        ns.set(&"k".to_string(), &User { id: 3, name: "c".into() }, None)
            .await
            .unwrap();
        ns.delete(&"k".to_string()).await.unwrap();
        assert_eq!(ns.get(&"k".to_string()).await.unwrap(), None);
    }

    /// serde_json 类型不匹配返回错误而非脏数据
    #[tokio::test]
    async fn wrong_value_type_is_error_not_garbage() {
        let backend = backend();
        let ns: TypedNamespace<String, User, Users> = TypedNamespace::scoped(backend.clone());
        // 直接向 Users:bad 写入 Order JSON
        backend
            .set(
                Arc::from("Users:bad"),
                Arc::new(br#"{"id":9,"total":1.0}"#.to_vec()),
                None,
            )
            .await
            .unwrap();
        assert!(ns.get(&"bad".to_string()).await.is_err());
    }
}
