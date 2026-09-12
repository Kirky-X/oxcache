// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 后端工厂注册中心
//!
//! [`BackendRegistry`] 按名注册/构建后端：字符串驱动的 `kind → factory`
//! 映射，供 kit / sdforge / 配置层**动态选择**后端（替代已删 cli 的管理面
//! 入口，架构决策见 change design D4）。各 feature 在全局注册中心自动
//! 注册自己的工厂（无 feature 时对应工厂不存在）。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::backend::factory::{BackendRegistry, BackendSpec};
//!
//! let spec = BackendSpec { kind: "moka".into(), capacity: 1000, ..Default::default() };
//! let backend = BackendRegistry::global().build(&spec).await?;
//!
//! // 自定义工厂
//! let registry = BackendRegistry::new();
//! registry.register("valkey", |spec| Box::pin(async { ... }));
//! ```

use crate::backend::CacheBackend;
use crate::error::{OxCacheError, OxCacheResult};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 后端构建描述（字符串驱动，serde 友好）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(
    any(feature = "serialization", feature = "full"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct BackendSpec {
    /// 后端类型名（注册中心查找键）
    pub kind: String,
    /// 容量（条目数；内存后端消费）
    #[cfg_attr(any(feature = "serialization", feature = "full"), serde(default))]
    pub capacity: u64,
    /// 默认 TTL（毫秒；0 = 使用后端默认）
    #[cfg_attr(any(feature = "serialization", feature = "full"), serde(default))]
    pub default_ttl_ms: u64,
    /// 连接串（redis/valkey/dragonfly 等）
    #[cfg_attr(any(feature = "serialization", feature = "full"), serde(default))]
    pub url: Option<String>,
}

impl BackendSpec {
    /// 指定 kind 的最小 spec
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            ..Default::default()
        }
    }

    /// 设置容量
    pub fn with_capacity(mut self, capacity: u64) -> Self {
        self.capacity = capacity;
        self
    }

    /// 设置连接串
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }
}

/// 后端工厂（异步构建，trait object 可注册）
#[async_trait::async_trait]
pub trait BackendFactory: Send + Sync {
    /// 按 spec 构建后端实例
    async fn build(&self, spec: &BackendSpec) -> OxCacheResult<Arc<dyn CacheBackend>>;
}

type BoxFutureBuild =
    std::pin::Pin<Box<dyn std::future::Future<Output = OxCacheResult<Arc<dyn CacheBackend>>> + Send>>;

/// 函数式工厂便捷包装
pub struct FnFactory<F>(pub F);

#[async_trait::async_trait]
impl<F> BackendFactory for FnFactory<F>
where
    F: Fn(&BackendSpec) -> BoxFutureBuild + Send + Sync,
{
    async fn build(&self, spec: &BackendSpec) -> OxCacheResult<Arc<dyn CacheBackend>> {
        (self.0)(spec).await
    }
}

/// 后端工厂注册中心
#[derive(Default)]
pub struct BackendRegistry {
    factories: RwLock<HashMap<String, Arc<dyn BackendFactory>>>,
}

impl BackendRegistry {
    /// 空注册中心
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册（或覆盖）kind 对应工厂
    pub fn register(
        &self,
        kind: impl Into<String>,
        factory: Arc<dyn BackendFactory>,
    ) -> &Self {
        // 与 build/registered 同口径：锁中毒时恢复数据继续写入，
        // 避免注册静默丢失后 build 报出误导性的 unknown kind
        self.factories
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(kind.into(), factory);
        self
    }

    /// 注册函数式工厂
    pub fn register_fn<F>(&self, kind: impl Into<String>, f: F) -> &Self
    where
        F: Fn(&BackendSpec) -> BoxFutureBuild + Send + Sync + 'static,
    {
        self.register(kind, Arc::new(FnFactory(f)))
    }

    /// 已注册 kind 列表（排序）
    pub fn registered(&self) -> Vec<String> {
        let mut kinds: Vec<String> = self
            .factories
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .keys()
            .cloned()
            .collect();
        kinds.sort();
        kinds
    }

    /// 按名构建后端
    ///
    /// 未知 kind 报错并附带可用列表（Agent DX：错误即文档）。
    pub async fn build(&self, spec: &BackendSpec) -> OxCacheResult<Arc<dyn CacheBackend>> {
        // 锁中毒时注册表数据本身仍一致，恢复后继续查找，
        // 避免把「已注册」误报成 unknown kind
        let factory = match self.factories.read() {
            Ok(map) => map.get(&spec.kind).cloned(),
            Err(poisoned) => poisoned.into_inner().get(&spec.kind).cloned(),
        };
        match factory {
            Some(factory) => factory.build(spec).await,
            None => Err(OxCacheError::InvalidInput(format!(
                "unknown backend kind '{}'; available: {}",
                spec.kind,
                self.registered().join(", ")
            ))),
        }
    }

