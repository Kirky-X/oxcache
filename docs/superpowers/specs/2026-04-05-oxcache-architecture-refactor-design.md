# Oxcache 架构重构设计文档

**日期**: 2026-04-05
**项目**: oxcache
**类型**: 基础模块架构重构

---

## 1. 概述

本文档定义了 oxcache 项目按照 BrickArchitecture 规范进行的架构重构方案。oxcache 作为基础模块，需要满足以下核心要求：

- **纯基础设施能力**：不携带任何业务语义，仅提供通用缓存功能
- **零内部依赖**：移除对 confers 的依赖，只保留第三方基础库依赖
- **trait 契约对外**：通过 trait 暴露契约，impl\_/ 下的具体实现对外不可见
- **必须提供内存实现**：`new_in_memory()` 供功能模块和测试使用

---

## 2. 目标目录结构

```
oxcache/
├── src/
│   ├── lib.rs              # 入口：pub use 控制对外可见性
│   ├── config.rs           # 配置结构体 + CacheConfigError
│   ├── error.rs            # 运行时错误 CacheError
│   ├── interface.rs        # 对外 trait 定义
│   ├── types.rs            # 公共数据类型
│   ├── health.rs           # 健康检查相关
│   ├── lifecycle.rs        # 生命周期管理
│   └── impl_/              # 具体实现（不对外暴露）
│       ├── mod.rs
│       ├── memory/
│       │   ├── mod.rs
│       │   ├── moka.rs
│       │   └── dashmap.rs
│       ├── redis/
│       │   ├── mod.rs
│       │   └── client.rs
│       ├── tiered/
│       │   ├── mod.rs
│       │   ├── custom_tiered.rs
│       │   └── score.rs
│       ├── database/
│       │   ├── mod.rs
│       │   ├── common.rs
│       │   ├── sqlite.rs
│       │   ├── connection_string.rs
│       │   └── partition/
│       │       └── mod.rs
│       ├── serialization/
│       │   ├── mod.rs
│       │   ├── json.rs
│       │   ├── bincode.rs
│       │   ├── unified.rs
│       │   ├── cache.rs
│       │   ├── depth_limited.rs
│       │   ├── extra.rs
│       │   └── utils.rs
│       ├── metrics/
│       │   ├── mod.rs
│       │   ├── backend.rs
│       │   └── unified.rs
│       ├── recovery/
│       │   ├── mod.rs
│       │   └── wal.rs
│       ├── sync/
│       │   ├── mod.rs
│       │   └── warmup.rs
│       ├── security/
│       │   ├── mod.rs
│       │   ├── validation.rs
│       │   ├── redaction.rs
│       │   ├── log.rs
│       │   └── regex.rs
│       ├── http/
│       │   ├── mod.rs
│       │   └── axum.rs
│       ├── builder/
│       │   ├── mod.rs
│       │   ├── cache_builder.rs
│       │   ├── oxcache_builder.rs
│       │   └── sorter.rs
│       ├── client/
│       │   ├── mod.rs
│       │   └── db_loader.rs
│       ├── utils/
│       │   ├── mod.rs
│       │   └── key_generator.rs
│       ├── chain.rs
│       ├── bloom_filter.rs
│       ├── rate_limiting.rs
│       ├── smart_strategy.rs
│       ├── telemetry.rs
│       ├── circuit_breaker.rs
│       ├── singleflight.rs
│       ├── internal.rs
│       ├── constants.rs
│       ├── features.rs
│       ├── events.rs
│       ├── mock.rs
│       └── testing/
│           ├── mod.rs
│           └── precommit.rs
└── Cargo.toml
```

### 2.1 对外暴露文件

