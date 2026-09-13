# 📘 Oxcache API 参考

> **⚠️ API 版本说明**
>
> 本文档描述 **Oxcache v0.5.0-rc.4** 的 API。

本文档提供 Oxcache 库的详细 API 参考。

## 📋 目录

<details open>
<summary>📑 目录</summary>

- [🎨 特性要求](#-特性要求)
- [🪄 缓存宏](#-缓存宏)
- [🧩 `Cache<K, V>`](#-cachek-v)
- [🧰 CacheBuilder](#-cachebuilder)
- [🔌 后端层](#-后端层)
- [📡 RedisBackend](#-redisbackend)
- [🐉 DragonflyBackend](#-dragonflybackend)
- [🌌 AerospikeBackend](#-aerospikebackend)
- [🔗 ChainCache](#-chaincache)
- [🔄 同步 API](#-同步-api)
- [🌸 布隆过滤器](#-布隆过滤器)
- [🕒 TTL 管理](#-ttl-管理)
- [🔒 安全特性](#-安全特性)
- [📈 可观测性](#-可观测性)
- [🚨 错误处理](#-错误处理)
- [🏷 类型别名](#-类型别名)
- [🧬 trait-kit 集成](#-trait-kit-集成kit-feature)
- [💻 示例](#-示例)

</details>

## 🎨 特性要求

Oxcache 使用特性门控来控制功能。以下是关键特性及其要求：

### 分层特性集

- **`minimal`**：仅 L1 缓存（memory + metrics + serialization + chrono），**默认启用**
- **`core`**：L1 + L2 缓存（minimal + redis）
- **`full`**：全量预设（core + macros + compression + batch + lua + testing + dragonfly + aerospike + lock；不含 `bloom`、`kit` 等选择加入特性）

### 组件特性

| 特性 | 说明 |
|------|------|
| `memory` | L1 缓存后端（Moka + DashMap） |
| `redis` | L2 缓存实现（Redis + regex） |
| `dragonfly` | Dragonfly 缓存后端（Redis 协议兼容） |
| `aerospike` | Aerospike 缓存后端（独立协议） |
| `macros` | `#[cached]` 属性宏所需 |
| `serialization` | 序列化层（serde + serde_json + serde_stacker 深度防护） |
| `compression` | 自适应压缩（zstd 阈值触发，兼容旧 gzip） |
| `metrics` | 内置指标与可观测性 |
| `batch` | `BatchWriter` 缓冲 L2 写入 |
| `lua` | Lua 脚本执行支持（需要 `redis`） |
| `testing` | 测试支持工具 |
| `lock` / `red-lock` | 分布式锁与 RedLock 多节点多数派锁 |
| `telemetry` | `tracing` 门面遥测 |
| `invalidation` | 跨实例失效总线（Redis Pub/Sub） |
| `encrypt` / `integrity` | 值级加密与完整性装饰器 |
| `serde-bincode` / `postcard` | 二进制序列化格式 |
| `config-confers` | confers 配置驱动构建 + 热更新 |
| `degradation` | 自动降级与恢复（三态状态机） |
| `audit` | 结构化审计事件流 |
| `versioning` | 版本化 CAS |
| `bloom` | 负查询过滤（**不包含**在 `full` 中） |
| `kit` | trait-kit AsyncKit 集成（**不包含**在 `full` 中） |

### 示例配置

```toml
# 全量特性（推荐）
oxcache = { version = "0.5.0-rc.4", features = ["full"] }

# 核心功能（L1 + L2）
oxcache = { version = "0.5.0-rc.4", features = ["core"] }

# 最小特性 - 仅 L1 缓存（默认）
oxcache = { version = "0.5.0-rc.4", features = ["minimal"] }

# 自定义选择（例如在 core 基础上添加 bloom）
oxcache = { version = "0.5.0-rc.4", features = ["core", "macros", "bloom"] }
```

### 特性依赖

部分特性需要或隐含其他特性：

| 特性 | 所需特性 | 说明 |
|------|----------|------|
| `lua` | `redis` | Lua 脚本执行 |
| `lock` | `redis` | 分布式锁 |
| `red-lock` | `lock` | RedLock 多节点多数派锁 |
| `metrics` | `serialization`, `chrono`, `dashmap` | 内置指标 |
| `macros` | `minimal` | 属性宏 |
| `core` | `minimal`, `redis` | 核心 L1 + L2 缓存 |
| `full` | `core`, `macros`, `compression`, `batch`, `lua`, `testing`, `dragonfly`, `aerospike`, `lock` | 全量预设（注意：`bloom`、`kit` 不在 `full` 中） |

## 🪄 缓存宏

### `#[cached]` 属性宏

零模板代码的函数缓存装饰器。需要 `macros` 特性。

**参数：**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `service` | `&str` | 否 | `"default"` | 缓存服务名（用于查找已注册的 `Cache` 实例） |
| `ttl` | `u64` | 否 | `None` | 生存时间（秒），设置时为 `Some(n)` |
| `key` | `&str` | 否 | 自动生成 | 自定义缓存键格式 |
| `key_prefix` | `&str` | 否 | `""` | 自动生成缓存键的前缀 |
| `sync` | （标志） | 否 | async | 使用同步代码路径（`get_bytes_sync`/`set_bytes_sync`）；不能与 `async fn` 组合使用 |
| `skip_cache_write` | （标志） | 否 | `false` | 跳过 Ok 结果的缓存写入（Err 结果本就不缓存） |
| `single_flight` | （标志） | 否 | `false` | 同 key 并发 miss 仅回源一次 |
| `strict` | （标志） | 否 | `false` | 未注册缓存时 panic 而非静默穿透 |
| `condition` | 函数路径 | 否 | — | 执行前谓词，返回 false 时旁路缓存 |
| `cache_none` | （标志） | 否 | `false` | 缓存 `None` 结果 |

该宏通过 `oxcache::__internal_get_cache(service)` 从内部注册表获取 `Cache` 实例。
如果 `service` 下未注册缓存，则原始函数不经缓存直接执行（`strict` 模式改为 panic）。
使用 `sync` 标志时，`Cache` 必须通过 `sync_mode(true)` 构建。

**示例（异步）：**

```rust
// Cargo.toml: oxcache = { version = "0.5.0-rc.4", features = ["macros"] }
use oxcache::cached;

#[cached(service = "default", ttl = 3600)]
async fn fetch_user(user_id: &str) -> Result<User, String> {
    // 函数体
    # Ok(User { /* ... */ })
}
```

**自定义键格式：**

```rust
#[cached(service = "default", ttl = 3600, key = "user:{user_id}")]
async fn fetch_user(user_id: &str) -> Result<User, String> {
    // 函数体
    # Ok(User { /* ... */ })
}
```

**默认键生成**：未设置 `key` 和 `key_prefix` 时：
`{service}:{fn_name}:{arg1:arg2:...}`。

## 🧩 `Cache<K, V>`

主要的类型安全缓存类型。`K: CacheKey`，`V: Serialize + Deserialize`。

对于无需类型参数的字节级操作，使用 `BytesCache` 类型别名：

```rust
use oxcache::BytesCache;

let cache: BytesCache = Cache::builder().build().await?; // Cache<String, Vec<u8>>
cache.set_bytes("k", b"raw".to_vec(), None).await?;
let v: Option<Vec<u8>> = cache.get_bytes("k").await?;
```

**构建：**

```rust
use oxcache::Cache;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct User { id: u64, name: String }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 默认 Moka 内存后端（容量 10000）
    let cache: Cache<String, User> = Cache::builder().build().await?;

    // 注册供 #[cached] 宏使用
    cache.register_for_macro("default").await?;
    Ok(())
}
```

**构造方法：**

| 构造方法 | 特性 | 说明 |
|----------|------|------|
| `Cache::builder()` | — | 返回 `CacheBuilder<K, V>` |
| `Cache::new()` | `memory` | 默认 Moka 后端（同步，无需 `.await`） |
| `Cache::memory().await` | — | 便捷方法：默认 Moka 后端 |
| `Cache::redis(url).await` | `redis` | 便捷方法：Redis 后端 |
| `Cache::with_dependencies(backend)` | — | 从 `Arc<dyn CacheBackend>` 构造 |

### 异步操作

#### `get(&self, key: &K) -> OxCacheResult<Option<V>>`

从缓存获取值。未找到时返回 `Ok(None)`。

```rust
let user: Option<User> = cache.get(&"user:1".to_string()).await?;
```

#### `set(&self, key: &K, value: &V) -> OxCacheResult<()>`

设置值，不设单条目 TTL（使用后端的全局 TTL，如有）。

```rust
cache.set(&"user:1".to_string(), &user).await?;
```

#### `set_with_ttl(&self, key: &K, value: &V, ttl: Option<Duration>) -> OxCacheResult<()>`

设置值并指定可选的单条目 TTL。`Some(d)` 覆盖该条目的后端全局 TTL；
`None` 回退到全局 TTL。

```rust
use std::time::Duration;
cache.set_with_ttl(&"user:1".to_string(), &user, Some(Duration::from_secs(3600))).await?;
```

#### `get_by_str(&self, key: &str) -> OxCacheResult<Option<V>>` / `set_by_str(&self, key: &str, value: &V, ttl: Option<Duration>) -> OxCacheResult<()>`

借用键读写，消除热路径上的 `String` 分配（`set` 路径仅 1 次 `Arc<str>` 分配）。
实测收益见[性能基线](PERFORMANCE.md)。

#### `delete(&self, key: &K) -> OxCacheResult<()>`

从缓存删除值。

#### `exists(&self, key: &K) -> OxCacheResult<bool>`

检查键是否存在于缓存中。

#### `clear(&self) -> OxCacheResult<()>`

清空所有条目。

#### `get_or<F, Fut>(&self, key: &K, fallback: F) -> OxCacheResult<V>`

获取值或通过 `fallback` 计算（单飞模式：同一键的并发调用共享一次计算）。

```rust
let user = cache.get_or(&"user:1".to_string(), || async {
    fetch_user_from_db(1).await
}).await?;
```

#### `ttl(&self, key: &K) -> OxCacheResult<Option<Duration>>`

获取键的剩余生存时间。如果键没有单条目 TTL 或不存在，返回 `Ok(None)`。
适用于保留 TTL 的更新流程：

```rust
let original_ttl = cache.ttl(&"user:1".to_string()).await?;
cache.set_with_ttl(&"user:1".to_string(), &new_user, original_ttl).await?;
```

#### `expire(&self, key: &K, ttl: Duration) -> OxCacheResult<bool>`

更新已有键的 TTL 而不修改其值。TTL 更新成功返回 `Ok(true)`，键不存在返回 `Ok(false)`。

### 生命周期与统计

| 方法 | 返回值 | 说明 |
|------|--------|------|
| `health_check().await` | `OxCacheResult<()>` | 后端健康检查 |
| `stats().await` | `Result<HashMap<String,String>>` | 后端特定统计信息 |
| `len().await` | `OxCacheResult<u64>` | 条目数量 |
| `is_empty().await` | `OxCacheResult<bool>` | 缓存是否为空 |
| `capacity().await` | `OxCacheResult<u64>` | 配置的容量（Redis 为 0） |
| `shutdown().await` | `()` | 关闭并释放资源 |
| `register_for_macro(service).await` | `OxCacheResult<()>` | 注册供 `#[cached]` 宏使用 |

## 🧰 CacheBuilder

`CacheBuilder<K, V>` 是 `Cache` 的统一构建器。通过 `Cache::builder()` 获取。

**方法：**

| 方法 | 签名 | 说明 |
|------|------|------|
| `backend_arc` | `(backend: Arc<dyn CacheBackend>) -> Self` | 添加预构建的后端 |
| `ttl` | `(ttl: Duration) -> Self` | 缓存条目的默认 TTL |
| `tti` | `(tti: Duration) -> Self` | 内存后端的默认 TTI（time-to-idle） |
| `capacity` | `(capacity: u64) -> Self` | 内存后端的容量（默认 10000） |
| `sync_mode` | `(enabled: bool) -> Self` | 启用同步 API（参见[同步 API](#-同步-api)） |
| `serialization_format` | `(format: SerializationFormat) -> Self` | 切换序列化格式（JSON/Bincode/Postcard，需 `serde-bincode`/`postcard`） |
| `null_cache_ttl` | `(ttl: Duration) -> Self` | 空值哨兵 TTL（穿透防护） |
| `ttl_jitter` | `(factor: f64) -> Self` | TTL 抖动系数（防批量同时过期） |
| `metrics` | `(recorder: Arc<dyn MetricsRecorder>) -> Self` | 注入 `MetricsRecorder`（覆盖纯 L1 路径指标） |
| `audit_publisher` | `(publisher: Arc<dyn AuditEventPublisher>) -> Self` | 注入审计事件发布器（`audit` 特性，hit/miss/set/delete 结构化事件、键脱敏） |
| `build` | `async (self) -> Result<Cache<K, V>>` | 构建缓存实例（异步包装，内部无 await） |
| `build_sync` | `(self) -> Result<Cache<K, V>>` | 同步构建缓存实例（无需运行时） |

> **注意：** `CacheBuilder` 没有 `.redis(...)`、`.tiered(...)`、`.with_backend(...)`、
> `.batch_writes(...)` 或 `.auto_promote(...)` 方法。使用
> `.backend_arc(Arc::new(...))` 插入 `RedisBackend` 或其他后端。

**默认 Moka 路径**（未调用 `backend_arc` 时）：使用给定的 `capacity`/`ttl`/`tti`
构建 `MokaMemoryBackend`。这是唯一支持 `sync_mode(true)` 的配置。

**示例：**

```rust
use oxcache::{Cache, CacheBuilder};
use std::time::Duration;

// 仅 L1 内存缓存
let cache: Cache<String, String> = Cache::builder()
    .capacity(10000)
    .ttl(Duration::from_secs(3600))
    .tti(Duration::from_secs(600))
    .build()
    .await?;
```

```rust
use oxcache::{Cache, backend::RedisBackend};
use std::sync::Arc;

// 通过预构建后端实现 L2（Redis）缓存
let redis = RedisBackend::new("rediss://localhost:6379").await?;
let cache: Cache<String, String> = Cache::builder()
    .backend_arc(Arc::new(redis))
    .build()
    .await?;
```

**同步 API 限制：** `sync_mode(true)` 与 `backend_arc(...)` 组合使用时
返回 `Err(OxCacheError::NotSupported)`，因为 stable Rust 不支持
`Arc<dyn CacheBackend>` 到 `Arc<dyn SyncCacheBackend>` 的上转（缺少 `trait_upcasting`）。
使用默认 Moka 后端配合 `sync_mode`，或通过 `Cache::with_dependencies` + `set_sync_backend`
手动接入同步后端。

## 🔌 后端层

`backend` 模块暴露后端 trait 和实现。

### 后端 Trait

```rust
#[async_trait]
pub trait CacheReader: Send + Sync + 'static {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>>;
    async fn exists(&self, key: &str) -> OxCacheResult<bool>;
    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>>;
    async fn len(&self) -> OxCacheResult<u64>;
    async fn is_empty(&self) -> OxCacheResult<bool>;  // 默认实现
    async fn capacity(&self) -> OxCacheResult<u64>;
    async fn stats(&self) -> OxCacheResult<HashMap<String, String>>;
    async fn get_many(&self, keys: &[String]) -> OxCacheResult<Vec<Option<Vec<u8>>>>;  // 默认实现
}

/// 批量写入条目类型：`(Arc<str> key, Arc<Vec<u8>> value, Option<Duration> ttl)`
pub type CacheSetItem = (Arc<str>, Arc<Vec<u8>>, Option<Duration>);

#[async_trait]
pub trait CacheWriter: Send + Sync + 'static {
    async fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()>;
    async fn delete(&self, key: &str) -> OxCacheResult<()>;
    async fn clear(&self) -> OxCacheResult<()>;
    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool>;
    async fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()>;  // 默认实现
    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()>;  // 默认实现
}

#[async_trait]
pub trait CacheConnector: Send + Sync + 'static {
    async fn health_check(&self) -> OxCacheResult<()>;
    async fn shutdown(&self);
    fn backend_kind(&self) -> BackendKind;
    #[cfg(feature = "lua")]
    fn as_lua_executor(&self) -> Option<&dyn LuaExecutor> { None }
}

// blanket 实现：实现以上三个 trait 的类型自动成为 CacheBackend。
pub trait CacheBackend: CacheReader + CacheWriter + CacheConnector + 'static {}
```

### 后端类型

| 类型 | 特性 | 说明 |
|------|------|------|
| `MokaMemoryBackend` | `memory` | 使用 Moka 的内存缓存（LRU/TinyLFU 淘汰）。通过 `moka::Expiry` 支持单条目 TTL。 |
| `DashMapMemoryBackend` | `memory` | 使用 DashMap 的纯内存并发缓存（懒 TTL 过期，FIFO O(1) 超容量淘汰）。 |
| `RedisBackend` | `redis` | 使用 Redis 的分布式缓存（Standalone/Sentinel/Cluster）。 |
| `DragonflyBackend` | `dragonfly` | Dragonfly 缓存（Redis 协议兼容，包装 RedisBackend）。 |
| `AerospikeBackend` | `aerospike` | Aerospike 持久化 KV 存储（独立协议，feature-gated）。 |
| `ChainCache` | — | 多级缓存链（参见 [ChainCache](#-chaincache)）。 |
| `BloomFilterBackend` | `bloom` | 装饰器，布隆过滤器判定 key 不存在时跳过内部后端。 |

### 内存后端辅助

```rust
use oxcache::backend::{moka_memory, dashmap_memory, default_memory_backend, MokaMemoryBackend};

let moka = MokaMemoryBackend::builder().capacity(10000).build();
```

`MokaMemoryBackend::builder()` 暴露 `.capacity(u64)`、`.ttl(Duration)`、
`.time_to_idle(Duration)` 和 `.build()`（同步）。

### 后端分数

每个后端报告一个 `score()`（越高 = 越快），供 `ChainCache` 排序读写：

| 后端 | score | `is_persistent()` |
|------|:-----:|:-----------------:|
| Moka | 100 | false |
| DashMap | 90 | false |
| Redis | 50 | true |
| Valkey | 50 | true |
| Dragonfly | 50 | true |
| Aerospike | 30 | true |

## 📡 RedisBackend

`RedisBackend` 是 L2 分布式缓存。使用 `redis::aio::ConnectionManager`
进行连接池管理，重导出位于 `oxcache::backend::RedisBackend`。

**安全性：** 连接必须使用 TLS（`rediss://`）。非 TLS 连接会被拒绝，
除非设置了环境变量 `OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS`
（或 `=development-only`）。

### 构造方法

```rust
use oxcache::backend::RedisBackend;

// 从连接字符串构造（推荐使用 TLS）
let backend = RedisBackend::new("rediss://localhost:6379").await?;

// 显式指定连接池大小（当前等价于 new()）
let backend = RedisBackend::with_pool("rediss://localhost:6379", 8).await?;

// 通过 builder 构造
let backend = RedisBackend::builder()
    .connection_string("rediss://localhost:6379")
    .mode(oxcache::backend::RedisMode::Standalone)
    .build()
    .await?;
```

**`RedisBackendBuilder` 方法：**

| 方法 | 说明 |
|------|------|
| `connection_string(&str)` | 设置 Redis 连接字符串 |
| `mode(RedisMode)` | 设置 Redis 模式（`Standalone`/`Sentinel`/`Cluster`/`ValkeyStandalone`） |
| `build().await` | 构建 `RedisBackend`（2 秒连接超时） |

### 实例方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `ping().await` | `OxCacheResult<String>` | Ping 服务器（返回 `"PONG"`） |
| `mode()` | `RedisMode` | 配置的 Redis 模式 |
| `client()` | `&Client` | 底层 `redis::Client` |
| `redact_connection_string(s)` | `String`（关联方法） | 脱敏连接字符串中的密码 |

### Pipeline 批量操作

适用于高吞吐场景，使用 Redis pipeline（单次往返）：

```rust
// 批量 SET
backend.set_many_pipeline(&[("k1", v1), ("k2", v2)], Some(Duration::from_secs(60))).await?;

// 批量 GET
let values: Vec<Option<Vec<u8>>> = backend.get_many_pipeline(&["k1", "k2"]).await?;

// 批量 DEL
backend.delete_many_pipeline(&["k1", "k2"]).await?;
```

### Lua 脚本（`lua` 特性）

启用 `lua` 特性后，`RedisBackend` 实现 `LuaExecutor`：

```rust
use oxcache::backend::interface::LuaExecutor;

// EVAL
let val = backend.eval_lua("return redis.call('GET', KEYS[1])", &["k1"], &[]).await?;

// SCRIPT LOAD + EVALSHA
let sha = backend.script_load("return 1 + 1").await?;
let val = backend.eval_sha(&sha, &[], &[]).await?;
```

所有 Lua 脚本在执行前通过 `validate_lua_script` 校验。

## 🐉 DragonflyBackend

`DragonflyBackend` 包装 `RedisBackend`，提供对 Dragonfly 服务器的缓存支持。
Dragonfly 兼容 Redis 协议，因此复用 Redis 的全部读写路径。

```rust
use oxcache::backend::DragonflyBackend;
use std::sync::Arc;

// 构造 Dragonfly 后端（需要 dragonfly feature）
let backend = DragonflyBackend::new("redis://localhost:6379", 8).await?;

// 作为 ChainCache L2 使用
use oxcache::backend::MokaMemoryBackend;
use oxcache::cache::chain::{ChainCacheBuilder, ChainLink};
let chain = ChainCacheBuilder::default()
    .link(ChainLink::new(MokaMemoryBackend::new(), 100, false, "moka"))
    .link(ChainLink::new(backend, 50, true, "dragonfly"))
    .build();
```

**限制：**
- `as_atomic_writer()` 返回 `None`（Dragonfly 原子操作兼容性待验证）
- `backend_kind()` 返回 `BackendKind::Dragonfly`

## 🌌 AerospikeBackend

`AerospikeBackend` 通过 `aerospike` feature 启用，
提供 Aerospike 持久化 KV 存储后端。

```rust
use oxcache::backend::{AerospikeBackend, AerospikeConfig};

let config = AerospikeConfig {
    seed_nodes: vec!["127.0.0.1:3000".to_string()],
    namespace: "test".to_string(),
    set_name: "cache".to_string(),
    default_ttl: 3600,
    ip_map: None, // Docker/NAT 环境设置 IP 转换表
};
let backend = AerospikeBackend::new(config).await?;
```

**不支持的操作：** `len()`、`capacity()`、`keys()`、`clear()` 返回 `Err(NotSupported)`。

## 🔗 ChainCache

`ChainCache` 管理多个后端，按分数降序排列。读取从最高分后端开始
（遇到 `None` 或错误时穿透到下一个链接）；写入**并发**扇出到所有后端，
容忍单链接失败（仅在*所有*后端都失败时才报错）。回填（backfill）可选地在
低分后端命中时**异步**填充更高分后端。
健康检查并发执行，每个后端 5 秒超时。

```rust
use oxcache::cache::{ChainCache, ChainLink};
use oxcache::backend::{MokaMemoryBackend, RedisBackend};
use std::time::Duration;

let l1 = MokaMemoryBackend::builder().capacity(10000).ttl(Duration::from_secs(300)).build();
let l2 = RedisBackend::new("rediss://localhost:6379").await?;

let chain = ChainCache::builder()
    .link(ChainLink::from_backend(l1))   // L1，分数 100
    .link(ChainLink::from_backend(l2))   // L2，分数 50
    .enable_backfill()
    .default_time_to_live(Duration::from_secs(600))
    .build();  // 同步构建，返回 ChainCache

chain.set("key", b"value".to_vec(), None).await?;
let v = chain.get("key").await?; // Some(Vec<u8>)
```

### `ChainCacheBuilder`

| 方法 | 说明 |
|------|------|
| `link(ChainLink)` | 添加一个链接 |
| `links(Vec<ChainLink>)` | 添加多个链接 |
| `backend(B)` | 添加一个后端（通过 `ChainLink::from_backend` 自动包装） |
| `default_time_to_live(Duration)` | `set` 以 `ttl=None` 调用时使用的默认 TTL |
| `enable_backfill()` / `disable_backfill()` | 切换回填（默认关闭） |
| `enable_race_read()` / `disable_race_read()` | 切换并发首次命中读取（默认关闭） |
| `event_publisher(Arc<dyn EventPublisher>)` | 配置事件发布器（后端操作失败时发射事件） |
| `build()` | 构建 `ChainCache`（同步；按分数降序排列链接） |

### `ChainLink`

| 构造方法 | 说明 |
|----------|------|
| `new(backend, score, is_persistent, name)` | 手动构造 |
| `from_backend(B)` | 从 `BackendScore` 自动推导 score/persistent/name（仅异步 API） |
| `from_sync_backend(B)` | 类似 `from_backend` 但同时填充同步后端句柄（需要 `SyncCacheBackend`） |

`ChainLink` 访问器：`backend()`、`try_as_sync_backend()`、`score()`、
`is_persistent()`、`name()`。

### TTL 行为

`ChainCache` 本身不存储 TTL；它透明地转发到各链接：

- `set(key, value, Some(d))` → 所有链接使用相同的 TTL `d`。
- `set(key, value, None)` → 链接使用 `default_ttl`（如已设置），否则使用各链接自身的全局 TTL。
- `ttl(key)` → 返回从最高分开始扫描找到的第一个 `Some(ttl)`。
- `expire(key, d)` → 转发到所有链接；任一链接成功即返回 `Ok(true)`。

### ChainCache 同步 API

`ChainCache` 暴露 `get_sync`/`set_sync`/`delete_sync`。这些方法要求**每个**
链接都支持 `SyncCacheBackend`（即通过 `from_sync_backend` 构建）；
否则返回 `Err(OxCacheError::NotSupported)`。

## 🔄 同步 API

同步 API 镜像异步 API，但通过 `tokio::task::block_in_place` 阻塞。
它需要**多线程** Tokio 运行时（从 current-thread 运行时调用会返回 `Err(NotSupported)`）。

### `SyncCacheBackend` Trait 层级

```rust
pub trait SyncCacheReader: Send + Sync + 'static { fn get(&self, key: &str) -> OxCacheResult<Option<Arc<Vec<u8>>>>; /* ... */ }
pub trait SyncCacheWriter: Send + Sync + 'static { fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()>; /* ... */ }
pub trait SyncCacheConnector: Send + Sync + 'static { fn health_check(&self) -> OxCacheResult<()>; /* ... */ }
pub trait SyncCacheBackend: SyncCacheReader + SyncCacheWriter + SyncCacheConnector {}
```

实现者：`MokaMemoryBackend`、`DashMapMemoryBackend`、`RedisBackend`。

### 在 `Cache<K, V>` 上启用同步

```rust
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await?;

    // 同步方法现在可用
    cache.set_sync(&"k".to_string(), &"v".to_string())?;
    let v = cache.get_sync(&"k".to_string())?; // Some("v")
    Ok(())
}
```

### `Cache<K, V>` 上的同步方法

| 方法 | 说明 |
|------|------|
| `get_sync(&K)` | `OxCacheResult<Option<V>>` |
| `set_sync(&K, &V)` | `OxCacheResult<()>`（不设单条目 TTL） |
| `set_with_ttl_sync(&K, &V, Option<Duration>)` | `OxCacheResult<()>` |
| `delete_sync(&K)` | `OxCacheResult<()>` |
| `exists_sync(&K)` | `OxCacheResult<bool>` |
| `ttl_sync(&K)` | `OxCacheResult<Option<Duration>>` |
| `expire_sync(&K, Duration)` | `OxCacheResult<bool>` |
| `get_or_sync(&K, fallback)` | `OxCacheResult<V>`（单飞，同步） |
| `clear_sync()` | `OxCacheResult<()>` |

当 `sync_mode` 为 `false`（默认）时，所有 `*_sync` 方法返回
`Err(OxCacheError::NotSupported)`。

## 🌸 布隆过滤器

`bloom` 特性（**不包含**在 `full` 中）提供负查询过滤。
`BloomFilterBackend` 包装任意 `CacheBackend`，当布隆过滤器判定键不存在时跳过内部后端。

### `BloomFilter`

```rust
use oxcache::features::bloom_filter::BloomFilter;

let bf = BloomFilter::new(100_000, 0.01); // 容量、误判率
bf.insert("key1");
assert!(bf.contains("key1"));     // 已插入的键始终返回 true
assert!(!bf.contains("absent"));  // 通常为 false（可能误判）
```

**方法：** `insert(&str)`、`contains(&str) -> bool`、`clear()`、`len() -> u64`、
`is_empty() -> bool`、`capacity() -> usize`、`false_positive_rate() -> f64`、
`load_factor() -> f64`、`rebuild(new_capacity)`。

`BloomFilter` 是 `Clone` 的，通过 `Arc<RwLock<...>>` 共享状态，
因此一个克隆上的插入对所有克隆可见。

### `BloomFilterBackend`

```rust
use oxcache::backend::MokaMemoryBackend;
use oxcache::features::bloom_filter::{BloomFilterBackend, BloomFilterBackendBuilder};

let inner = MokaMemoryBackend::new();
let backend = BloomFilterBackend::new(inner);  // 装饰器包装内部后端

// 或经 builder 配置容量与误判率
let backend = BloomFilterBackend::builder()
    .capacity(10_000)
    .false_positive_rate(0.01)
    .inner(MokaMemoryBackend::new())
    .build()?;
```

`BloomFilterBackendBuilder` 允许配置底层 `BloomFilter`
（容量、误判率）。该装饰器实现了 `CacheBackend`，因此可用于任何
需要 `CacheBackend` 的地方（包括 `ChainCache` 链接和 `CacheBuilder::backend_arc`）。

## 🕒 TTL 管理

所有后端（Moka、DashMap、Redis、Mock、Chain、Bloom）都通过
`set(key, value, Some(ttl))` 支持单条目 TTL。

- **Moka** 使用 `moka::Expiry` trait 实现真正的单条目 TTL，覆盖构建器设置的全局 TTL。
- **DashMap** 使用懒过期（条目在访问时过期）。
- **Redis** 使用 `SETEX`。

各后端的完整行为对照表见 [README](../README.md#-ttl-行为对照表)。

### TTL 方法

| 方法 | 异步 | 同步 |
|------|------|------|
| 读取剩余 TTL | `cache.ttl(&key).await` | `cache.ttl_sync(&key)` |
| 更新已有键的 TTL | `cache.expire(&key, d).await` | `cache.expire_sync(&key, d)` |
| 设置显式 TTL | `cache.set_with_ttl(&key, &v, Some(d)).await` | `cache.set_with_ttl_sync(&key, &v, Some(d))` |

## 🔒 安全特性

安全函数在 **crate 根**重导出（启用 `redis` 或 `full` 特性时），不在 `oxcache::security` 下：

```rust
use oxcache::{
    validate_redis_key, validate_lua_script, validate_scan_pattern, clamp_scan_count,
    redact_cache_key, redact_connection_string, redact_field, redact_value, Redacted,
    // 安全日志辅助：
    // log_cache_key, sanitize_message,
};
```

> **导入路径说明：** 使用 `use oxcache::validate_redis_key`（crate 根重导出），
> 而非 `use oxcache::security::validate_redis_key`。`security` 模块本身是
> `pub(crate)` 不可直接访问。完整的威胁模型与安全设计见[安全文档](SECURITY.md)。

### 键校验（validate_redis_key）

校验 Redis 键格式和内容。

```rust
use oxcache::validate_redis_key;

validate_redis_key("user:123").expect("合法的键");
// 空、过长或包含危险字符的键返回 Err(OxCacheError::InvalidInput)
```

**校验规则：**
- 键不能为空
- 键不能超过 512KB
- 键不能包含危险字符（`\r`、`\n`、`\0`）与命令注入字符（`;`、`|`、`&`、`` ` ``）
- 拒绝除 `\t` 外的 Unicode 控制字符
- 键会扫描 SQL 注入和路径遍历模式（`../`、`etc/passwd`）

### Lua 脚本校验（validate_lua_script）

校验 Lua 脚本的安全性。

```rust
use oxcache::validate_lua_script;

validate_lua_script("return redis.call('GET', KEYS[1])", 1).expect("合法的脚本");
```

**校验规则：**
- 脚本长度不能超过 10KB
- 键数量不能超过 100
- 阻止危险命令：`FLUSHALL`、`FLUSHDB`、`KEYS`、`SHUTDOWN`、`DEBUG`、`CONFIG`、`SAVE`、`BGSAVE`、`MONITOR`
- 拦截 `os.execute` / `io.popen` / `loadstring` 等沙箱逃逸调用与无限循环模式
- 注释预处理防止通过注释绕过

### SCAN 校验（validate_scan_pattern 与 clamp_scan_count）

校验 SCAN 模式以防止 ReDoS 攻击。

```rust
use oxcache::validate_scan_pattern;

validate_scan_pattern("user:*").expect("合法的模式");
```

**校验规则：**
- 模式长度不能超过 256 个字符
- 最多 10 个通配符（`*`）

`clamp_scan_count(count: usize) -> usize` 将 SCAN `COUNT` 参数钳制到安全范围（1–1000）。

### 敏感数据脱敏

```rust
use oxcache::{redact_connection_string, redact_cache_key, redact_value, Redacted};

let safe = redact_connection_string("redis://:secret@host:6379");
assert!(!safe.contains("secret"));
```

`Redacted` 是一个包装类型，防止内部值被意外记录到日志。
使用 `redact_field` / `redact_value` 在日志中包装敏感数据。

## 📈 可观测性

### 指标（`metrics` 特性）

缓存统计和导出辅助函数在 crate 根重导出：

```rust
use oxcache::{CacheStats, get_enhanced_stats, export_json_format, export_prometheus_format};

let stats: CacheStats = get_enhanced_stats();
println!("L1 命中: {}", stats.l1_hits);
println!("整体命中率: {:.2}%", stats.overall_hit_rate() * 100.0);

// 导出为 Prometheus / JSON 文本（export_json_format 返回 Result）
let prometheus_text = export_prometheus_format();
let json_text = export_json_format()?;
```

`CacheStats` 以公共字段暴露 L1/L2 命中、未命中与操作计数（`l1_hits` / `l1_misses` /
`l2_hits` / `l2_misses` / `total_operations` 等），并提供 `l1_hit_rate` /
`l2_hit_rate` / `overall_hit_rate` 命中率方法与 `export_prometheus` / `export_json` 实例方法。
`MetricsCollector`（位于 `oxcache::infra::metrics::backend`）提供底层
计数器（L1/L2 命中/未命中、操作计数器）；标准 exposition 格式导出经
`export_prometheus_standard()`。

### 事件发射（`EventPublisher`）

Oxcache 通过 `EventPublisher` trait 提供结构化事件发射机制。
后端操作失败时，`ChainCache` 通过配置的 `EventPublisher` 抛出事件，
用户可自行决定处理方式（日志、metrics、告警或忽略）。

```rust
use oxcache::EventPublisher;  // crate 根重导出（core 模块为私有）
use std::sync::Arc;

// 实现自定义事件发布器
struct MyPublisher;
impl EventPublisher for MyPublisher { /* ... */ }

// 配置到 ChainCache
let chain = ChainCache::builder()
    .link(ChainLink::from_backend(l1))
    .event_publisher(Arc::new(MyPublisher))
    .build();
```

内置 metrics 实现通过 `metrics` 特性可用（无外部 OpenTelemetry 依赖；
OTLP 导出如需要应由应用层处理）。`metrics` 特性引入 `serialization`、
`chrono` 和 `dashmap` 用于内部统计收集和 JSON 导出。

## 🚨 错误处理

### `OxCacheError`

所有缓存操作返回 `Result<T, OxCacheError>`（别名为 `oxcache::OxCacheResult<T>`）。

```rust
use oxcache::OxCacheError;

match result {
    Ok(value) => /* ... */,
    Err(OxCacheError::NotFound(key)) => println!("键未找到: {}", key),
    Err(OxCacheError::NotSupported(msg)) => println!("不支持: {}", msg),
    Err(OxCacheError::Connection(msg)) => println!("连接错误: {}", msg),
    Err(OxCacheError::Serialization(msg)) => println!("序列化错误: {}", msg),
    Err(e) => println!("其他错误 ({}): {}", e.code(), e),
}
```

### 错误变体与错误码

| 变体 | 错误码 | 可恢复 | 说明 |
|------|--------|--------|------|
| `NotFound(String)` | `OXCACHE_001` | 否 | 键未找到 |
| `Connection(String)` | `OXCACHE_002` | 是 | 网络连接失败 |
| `Serialization(String)` | `OXCACHE_003` | 否 | 序列化/反序列化失败 |
| `Operation(String)` | `OXCACHE_004` | 否 | 一般操作失败 |
| `Degraded(String)` | `OXCACHE_005` | 否 | 缓存处于降级模式 |
| `L1Error(String)` | `OXCACHE_006` | 否 | L1 缓存操作失败 |
| `L2Error(String)` | `OXCACHE_007` | 是 | L2 缓存操作失败 |
| `NotSupported(String)` | `OXCACHE_009` | 否 | 操作不支持（如未启用 `sync_mode` 的同步 API） |
| `WalError(String)` | `OXCACHE_010` | 否 | WAL 操作失败 |
| `DatabaseError(String)` | `OXCACHE_011` | 否 | 数据库错误 |
| `RedisError(...)` | `OXCACHE_012` | 是 | Redis 错误 |
| `IoError(io::Error)` | `OXCACHE_013` | 否 | I/O 错误 |
| `BackendError(String)` | `OXCACHE_014` | 是 | 后端错误 |
| `Timeout(String)` | `OXCACHE_015` | 是 | 操作超时 |
| `ShutdownError(String)` | `OXCACHE_016` | 否 | 关闭错误 |
| `KeyTooLong(usize, usize)` | `OXCACHE_017` | 否 | 键超过最大长度 |
| `ValueTooLarge(usize, usize)` | `OXCACHE_018` | 否 | 值超过最大大小 |
| `BufferFull(String)` | `OXCACHE_019` | 是 | 批量写入缓冲区已满 |
| `InvalidInput(String)` | `OXCACHE_020` | 否 | 无效输入 |
| `InvalidKey(String)` | `OXCACHE_021` | 否 | 无效键 |
| `LockError(String)` | `OXCACHE_022` | 否 | 锁中毒 |
| `ServiceNotFound(String)` | `OXCACHE_023` | 否 | 服务未在注册表中 |
| `Internal(String)` | `OXCACHE_024` | 否 | 内部状态损坏 |

### 辅助方法

- `OxCacheError::code() -> &'static str`：稳定错误码（如 `"OXCACHE_009"`）。
- `is_recoverable() -> bool`：`Connection`/`Timeout`/`RedisError`/`L2Error`/`BackendError`/`BufferFull` 返回 `true`。
- `is_not_found() -> bool`、`is_connection_error() -> bool`、`is_degraded() -> bool`。

### `OxCacheConfigError`（`redis` 特性）

配置阶段错误（由工厂函数和构建器返回）：

```rust
pub enum OxCacheConfigError {
    MissingField(String),
    InvalidValue { field: String, reason: String },
    UnsupportedBackend(String),
    ConnectionFailed(String),
}
pub type OxCacheConfigResult<T> = std::result::Result<T, OxCacheConfigError>;
```

## 🏷 类型别名

```rust
pub type OxCacheResult<T> = std::result::Result<T, OxCacheError>;
pub type RedisMode = RedisModeType; // Standalone | Sentinel | Cluster | ValkeyStandalone
```

## 🧬 trait-kit 集成（kit feature）

启用 `kit` feature 后，oxcache 提供 trait-kit `AsyncKit` 集成模块 `oxcache::integrations::kit`：

| 类型 / 函数 | 说明 |
|------------|------|
| `OxcacheModule` | 实现 `AsyncAutoBuilder`，构建 `Arc<dyn CacheBackend + Send + Sync>` 能力 |
| `OxcacheConfig` | 模块配置（`capacity: u64`, `ttl: Option<Duration>`, `tti: Option<Duration>`, `backend` 枚举 Memory/Redis/Chain） |
| `OxcacheBuildObserver` | 实现 `BuildObserver`，可通过 `AsyncKit::with_observer` 注册构建观察者 |
| `register_cache_shutdown` | 将 `CacheBackend` 关闭映射到 `AsyncShutdownCoordinator` 三阶段 |
| `CacheBackendDecorator` | 装饰器类型别名 `Arc<dyn Fn(Arc<dyn CacheBackend>) -> Arc<dyn CacheBackend>>` |
| `register_cache_decorator` | 辅助函数，调用 `kit.decorate::<OxcacheModule>(decorator)` |

```rust
use oxcache::integrations::kit::*;
use trait_kit::prelude::*;
use std::sync::Arc;

let mut kit = AsyncKit::new();
kit.set_config(OxcacheConfig::default());
kit.with_observer(Arc::new(OxcacheBuildObserver));
kit.register::<OxcacheModule>().unwrap();

// 注册装饰器
register_cache_decorator(&kit, |backend| backend);

let built = kit.build().await.unwrap();
let backend = built.require::<OxcacheModule>().unwrap();

// 注册三阶段关闭
let coord = AsyncShutdownCoordinator::new();
register_cache_shutdown(&coord, backend.clone()).unwrap();
coord.shutdown().await.unwrap();
```

## 💻 示例

参见 [examples/](../examples/) 目录获取更多使用示例：

| 目录 | 内容 |
|------|------|
| [01_basics](../examples/src/01_basics/) | 基础操作 |
| [02_advanced](../examples/src/02_advanced/) | 高级特性 |
| [03_config](../examples/src/03_config/) | 配置 |
| [05_database](../examples/src/05_database/) | 数据库集成 |
| [06_features](../examples/src/06_features/) | 特性演示 |