    /// 内置工厂（按 feature 注册）
    fn with_builtins() -> Self {
        let registry = Self::new();

        #[cfg(feature = "memory")]
        {
            registry.register_fn("moka", |spec: &BackendSpec| {
                let spec = spec.clone();
                Box::pin(async move {
                    let mut builder =
                        crate::backend::MokaMemoryBackend::builder()
                            .capacity(spec.capacity.max(1));
                    if spec.default_ttl_ms > 0 {
                        builder = builder.ttl(std::time::Duration::from_millis(spec.default_ttl_ms));
                    }
                    Ok(Arc::new(builder.build()) as Arc<dyn CacheBackend>)
                })
            });
            registry.register_fn("dashmap", |spec: &BackendSpec| {
                let spec = spec.clone();
                Box::pin(async move {
                    let mut builder = crate::backend::DashMapMemoryBackend::builder();
                    if spec.capacity > 0 {
                        builder = builder.capacity(spec.capacity as usize);
                    }
                    if spec.default_ttl_ms > 0 {
                        builder =
                            builder.default_ttl(std::time::Duration::from_millis(spec.default_ttl_ms));
                    }
                    Ok(Arc::new(builder.build()) as Arc<dyn CacheBackend>)
                })
            });
            // "memory" 作为 moka 的别名
            registry.register_fn("memory", |spec: &BackendSpec| {
                let spec = spec.clone();
                Box::pin(async move {
                    let mut builder =
                        crate::backend::MokaMemoryBackend::builder()
                            .capacity(spec.capacity.max(1));
                    if spec.default_ttl_ms > 0 {
                        builder = builder.ttl(std::time::Duration::from_millis(spec.default_ttl_ms));
                    }
                    Ok(Arc::new(builder.build()) as Arc<dyn CacheBackend>)
                })
            });
        }

        #[cfg(feature = "redis")]
        {
            registry.register_fn("redis", |spec: &BackendSpec| {
                let spec = spec.clone();
                Box::pin(async move {
                    let url = spec
                        .url
                        .clone()
                        .unwrap_or_else(|| "redis://127.0.0.1:6379".to_string());
                    Ok(Arc::new(crate::backend::RedisBackend::new(&url).await?)
                        as Arc<dyn CacheBackend>)
                })
            });
        }

        registry
    }
}

/// 全局默认注册中心（OnceLock，内置工厂按 feature 自动注册）
pub static GLOBAL_BACKEND_REGISTRY: Lazy<BackendRegistry> = Lazy::new(BackendRegistry::with_builtins);

impl BackendRegistry {
    /// 全局默认注册中心
    pub fn global() -> &'static BackendRegistry {
        &GLOBAL_BACKEND_REGISTRY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;

    #[tokio::test]
    async fn builtin_memory_factories_build() {
        let registry = BackendRegistry::global();
        let spec = BackendSpec::new("moka").with_capacity(500);
        let backend = registry.build(&spec).await.unwrap();
        assert_eq!(backend.capacity().await.unwrap(), 500);
        assert_eq!(backend.backend_kind(), crate::backend::BackendKind::Moka);

        // "memory" 别名
        let backend = registry.build(&BackendSpec::new("memory")).await.unwrap();
        assert_eq!(backend.backend_kind(), crate::backend::BackendKind::Moka);

        let backend = registry
            .build(&BackendSpec::new("dashmap").with_capacity(64))
            .await
            .unwrap();
        assert_eq!(backend.backend_kind(), crate::backend::BackendKind::DashMap);
        assert_eq!(backend.capacity().await.unwrap(), 64);
    }

    #[tokio::test]
    async fn unknown_kind_error_lists_available() {
        let registry = BackendRegistry::global();
        let err = match registry.build(&BackendSpec::new("nosuch-backend")).await {
            Err(e) => e,
            Ok(_) => panic!("未知 kind 必须报错"),
        };
        let msg = err.to_string();
        assert!(msg.contains("nosuch-backend"), "msg: {msg}");
        assert!(msg.contains("available:"), "错误应附带可用列表: {msg}");
    }

    #[tokio::test]
    async fn custom_factory_registration_and_lookup() {
        let registry = BackendRegistry::new();
        registry.register_fn("mock:test", |spec: &BackendSpec| {
            let spec = spec.clone();
            Box::pin(async move {
                Ok(Arc::new(MockBackend::new(
                    "mock-test",
                    spec.capacity.min(255) as u8,
                    false,
                )) as Arc<dyn CacheBackend>)
            })
        });

        assert_eq!(registry.registered(), vec!["mock:test".to_string()]);
        let backend = registry
            .build(&BackendSpec::new("mock:test").with_capacity(7))
            .await
            .unwrap();
        assert!(backend.exists("nothing").await.unwrap().eq(&false));

        // 覆盖注册
        registry.register_fn("mock:test", |_spec| {
            Box::pin(async move {
                Ok(Arc::new(MockBackend::new("mock-2", 10, true))
                    as Arc<dyn CacheBackend>)
            })
        });
        let backend = registry.build(&BackendSpec::new("mock:test")).await.unwrap();
        assert_eq!(backend.stats().await.unwrap().get("type").map(String::as_str), Some("mock-2"));
    }

    #[tokio::test]
    async fn empty_registry_reports_empty_available_list() {
        let registry = BackendRegistry::new();
        let err = match registry.build(&BackendSpec::new("anything")).await {
            Err(e) => e,
            Ok(_) => panic!("空注册中心必须报错"),
        };
        assert!(err.to_string().contains("available: "), "got {err}");
    }

    #[test]
    fn spec_serde_roundtrip() {
        let spec = BackendSpec::new("moka").with_capacity(100);
        let json = serde_json::to_string(&spec).unwrap();
        let back: BackendSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, spec);
    }
}