| 文件           | 职责                         | 对外可见 |
| -------------- | ---------------------------- | -------- |
| `lib.rs`       | 统一 pub use，模块对外入口   | —        |
| `config.rs`    | 配置结构体定义与语义校验     | ✅       |
| `error.rs`     | 运行时错误类型               | ✅       |
| `interface.rs` | trait 定义，唯一对外契约     | ✅       |
| `types.rs`     | 公共数据类型（枚举、结构体） | ✅       |
| `health.rs`    | 健康检查辅助功能             | ✅       |
| `lifecycle.rs` | 生命周期管理工具             | ✅       |

### 2.2 内部实现模块（impl\_/）

| 目录/文件                 | Feature Flag      | 说明           |
| ------------------------- | ----------------- | -------------- |
| `impl_/memory/`           | `moka`, `dashmap` | 内存后端实现   |
| `impl_/redis/`            | `redis`           | Redis 后端实现 |
| `impl_/tiered/`           | 默认              | 分层后端实现   |
| `impl_/database/`         | `database`        | 数据库适配器   |
| `impl_/serialization/`    | `serialization`   | 序列化支持     |
| `impl_/metrics/`          | `metrics`         | 指标收集       |
| `impl_/recovery/`         | `wal-recovery`    | WAL 恢复       |
| `impl_/sync/`             | `sync`            | 同步工具       |
| `impl_/security/`         | `redis`           | 安全工具       |
| `impl_/http/`             | `http-cache`      | HTTP 缓存      |
| `impl_/builder/`          | 默认              | Cache 构建器   |
| `impl_/bloom_filter.rs`   | `bloom-filter`    | 布隆过滤器     |
| `impl_/rate_limiting.rs`  | `rate-limiting`   | 限流器         |
| `impl_/smart_strategy.rs` | `smart-strategy`  | 智能策略       |
| `impl_/telemetry.rs`      | `opentelemetry`   | 遥测支持       |
| `impl_/mock.rs`           | `testing`         | Mock 实现      |

---

## 3. 文件迁移计划

### 3.1 核心对外文件

| 现有文件                   | 目标位置            | 说明              |
| -------------------------- | ------------------- | ----------------- |
| `src/backend/interface.rs` | `src/interface.rs`  | 重新设计 trait    |
| `src/cache/interface.rs`   | 合并到上述文件      | UnifiedCache 拆分 |
| `src/core/types.rs`        | `src/types.rs`      | 整合公共类型      |
| `src/traits/cacheable.rs`  | 合并到 interface.rs | Cacheable trait   |
| `src/traits/cache_key.rs`  | 合并到 interface.rs | CacheKey trait    |
| `src/error.rs`             | `src/error.rs`      | 拆出配置错误      |
| 新建                       | `src/config.rs`     | 配置结构体 + 校验 |
| 新建                       | `src/health.rs`     | 健康检查          |
| 新建                       | `src/lifecycle.rs`  | 生命周期管理      |

### 3.2 内存后端 → impl\_/memory/

| 现有文件                                | 目标位置                      |
| --------------------------------------- | ----------------------------- |
| `src/backend/client/moka/backend.rs`    | `src/impl_/memory/moka.rs`    |
| `src/backend/client/moka/mod.rs`        | 合并到上述文件                |
| `src/backend/client/dashmap/backend.rs` | `src/impl_/memory/dashmap.rs` |
| `src/backend/client/dashmap/mod.rs`     | 合并到上述文件                |

### 3.3 Redis 后端 → impl\_/redis/

| 现有文件                             | 目标位置                    |
| ------------------------------------ | --------------------------- |
| `src/backend/client/redis/mod.rs`    | `src/impl_/redis/mod.rs`    |
| `src/backend/client/redis/client.rs` | `src/impl_/redis/client.rs` |

### 3.4 分层后端 → impl\_/tiered/

| 现有文件                       | 目标位置                            |
| ------------------------------ | ----------------------------------- |
| `src/backend/custom_tiered.rs` | `src/impl_/tiered/custom_tiered.rs` |
| `src/backend/score.rs`         | `src/impl_/tiered/score.rs`         |
| `src/backend/mod.rs`           | `src/impl_/tiered/mod.rs`           |

### 3.5 数据库模块 → impl\_/database/

| 现有文件                            | 目标位置                                  |
| ----------------------------------- | ----------------------------------------- |
| `src/database/mod.rs`               | `src/impl_/database/mod.rs`               |
| `src/database/common.rs`            | `src/impl_/database/common.rs`            |
| `src/database/sqlite.rs`            | `src/impl_/database/sqlite.rs`            |
| `src/database/connection_string.rs` | `src/impl_/database/connection_string.rs` |
| `src/database/partition/mod.rs`     | `src/impl_/database/partition/mod.rs`     |

> 注：MySQL/PostgreSQL 适配器已在之前版本移除，不参与本次迁移。

### 3.6 序列化模块 → impl\_/serialization/

| 现有文件                             | 目标位置                                   |
| ------------------------------------ | ------------------------------------------ |
| `src/serialization/mod.rs`           | `src/impl_/serialization/mod.rs`           |
| `src/serialization/json.rs`          | `src/impl_/serialization/json.rs`          |
| `src/serialization/bincode.rs`       | `src/impl_/serialization/bincode.rs`       |
| `src/serialization/unified.rs`       | `src/impl_/serialization/unified.rs`       |
| `src/serialization/cache.rs`         | `src/impl_/serialization/cache.rs`         |
| `src/serialization/depth_limited.rs` | `src/impl_/serialization/depth_limited.rs` |
| `src/serialization/extra.rs`         | `src/impl_/serialization/extra.rs`         |
| `src/serialization/utils.rs`         | `src/impl_/serialization/utils.rs`         |

### 3.7 指标模块 → impl\_/metrics/

| 现有文件                 | 目标位置                       |
| ------------------------ | ------------------------------ |
| `src/metrics/mod.rs`     | `src/impl_/metrics/mod.rs`     |
| `src/metrics/backend.rs` | `src/impl_/metrics/backend.rs` |
| `src/metrics/unified.rs` | `src/impl_/metrics/unified.rs` |

### 3.8 安全模块 → impl\_/security/

| 现有文件                     | 目标位置                           |
| ---------------------------- | ---------------------------------- |
| `src/security/mod.rs`        | `src/impl_/security/mod.rs`        |
| `src/security/validation.rs` | `src/impl_/security/validation.rs` |
| `src/security/redaction.rs`  | `src/impl_/security/redaction.rs`  |
| `src/security/log.rs`        | `src/impl_/security/log.rs`        |
| `src/security/regex.rs`      | `src/impl_/security/regex.rs`      |

### 3.9 恢复与同步 → impl*/recovery/, impl*/sync/

| 现有文件              | 目标位置                    |
| --------------------- | --------------------------- |
| `src/recovery/mod.rs` | `src/impl_/recovery/mod.rs` |
| `src/recovery/wal.rs` | `src/impl_/recovery/wal.rs` |
| `src/sync/mod.rs`     | `src/impl_/sync/mod.rs`     |
| `src/sync/warmup.rs`  | `src/impl_/sync/warmup.rs`  |

### 3.10 HTTP 缓存 → impl\_/http/

| 现有文件           | 目标位置                 |
| ------------------ | ------------------------ |
| `src/http/mod.rs`  | `src/impl_/http/mod.rs`  |
| `src/http/axum.rs` | `src/impl_/http/axum.rs` |

### 3.11 构建器 → impl\_/builder/

| 现有文件                         | 目标位置                               |
| -------------------------------- | -------------------------------------- |
| `src/builder/mod.rs`             | `src/impl_/builder/mod.rs`             |
| `src/builder/cache_builder.rs`   | `src/impl_/builder/cache_builder.rs`   |
| `src/builder/oxcache_builder.rs` | `src/impl_/builder/oxcache_builder.rs` |
| `src/builder/sorter.rs`          | `src/impl_/builder/sorter.rs`          |

### 3.12 客户端工具 → impl\_/client/

| 现有文件                  | 目标位置                        |
| ------------------------- | ------------------------------- |
| `src/client/mod.rs`       | `src/impl_/client/mod.rs`       |
| `src/client/db_loader.rs` | `src/impl_/client/db_loader.rs` |

### 3.13 工具模块 → impl\_/utils/

| 现有文件                     | 目标位置                           |
| ---------------------------- | ---------------------------------- |
| `src/utils/mod.rs`           | `src/impl_/utils/mod.rs`           |
| `src/utils/key_generator.rs` | `src/impl_/utils/key_generator.rs` |

### 3.14 核心内部模块 → impl\_/

| 现有文件                | 目标位置                 |
| ----------------------- | ------------------------ |
| `src/core/constants.rs` | `src/impl_/constants.rs` |
| `src/core/features.rs`  | `src/impl_/features.rs`  |
| `src/core/events.rs`    | `src/impl_/events.rs`    |

### 3.15 其他功能模块 → impl\_/

| 现有文件                     | 目标位置                       |
| ---------------------------- | ------------------------------ |
| `src/bloom_filter.rs`        | `src/impl_/bloom_filter.rs`    |
| `src/rate_limiting.rs`       | `src/impl_/rate_limiting.rs`   |
| `src/smart_strategy.rs`      | `src/impl_/smart_strategy.rs`  |
| `src/telemetry.rs`           | `src/impl_/telemetry.rs`       |
| `src/circuit_breaker/mod.rs` | `src/impl_/circuit_breaker.rs` |
| `src/singleflight/mod.rs`    | `src/impl_/singleflight.rs`    |
| `src/internal.rs`            | `src/impl_/internal.rs`        |
| `src/cache/chain.rs`         | `src/impl_/chain.rs`           |

### 3.16 测试支持 → impl\_/testing/

| 现有文件                   | 目标位置                         |
| -------------------------- | -------------------------------- |
| `src/testing/mock.rs`      | `src/impl_/mock.rs`              |
| `src/testing/mod.rs`       | `src/impl_/testing/mod.rs`       |
| `src/testing/precommit.rs` | `src/impl_/testing/precommit.rs` |

### 3.17 删除的文件/模块

| 文件/目录                      | 原因                       |
| ------------------------------ | -------------------------- |
| `src/config/mod.rs`            | confers 配置模块，移除依赖 |
| `src/config/confers_config.rs` | confers 集成代码           |
| `src/cli/`                     | CLI 不属于基础模块         |
| `src/backend/client/mod.rs`    | 合并到 impl\_/memory/ 等   |
| `src/cache/mod.rs`             | 整合到顶层                 |
| `src/traits/mod.rs`            | 整合到 interface.rs        |
| `src/core/mod.rs`              | 拆分后不再需要             |

---

## 4. Trait 设计（按层级拆分）

### 4.1 接口定义

```rust
// src/interface.rs

use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;
use crate::error::{CacheError, Result};

/// 缓存键 trait
pub trait CacheKey: Send + Sync {
    fn to_key_string(&self) -> String;
}

/// 可缓存值 trait
pub trait Cacheable: Send + Sync + serde::Serialize + serde::de::DeserializeOwned {}

/// 单层缓存读操作
#[async_trait]
pub trait SimpleCacheReader: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn exists(&self, key: &str) -> Result<bool>;
    async fn ttl(&self, key: &str) -> Result<Option<Duration>>;
}

/// 单层缓存写操作
#[async_trait]
pub trait SimpleCacheWriter: Send + Sync {
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool>;
    async fn clear(&self) -> Result<()>;
}

/// 分层缓存读操作
#[async_trait]
pub trait TieredCacheReader: Send + Sync {
    async fn get_l1(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn get_l2(&self, key: &str) -> Result<Option<Vec<u8>>>;
}

/// 分层缓存写操作
#[async_trait]
pub trait TieredCacheWriter: Send + Sync {
    async fn set_l1(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()>;
    async fn set_l2(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()>;
    async fn clear_l1(&self) -> Result<()>;
    async fn clear_l2(&self) -> Result<()>;
}

/// 组合 trait：简单缓存（单层）
pub trait SimpleCache: SimpleCacheReader + SimpleCacheWriter + Send + Sync {
    async fn health_check(&self) -> anyhow::Result<()>;
    async fn shutdown(&self);
    fn stats(&self) -> HashMap<String, String>;
}

/// 组合 trait：分层缓存
pub trait TieredCache: SimpleCache + TieredCacheReader + TieredCacheWriter {}

/// 批量读操作（可选）
#[async_trait]
pub trait BatchReader: Send + Sync {
    async fn get_many(&self, keys: &[&str]) -> Result<HashMap<String, Vec<u8>>>;
}

/// 批量写操作（可选）
#[async_trait]
pub trait BatchWriter: Send + Sync {
    async fn set_many(&self, items: &[(String, Vec<u8>, Option<Duration>)]) -> Result<()>;
    async fn delete_many(&self, keys: &[&str]) -> Result<()>;
}
```

---

## 5. 错误类型设计

### 5.1 配置错误（config.rs）

```rust
use thiserror::Error;

/// 配置阶段错误：模块初始化时产生
#[derive(Debug, Error)]
pub enum CacheConfigError {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid value for field '{field}': {reason}")]
    InvalidValue { field: String, reason: String },

    #[error("invalid backend type: '{0}'")]
    InvalidBackendType(String),

    #[error("capacity must be greater than 0")]
    ZeroCapacity,

    #[error("capacity {0} exceeds maximum {1}")]
    CapacityExceeded(u64, u64),

    #[error("TTL must be greater than 0")]
    ZeroTtl,

    #[error("TTL {0} seconds exceeds maximum {1} seconds")]
    TtlExceeded(u64, u64),

    #[error("connection string is empty")]
    EmptyConnectionString,
}
```

### 5.2 运行时错误（error.rs）

```rust
use thiserror::Error;

/// 运行时错误：模块使用时产生
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("connection failed: {0}")]
    Connection(String),

    #[error("operation failed: {0}")]
    Operation(String),

    #[error("key not found: {0}")]
    NotFound(String),

    #[error("L1 cache error: {0}")]
    L1Error(String),

    #[error("L2 cache error: {0}")]
    L2Error(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("operation not supported: {0}")]
    NotSupported(String),

    #[error("shutdown error: {0}")]
    ShutdownError(String),

    #[error("pool exhausted: {0}")]
    PoolExhausted(String),
}

pub type Result<T> = std::result::Result<T, CacheError>;
```

---

## 6. 配置结构设计

```rust
// src/config.rs
use serde::Deserialize;
use std::time::Duration;

/// 缓存配置
#[derive(Debug, Clone, Deserialize)]
pub struct CacheConfig {
    #[serde(default)]
    pub backend_type: BackendType,

    #[serde(default)]
    pub l1: Option<LayerConfig>,

    #[serde(default)]
    pub l2: Option<LayerConfig>,

    #[serde(default = "default_ttl")]
    pub default_ttl_secs: u64,
}

/// 单层配置
#[derive(Debug, Clone, Deserialize)]
pub struct LayerConfig {
    pub capacity: u64,

    #[serde(default)]
    pub ttl_secs: Option<u64>,

    #[serde(default)]
    pub connection_string: Option<String>,
}

/// 后端类型
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendType {
    #[default]
    Memory,
    Redis,
    Tiered,
}

fn default_ttl() -> u64 { 3600 }

impl CacheConfig {
    pub fn validate(&self) -> Result<(), CacheConfigError> {
        if let Some(ref l1) = self.l1 {
            l1.validate(Layer::L1)?;
        }
        if let Some(ref l2) = self.l2 {
            l2.validate(Layer::L2)?;
        }
        Ok(())
    }
}

impl LayerConfig {
    const MAX_CAPACITY: u64 = 1_000_000_000;
    const MAX_TTL_SECS: u64 = 30 * 24 * 60 * 60;

    pub fn validate(&self, layer: Layer) -> Result<(), CacheConfigError> {
        if self.capacity == 0 {
            return Err(CacheConfigError::ZeroCapacity);
        }
        if self.capacity > Self::MAX_CAPACITY {
            return Err(CacheConfigError::CapacityExceeded(self.capacity, Self::MAX_CAPACITY));
        }
        if let Some(ttl) = self.ttl_secs {
            if ttl == 0 {
                return Err(CacheConfigError::ZeroTtl);
            }
            if ttl > Self::MAX_TTL_SECS {
                return Err(CacheConfigError::TtlExceeded(ttl, Self::MAX_TTL_SECS));
            }
        }
        Ok(())
    }
}
```

---

## 7. 工厂函数（lib.rs）

```rust
// src/lib.rs

pub use crate::config::{CacheConfig, CacheConfigError, LayerConfig, BackendType};
pub use crate::error::{CacheError, Result};
pub use crate::interface::{
    CacheKey, Cacheable,
    SimpleCache, SimpleCacheReader, SimpleCacheWriter,
    TieredCache, TieredCacheReader, TieredCacheWriter,
    BatchReader, BatchWriter,
};
pub use crate::types::{Layer, CacheStats, CacheEntry};

mod impl_;

/// 创建内存缓存（默认实现）
pub fn new_in_memory() -> impl SimpleCache {
    impl_::memory::moka::MokaBackend::new_default()
}

/// 根据配置创建缓存
pub async fn new(config: CacheConfig) -> Result<impl TieredCache, CacheConfigError> {
    config.validate()?;
    impl_::tiered::TieredBackend::from_config(config).await
}

/// 创建指定容量的内存缓存
pub fn with_capacity(capacity: u64) -> Result<impl SimpleCache, CacheConfigError> {
    impl_::memory::moka::MokaBackend::with_capacity(capacity)
}

/// 创建 Redis 缓存
#[cfg(feature = "redis")]
pub async fn new_redis(url: &str) -> Result<impl SimpleCache, CacheConfigError> {
    impl_::redis::RedisBackend::new(url).await
}

/// 创建分层缓存（L1 + L2）
#[cfg(all(feature = "moka", feature = "redis"))]
pub async fn new_tiered(l1_capacity: u64, redis_url: &str) -> Result<impl TieredCache, CacheConfigError> {
    impl_::tiered::TieredBackend::new(l1_capacity, redis_url).await
}
```

---

## 8. 生命周期管理

### 8.1 健康检查（health.rs）

```rust
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub healthy: bool,
    pub latency_ms: u64,
    pub message: Option<String>,
}

pub async fn check_health<C: SimpleCache>(cache: &C) -> HealthStatus {
    let start = std::time::Instant::now();
    match cache.health_check().await {
        Ok(()) => HealthStatus {
            healthy: true,
            latency_ms: start.elapsed().as_millis() as u64,
            message: None,
        },
        Err(e) => HealthStatus {
            healthy: false,
            latency_ms: start.elapsed().as_millis() as u64,
            message: Some(e.to_string()),
        },
    }
}
```

### 8.2 优雅关闭（lifecycle.rs）

```rust
use std::time::Duration;

pub struct ShutdownConfig {
    pub per_module_timeout: Duration,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self { per_module_timeout: Duration::from_secs(5) }
    }
}

pub async fn shutdown_all(caches: &[&dyn SimpleCache], config: ShutdownConfig) {
    let futures: Vec<_> = caches.iter().map(|c| async {
        let _ = tokio::time::timeout(config.per_module_timeout, c.shutdown()).await;
    }).collect();
    futures::future::join_all(futures).await;
}
```

---

## 9. impl\_/mod.rs 结构

```rust
// src/impl_/mod.rs

pub mod memory;
pub mod tiered;
pub mod builder;
pub mod chain;
pub mod internal;
pub mod constants;
pub mod features;
pub mod events;

#[cfg(feature = "redis")]
pub mod redis;

#[cfg(feature = "redis")]
pub mod security;

#[cfg(feature = "database")]
pub mod database;

#[cfg(feature = "serialization")]
pub mod serialization;

#[cfg(feature = "metrics")]
pub mod metrics;

#[cfg(feature = "wal-recovery")]
pub mod recovery;

#[cfg(feature = "sync")]
pub mod sync;

#[cfg(feature = "http-cache")]
pub mod http;

#[cfg(feature = "bloom-filter")]
pub mod bloom_filter;

#[cfg(feature = "rate-limiting")]
pub mod rate_limiting;

#[cfg(feature = "smart-strategy")]
pub mod smart_strategy;

#[cfg(feature = "opentelemetry")]
pub mod telemetry;

#[cfg(any(feature = "testing", test))]
pub mod mock;

#[cfg(test)]
pub mod testing;
```

---

## 10. 依赖变更

### 10.1 移除 confers

```toml
# 删除以下内容
[dependencies.confers]
path = "/home/dev/projects/confers"
optional = true
features = ["toml", "json", "env", "validation", "schema"]
```

### 10.2 保留核心依赖

```toml
[dependencies]
tokio = { version = "~1.50", features = ["rt", "rt-multi-thread", "sync", "time", "macros"] }
async-trait = "0.1"
thiserror = "2.0"
anyhow = "~1.0"
serde = { version = "~1.0", features = ["derive"] }
serde_json = "~1.0"

moka = { version = "0.12", features = ["future"], optional = true }
dashmap = { version = "6.1", optional = true }
redis = { version = "1.1", features = ["aio", "tokio-comp"], optional = true }
tracing = { version = "~0.1", optional = true }
```

---

## 11. 实施步骤

### 阶段一：基础结构搭建（1-2 天）

1. 创建 `src/impl_/` 目录结构
2. 创建对外核心文件（config.rs, interface.rs, types.rs 等）
3. 更新 lib.rs 入口

### 阶段二：核心后端迁移（2-3 天）

1. 迁移内存后端 → impl\_/memory/
2. 迁移 Redis 后端 → impl\_/redis/
3. 迁移分层后端 → impl\_/tiered/
4. 迁移构建器 → impl\_/builder/

### 阶段三：功能模块迁移（2-3 天）

1. 迁移序列化 → impl\_/serialization/
2. 迁移数据库 → impl\_/database/
3. 迁移指标 → impl\_/metrics/
4. 迁移安全 → impl\_/security/
5. 迁移恢复/同步 → impl*/recovery/, impl*/sync/
6. 迁移其他功能模块

### 阶段四：辅助模块迁移（1 天）

1. 迁移 core 内部模块
2. 迁移 utils, client
3. 整合 traits
4. 迁移测试支持

### 阶段五：清理与依赖移除（1 天）

1. 移除 confers 依赖
2. 删除旧目录
3. 更新所有 import 路径

### 阶段六：测试与验证（1-2 天）

1. 更新测试用例
2. 运行完整测试套件
3. cargo clippy / fmt 检查
4. 基准测试

---

## 12. 验收标准

- [ ] 目录结构符合 BrickArchitecture 规范
- [ ] 所有 trait 方法使用 `&self`，内部状态通过 RwLock/Mutex 保护
- [ ] 配置错误和运行时错误分离
- [ ] 不向上泄漏第三方错误类型
- [ ] 提供 `new_in_memory()` 工厂函数
- [ ] 移除 confers 依赖
- [ ] 所有测试通过，零警告
- [ ] 文档覆盖所有公开 API
