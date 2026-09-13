# 🏗️ Oxcache 架构文档

本文档描述 Oxcache 库（v0.5.0-rc.4）的架构、设计决策和技术细节。

## 📋 目录

<details open>
<summary>📑 目录</summary>

- [🧭 概述](#-概述)
- [🧱 总体架构](#-总体架构)
- [🧩 组件](#-组件)
  - [1. 内部缓存注册表](#1-内部缓存注册表)
  - [2. 缓存接口](#2-缓存接口)
  - [3. 后端层](#3-后端层)
  - [4. 特性模块](#4-特性模块)
  - [5. 基础设施模块](#5-基础设施模块)
  - [6. 安全模块](#6-安全模块)
  - [7. 键生成器](#7-键生成器)
  - [8. 事件模块](#8-事件模块)
  - [9. 配置与注册模块](#9-配置与注册模块)
- [🔀 数据流](#-数据流)
- [📐 一致性模型](#-一致性模型)
- [🧯 故障处理](#-故障处理)
- [⚡ 性能优化](#-性能优化)
- [🔒 安全](#-安全)
- [📈 可扩展性](#-可扩展性)
- [🎨 特性标志](#-特性标志)
- [🔮 未来增强](#-未来增强)
- [📚 参考资料](#-参考资料)

</details>

## 🧭 概述

Oxcache 是一个多级缓存系统，专为高性能、生产就绪的应用设计。它整合了：

- **L1 缓存**：使用 Moka（LRU/TinyLFU 淘汰）或 DashMap 的内存缓存
- **L2 缓存**：使用 Redis（Standalone/Sentinel/Cluster）、Valkey、Dragonfly 或 Aerospike 的分布式缓存
- **ChainCache**：按分数排序的多后端缓存链，支持回填与竞速读取
- **同步 API**：异步 API 的同步镜像（`get_sync` / `set_sync` / …），用于非异步调用场景
- **布隆过滤器**：可选装饰器，在负查询到达内部后端前短路返回
- **单条目 TTL**：所有后端统一支持 `ttl` / `expire` 操作
- **穿透与击穿防护**：单飞去重（64 分片）、空值哨兵、TTL 抖动、布隆负查询短路
- **跨实例失效**：`invalidation` 特性经 Redis Pub/Sub 广播失效事件，键空间通知第二通道
- **版本化 CAS**：`versioning` 特性提供内存与 Redis WATCH/MULTI/EXEC 两种 `compare_and_swap` 实现
- **自动降级**：`degradation` 特性三态状态机（Active/Degraded/HalfOpen），故障时降级 L1-only 并自动探测恢复

> **说明**：文档历史版本中引用的 WAL（Write-Ahead-Log）恢复层不存在于当前代码库中，持久性委托给 Redis 后端本身；跨实例失效层则以 `invalidation` 特性形式在 0.5.0-rc.4 落地。

### 设计目标

1. **性能**：L1 延迟 50-100ns，L2 延迟 1-5ms（P99，随环境变化）
2. **可靠性**：后端 trait 层级让调用方在 L2 不可达时优雅降级
3. **易用性**：通过 `#[cached]` 宏实现零模板代码集成
4. **可观测性**：指标（`CacheStats`、Prometheus/JSON 导出）、`telemetry` 遥测、健康检查
5. **安全性**：Redis 键 / Lua 脚本 / SCAN 模式的输入校验，敏感数据脱敏

## 🧱 总体架构

Oxcache 采用「统一接口 + 可插拔后端」的分层设计。应用只面对 `Cache<K, V>` 一个类型安全入口，序列化（`infra::serialization`）与指标（`infra::metrics`）横切其后。所有读写最终落到实现 `CacheReader` / `CacheWriter` / `CacheConnector` 三个 trait 的后端上，blanket impl 自动将其组合为 `CacheBackend`。`#[cached]` 宏由独立的 `oxcache_macros` crate 提供，经 `internal::MACRO_CACHES` 注册表把函数调用接入同一套缓存路径。

```mermaid
flowchart TD
    APP["应用代码"] --> CACHE
    MACRO["oxcache_macros<br/>cached 属性宏"] --> REG["internal<br/>MACRO_CACHES 注册表"]
    REG --> CACHE["cache<br/>Cache / CacheBuilder / ChainCache"]
    CACHE --> BACKEND["backend<br/>CacheReader / CacheWriter / CacheConnector"]
    CACHE --> INFRA["infra<br/>serialization / metrics"]
    CACHE --> SEC["security<br/>输入校验 / 脱敏"]
    CACHE --> UTILS["utils<br/>KeyGenerator"]
    CACHE --> ERR["error<br/>OxCacheError"]
    BACKEND --> MEM["memory<br/>Moka / DashMap"]
    BACKEND --> DIST["redis / dragonfly / aerospike"]
    BACKEND --> FEATS["features<br/>bloom_filter / dist_lock / encryption / invalidation"]
```

`batch`（缓冲写入）、`integrations::kit`（生命周期集成）、`i18n`、`config`、`traits`、`testing` 等模块按特性门控挂载。

## 🧩 组件

### 1. 内部缓存注册表

**位置**：`src/internal.rs`

**职责**：缓存实例的中央注册表，供 `#[cached]` 宏使用。

**数据结构**：

```rust
type MacroCacheMap = Mutex<HashMap<String, Arc<Cache<String, Vec<u8>>>>>;
static MACRO_CACHES: once_cell::sync::OnceCell<MacroCacheMap> = ...;
```

注册表存储**具体的 `Cache<String, Vec<u8>>` Arc 句柄**（非 trait 对象），以服务名为键。使用 `Mutex<HashMap<…>>` 而非 `DashMap`。

**公共内部函数**（仅两个）：

- `__internal_register_cache(name, cache: Arc<Cache<String, Vec<u8>>>)` — 注册/覆盖某服务的缓存（同步，互斥锁中毒时为 no-op）
- `__internal_get_cache(name) -> Option<Arc<Cache<String, Vec<u8>>>>` — 按服务名获取缓存（同步）

两者均从 `oxcache::internal` 重导出，`oxcache::__internal_get_cache` 在 crate 根重导出供宏生成代码使用。

**线程安全**：互斥锁仅在 map 变更/查找期间持有（持锁期间无 await）。

**使用模式**：

```rust
use oxcache::Cache;

// 构建缓存实例
let cache: Cache<String, Vec<u8>> = Cache::builder().build().await?;

// 方式一：经 internal 模块注册（供 #[cached] 宏使用）
oxcache::internal::__internal_register_cache("my_service", Arc::new(cache.clone()));

// 方式二：使用 Cache 上的便捷方法
cache.register_for_macro("my_service").await?;

// 宏生成的代码从注册表获取缓存：
#[cached(service = "my_service", ttl = 300)]
async fn get_user(id: u64) -> User { /* ... */ }
```

### 2. 缓存接口

**位置**：`src/cache/`

**职责**：统一的类型安全缓存接口。

**模块结构**：

- `cache/mod.rs` — 模块根和重导出
- `cache/builder/` — `CacheBuilder` 实现
- `cache/api/` — 缓存操作实现（`basic_ops`、`batch_ops`、`bytes_ops`、`macros` 等）
- `cache/chain.rs` + `cache/chain/` — `ChainCache`、`ChainLink`、`ChainCacheBuilder`
- `cache/interface.rs` — `UnifiedCache` trait
- `cache/typed_namespace.rs` — `TypedNamespace` 类型化命名空间隔离

**关键类型**：

- `Cache<K, V>`：主缓存类型，泛型键（`K: CacheKey`）和值（`V: Serialize + DeserializeOwned`）
- `CacheBuilder<K, V>`：用于创建已配置缓存实例的构建器
- `ChainCache` / `ChainLink` / `ChainCacheBuilder`：多级缓存链（按分数排序）
- `L1Builder` / `L2Builder` / `ChainBuilder`：分层构建器 API（与 `CacheBuilder`/`ChainCacheBuilder` 并存）

`Cache<K, V>` 的构造方法（`builder` / `new` / `memory` / `redis` / `with_dependencies`）、`CacheBuilder` 全部方法与「`CacheBuilder` 不暴露 `.redis(...)` / `.tiered(...)` 等方法，组合多后端用 `ChainCache` + `.backend_arc(...)`」的说明见 [API 参考](API_REFERENCE.md#-cachebuilder)。要点：`sync_mode(true)` 不能与 `backend_arc(Arc<dyn CacheBackend>)` 组合，两者同时设置时 `build()` 返回 `Err(OxCacheError::NotSupported)`（OXCACHE_009）——这是 stable Rust 上 `trait_upcasting` 的临时限制。

关键异步方法（`get` / `set` / `set_with_ttl` / `get_by_str` / `set_by_str` / `delete` / `exists` / `clear` / `get_or` / `ttl` / `expire` / `get_bytes` / `set_bytes` / 生命周期与统计 / `register_for_macro`）与同步方法（`get_sync` / `set_sync` / `set_with_ttl_sync` / `delete_sync` / `exists_sync` / `clear_sync` / `ttl_sync` / `expire_sync` / `get_or_sync`）的逐项签名见 [API 参考](API_REFERENCE.md#-cachek-v) 与 [API 参考的同步 API 章节](API_REFERENCE.md#-同步-api)。

**线程安全**：所有操作通过 `Arc<dyn CacheBackend>`（同步路径为 `Option<Arc<dyn SyncCacheBackend>>`）保证线程安全。

**使用模式**：

```rust
use oxcache::Cache;
use std::time::Duration;

// 1) 简单内存缓存（默认）
let cache: Cache<String, User> = Cache::builder()
    .ttl(Duration::from_secs(3600))
    .capacity(10000)
    .build()
    .await?;

// 2) Redis 缓存（强制 TLS，除非设置了 OXCACHE_ALLOW_INSECURE_REDIS）
let cache: Cache<String, User> = Cache::redis("rediss://localhost:6379").await?;

// 3) 注入任意自定义后端
let backend: Arc<dyn oxcache::backend::CacheBackend> = /* ... */;
let cache: Cache<String, User> = Cache::builder()
    .backend_arc(backend)
    .ttl(Duration::from_secs(3600))
    .build()
    .await?;

// 4) 同步 API（Moka 需要 multi_thread tokio 运行时）
let cache: Cache<String, String> = Cache::builder()
    .sync_mode(true)
    .build()
    .await?;
cache.set_sync(&"k".to_string(), &"v".to_string())?;
let v = cache.get_sync(&"k".to_string())?;

// 注册供 #[cached] 宏使用
cache.register_for_macro("my_service").await?;
```

### 3. 后端层

**位置**：`src/backend/`

**职责**：可插拔的缓存后端实现，遵循 ISP 合规 trait 层级。

**模块结构**：

- `backend/mod.rs` — 模块根和重导出
- `backend/interface.rs` — `CacheReader` / `CacheWriter` / `CacheConnector` / `CacheBackend` 及其同步镜像；`AtomicCacheWriter` / `SyncAtomicCacheWriter` 用于原子操作
- `backend/memory/` — 内存后端实现（Moka、DashMap）及 Redis/Valkey 客户端
- `backend/dragonfly/` — Dragonfly 后端（基于 Redis 协议包装）
- `backend/aerospike/` — Aerospike 后端（独立协议，feature-gated）
- `backend/score.rs` — `BackendScore` / `Scores` 常量供 `ChainCache` 使用
- `backend/config_validation.rs` — Redis/Valkey URL / Sentinel 配置校验
- `backend/factory.rs` — `BackendFactory` / `BackendRegistry` / `BackendSpec` 后端工厂注册中心

**后端类型**（重导出在 `oxcache::backend::*`，部分在 crate 根）：

| 类型 | 路径 | 特性 | 说明 |
|------|------|------|------|
| `MokaMemoryBackend` | `oxcache::backend::MokaMemoryBackend` | `memory` | L1 缓存，Moka（LRU/TinyLFU） |
| `DashMapMemoryBackend` | `oxcache::backend::DashMapMemoryBackend` | `memory` | 纯内存并发缓存，FIFO O(1) 淘汰 |
| `RedisBackend` | `oxcache::backend::RedisBackend` | `redis` | L2 分布式缓存（Redis/Valkey） |
| `RedisBackendBuilder` | `oxcache::backend::RedisBackendBuilder` | `redis` | Redis/Valkey 构建器（模式、连接池、TLS） |
| `DragonflyBackend` | `oxcache::backend::DragonflyBackend` | `dragonfly` | Dragonfly 缓存（Redis 协议兼容） |
| `AerospikeBackend` | `oxcache::backend::AerospikeBackend` | `aerospike` | Aerospike 持久化 KV 存储 |
| `ChainCache` | `oxcache::cache::chain::ChainCache` | — | 按分数排序的多后端缓存链 |
| `BloomFilterBackend` | `oxcache::features::bloom_filter::BloomFilterBackend` | `bloom` | 负查询过滤装饰器 |

**异步 Trait 层级**（`backend/interface.rs`）：

```rust
#[async_trait]
pub trait CacheReader: Send + Sync + 'static {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>>;
    async fn exists(&self, key: &str) -> OxCacheResult<bool>;
    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>>;
    async fn len(&self) -> OxCacheResult<u64>;
    async fn is_empty(&self) -> OxCacheResult<bool> { /* 默认实现 */ }
    async fn capacity(&self) -> OxCacheResult<u64>;
    async fn stats(&self) -> OxCacheResult<HashMap<String, String>>;
    async fn get_many(&self, keys: &[String]) -> OxCacheResult<Vec<Option<Vec<u8>>>> { /* 默认 */ }
    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> { /* 默认：空 Vec */ }
}

/// 批量写入条目类型：`(Arc<str> key, Arc<Vec<u8>> value, Option<Duration> ttl)`
pub type CacheSetItem = (Arc<str>, Arc<Vec<u8>>, Option<Duration>);

#[async_trait]
pub trait CacheWriter: Send + Sync + 'static {
    async fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()>;
    async fn delete(&self, key: &str) -> OxCacheResult<()>;
    async fn clear(&self) -> OxCacheResult<()>;
    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool>;
    async fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> { /* 默认 */ }
    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> { /* 默认 */ }
}

#[async_trait]
pub trait CacheConnector: Send + Sync + 'static {
    async fn health_check(&self) -> OxCacheResult<()>;
    async fn shutdown(&self);
    fn backend_kind(&self) -> BackendKind;
    #[cfg(feature = "lua")]
    fn as_lua_executor(&self) -> Option<&dyn LuaExecutor> { None }
    fn as_atomic_writer(&self) -> Option<&dyn AtomicCacheWriter> { None }
}

#[async_trait]
pub trait CacheBackend: CacheReader + CacheWriter + CacheConnector + 'static {}
// blanket 实现：满足三个超级 trait 的任意 T 自动成为 CacheBackend。
```

**同步 Trait 层级**（镜像异步版本，无 `async`/`#[async_trait]`）：

```rust
pub trait SyncCacheReader: Send + Sync + 'static { /* 同步 fn */ }
pub trait SyncCacheWriter: Send + Sync + 'static { /* 同步 fn */ }
pub trait SyncCacheConnector: Send + Sync + 'static { /* 同步 fn */ }
pub trait SyncCacheBackend: SyncCacheReader + SyncCacheWriter + SyncCacheConnector + 'static {}
```

后端通过额外实现同步 trait 来选择加入同步 API。`Cache<K, V>::get_sync` 通过 `Arc<dyn SyncCacheBackend>` 分发。**异步和同步层级有意分离**，使后端可以只支持其一，且保持异步 trait 对象的对象安全（异步热路径上无 `block_in_place`）。

**AtomicCacheWriter**（独立 trait，非 `CacheWriter` 的超级 trait）：

```rust
#[async_trait]
pub trait AtomicCacheWriter: Send + Sync + 'static {
    async fn incr(&self, key: &str, delta: i64, ttl: Option<Duration>) -> OxCacheResult<i64>;
    async fn compare_and_swap(&self, key: &str, expected: Option<&[u8]>, new: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<bool>;
    async fn set_if_absent(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<bool>;
}

pub trait SyncAtomicCacheWriter: Send + Sync + 'static { /* 同步镜像 */ }
```

后端通过 `CacheConnector::as_atomic_writer()` 暴露原子能力。`Cache<K,V>::incr()` / `compare_and_swap()` / `set_if_absent()` 通过此运行时发现方法委托；后端缺少原子支持时返回 `Err(NotSupported)`。`versioning` 特性另提供带版本信封的 `MemoryVersionedCache` / `RedisVersionedCache`（WATCH/MULTI/EXEC）实现。

**`BackendKind` 枚举**（`Moka | DashMap | Redis | Valkey | Dragonfly | Aerospike | Chain | Mock | Unknown`）由 `backend_kind()` 返回，用于运行时标识而无需 `as_any()`。

**ChainCache 读取路径**：

```
1. 从最高分（L1）到最低分（L2）遍历 ChainLink
2. 命中 → 返回值
3. 未命中（None 或 Err）→ 记录警告并继续下一个链接
   （L1 读取失败优雅降级而非使请求失败）
4. 在非最高分链接命中后，如启用回填，
   异步填充更高分（更靠近 L1）的链接
   （fire-and-forget tokio::spawn，失败仅警告不传播）
```

启用 `enable_race_read()`（选择加入）后，读取路径改为**并发**查询**所有**
后端（`JoinSet`）并返回首个命中；非最高分命中时仍执行回填，
仅当所有后端都失败时读取才报错。

**ChainCache 写入路径**：

```
1. 并发（tokio::spawn / JoinSet）写入所有分数
   <= 写入者阈值的链接（通常所有非持久化 + 持久化写入者）
2. 持久化后端接收写入以保证持久性
3. 单链接失败仅记录日志并容忍；仅当所有后端都失败时
   写入才报错
4. 无 WAL（持久性委托给 Redis 后端本身）
```

**ChainCache 健康检查**：每个链接并发 ping，每个后端 5 秒超时；仅当所有链接都失败时 `health_check()` 才失败。

### 4. 特性模块

**位置**：`src/features/`

**职责**：可选能力和运行时特性信息。

**关键项**：

- `features::bloom_filter::BloomFilter` — 概率数据结构（`new(capacity, fpr)`、`insert`、`contains`、`clear`、`len`、`is_empty`）
- `features::bloom_filter::BloomFilterBackend` — 装饰器，包装任意 `CacheBackend`；`get` 时如键不在布隆过滤器中则直接返回 `Ok(None)`
- `features::dist_lock` — Redis 分布式锁（watchdog 续期、可重入）；`red-lock` 提供 `RedLock` 多节点多数派锁与 fencing token
- `features::encryption` — `EncryptedBackend` 值级 XChaCha20-Poly1305 加密装饰器
- `features::invalidation` — 跨实例失效总线（Redis Pub/Sub 广播 + 键空间通知）
- `features::degradation` — `DegradableBackend` 三态自动降级装饰器
- `features::compression` — `CompressingBackend` 自适应 zstd 压缩装饰器
- `features::audit` — `AuditEventPublisher` 结构化审计事件流
- `features::versioning` — 版本化 CAS 实现
- `features::confers_config` — confers 配置驱动构建（`OxcacheConfig` + `ConfigBus` 热更新）
- `get_l1_feature_info() / get_l2_feature_info() / get_all_feature_info()`
- `is_l1_enabled() / is_l2_enabled()`

### 5. 基础设施模块

**位置**：`src/infra/`

**职责**：指标、序列化和键校验工具。

**子模块**：

- `infra/metrics/backend.rs` — `MetricsCollector`、`LatencyHistogram`、`OperationCounter`、`FullMetrics`
- `infra/metrics/snapshot.rs` — `CacheStats`
- `infra/metrics/export.rs` — `export_json_format`、`export_prometheus_format`、`get_enhanced_stats`
- `infra/metrics/unified.rs` — `UnifiedMetrics`、原子计数器、直方图数据
- `infra/serialization/` — JSON 序列化器（`JsonSerializer`）、`UnifiedSerializer` 与二进制格式（`SerializationFormat`：Json/Bincode/Postcard）
- `infra::validate_cache_key(key)` — 键校验便捷方法

**Crate 根重导出**（启用 `metrics` 特性时）：

```rust
pub use infra::{export_json_format, export_prometheus_format, export_prometheus_standard, get_enhanced_stats, CacheStats};
```

**重要**：`MetricsCollector` **不**在 crate 根重导出。它位于 `oxcache::infra::metrics::backend::MetricsCollector::new() -> OxCacheResult<Self>`。指标注入端口的抽象为 `MetricsRecorder`（crate 根重导出，含 `NoOpMetricsRecorder` 默认实现）。

**序列化**：默认 **JSON**（`serialization` 特性引入 `serde` + `serde_json`）；`serde-bincode` / `postcard` 特性启用对应二进制格式，经 `CacheBuilder::serialization_format()` 切换，同一键前缀禁混格式。序列化在 `infra/serialization/` 中实现（`json.rs`、`binary.rs`、`unified.rs`、`utils.rs`、`depth_limited.rs`）：

- **`JsonSerializer`** — 异步/同步后端侧序列化器（值为 `Vec<u8>`）；`with_compression()` 启用 flate2 gzip 输出。
- **`UnifiedSerializer`** — 值级编解码（`serialize<T>` / `deserialize<T>` / `estimate_size`），供 `Cache<K, V>` 和 `#[cached]` 宏使用。
- **`depth_limited.rs`** — `deserialize_safe` 防止深层嵌套 JSON DoS：禁用 `serde_json` 递归限制（`unbounded_depth` 特性），流通过 `serde_stacker` 包装为基于堆栈的递归。`MAX_JSON_DEPTH` 常量加 64 MiB 反序列化大小上限和 gzip 魔数头检测加固了该路径。

线上无 base64 往返：值以原始字节传递（可选压缩）。

### 6. 安全模块

**位置**：`src/security/`

**职责**：输入校验和敏感数据脱敏。模块本身为 `pub(crate)` — 调用方必须使用 **crate 根重导出**。

**子模块**：

- `security/validation.rs` - Redis 键、Lua 脚本、SCAN 模式校验
- `security/redaction.rs` - 敏感数据脱敏（`Redacted` 包装器）
- `security/log.rs` - 安全日志工具
- `security/regex.rs` - 模式匹配

**Crate 根重导出**（启用 `redis` 或 `full` 特性时）：

```rust
pub use crate::security::{
    clamp_scan_count,
    log::{log_cache_key, sanitize_message},
    redaction::{redact_cache_key, redact_connection_string, redact_field, redact_value, Redacted},
    validate_lua_script, validate_redis_key, validate_scan_pattern,
};
```

> **导入路径说明**：`oxcache::security::*` 是**无效**路径（模块为 `pub(crate)`）。直接使用 `oxcache::validate_redis_key(...)` 等。函数名为 `validate_redis_key`（非 `validate_key`）。安全设计的完整说明见[安全文档](SECURITY.md)。

### 7. 键生成器

**位置**：`src/utils/`

**职责**：缓存键生成和管理。

**关键类型**：

- `KeyGenerator`：用于生成带命名空间和前缀的缓存键的工具（重导出在 `oxcache::KeyGenerator`）

**关键方法**：

- `new()`：创建默认键生成器
- `with_namespace(ns)`：设置命名空间用于键隔离
- `with_prefix_str(prefix)`：设置前缀用于键组织
- `generate(template, params)`：从模板生成键
- `generate_full(template, params)`：使用命名空间和前缀生成键
- `validate_key(key)`：校验键格式

**使用模式**：

```rust
use oxcache::KeyGenerator;

let generator = KeyGenerator::new()
    .with_namespace("myapp")
    .with_prefix_str("cache");

let key = generator.generate_full("user:{id}", &[("id", "123")]);
// 结果："myapp:cache:user:123"
```

### 8. 事件模块

**位置**：`src/core/events.rs`

**职责**：缓存事件系统，用于监控和钩子。

**关键类型**（重导出在 crate 根）：

- `CacheEventType`：事件类型枚举（`Hit`、`Miss`、`Set`、`Delete`、`Expire`、`Clear`、`Get`、`BatchStart`、`BatchEnd`、`Error`、`Connect`、`Disconnect`、`Custom(String)`）
- `CacheEvent`：事件数据结构（builder 模式构造）
- `EventPublisher`：事件发布 trait（提供 `NoopPublisher` 供测试使用）

**`CacheEvent` API**（builder 模式）：

```rust
let event = CacheEvent::new(CacheEventType::Hit)
    .with_key("user:123")
    .with_latency(15)
    .with_metadata("source", "l1");
```

字段：`event_type`、`key: Option<String>`、`timestamp: u64`（毫秒）、`latency_ms: Option<u64>`、`error: Option<String>`、`metadata: Vec<(String, String)>`。

### 9. 配置与注册模块

**位置**：`src/config/`、`src/registry.rs`、`src/backend/factory.rs`

- **`config/`**：私有模块（`mod config`），承载分布式参数类型（`DistributedConfig`：重试策略、熔断阈值、健康检查间隔）。公开的配置入口是 `CacheBuilder` 与 `RedisBackendBuilder`；`config-confers` 特性另提供 confers 加载的 `OxcacheConfig`（容量/TTL/熔断参数）与 `ConfigBus` watch 热更新。
- **`registry.rs`**：全局缓存注册表（`init` / `register` / `get` / `remove` / `clear`），供显式管理多个命名缓存实例。
- **`backend::factory`**：`BackendRegistry` 后端工厂注册中心，按名注册/构建后端（内置 moka/dashmap/memory，feature 门控 redis），serde 友好的 `BackendSpec`，供 kit 等动态选择后端。

## 🔀 数据流

### #[cached] 宏执行路径

`#[cached]` 宏通过自动处理缓存查找、存储和序列化实现零模板代码缓存。有效的宏参数为：`service`、`ttl`、`key`、`key_prefix`、`sync`、`skip_cache_write`、`single_flight`、`strict`、`condition`、`cache_none`（不存在 `key_generator` 或 `cache_type` 参数）。

```mermaid
sequenceDiagram
    participant App as 应用
    participant Gen as 宏生成代码
    participant Reg as MACRO_CACHES 注册表
    participant Cache as Cache
    participant BE as CacheBackend

    App->>Gen: 调用被标注函数
    Gen->>Reg: 按服务名查找缓存
    Reg-->>Gen: 返回缓存实例
    Gen->>Gen: 生成缓存键
    Gen->>Cache: get_bytes key
    Cache->>BE: get
    BE-->>Cache: Option bytes
    alt 缓存命中
        Cache-->>Gen: 字节值
        Gen->>Gen: JSON 反序列化
        Gen-->>App: 返回缓存值
    else 缓存未命中
        Gen->>Gen: 执行原函数
        Gen->>Gen: JSON 序列化结果
        Gen->>Cache: set_bytes key bytes ttl
        Cache->>BE: set
        Gen-->>App: 返回函数结果
    end
```

**宏生成代码结构**：

```rust
#[cached(service = "my_service", ttl = 300)]
async fn get_user(id: u64) -> OxCacheResult<User> {
    // ... 原始函数体 ...
}
```

展开后大致为：

```rust
async fn get_user(id: u64) -> OxCacheResult<User> {
    let cache_key = format!("my_service:get_user:{:?}", id);

    // 从注册表获取缓存（同步查找）
    let client = match oxcache::__internal_get_cache("my_service") {
        Some(c) => c,
        None => return { /* 原始代码 */ }.await,
    };

    // 尝试缓存命中（JSON 反序列化为 User）
    if let Ok(Some(bytes)) = client.get_bytes(&cache_key).await {
        if let Ok(val) = serde_json::from_slice::<User>(&bytes) {
            return Ok(val);
        }
    }

    // 执行原始函数
    let result = { /* 原始代码 */ }.await;

    // 成功时缓存结果
    if let Ok(ref val) = result {
        if let Ok(bytes) = serde_json::to_vec(val) {
            let _ = client.set_bytes(&cache_key, bytes, Some(Duration::from_secs(300))).await;
        }
    }

    result
}
```

未注册服务时默认静默穿透执行原函数，`strict` 模式改为 panic；`condition` 谓词返回 false 时旁路缓存。

### ChainCache 读取路径

```mermaid
flowchart TD
    A["Cache 读请求"] --> B["ChainCache 从最高分链接开始"]
    B --> C{"最高分链接命中？如 L1 Moka"}
    C -->|命中| D["返回值"]
    C -->|未命中或出错| E{"下一个链接命中？如 L2 Redis"}
    E -->|命中| F{"已启用回填？"}
    F -->|是| G["异步回填更高分链接"]
    F -->|否| D
    G --> D
    E -->|未命中或出错| H["返回 None"]
```

读取、写入与回填的逐步语义（含 `enable_race_read()` 竞速读取）见[后端层的 ChainCache 路径](#3-后端层)。

### 写操作（配合 #[cached] 宏）

```mermaid
flowchart TD
    A["应用<br/>cached 函数"] --> B["执行函数"]
    B --> C{"结果 Ok？"}
    C -->|否| D["返回错误"]
    C -->|是| E["序列化结果"]
    E --> F["set_bytes 到缓存<br/>带 TTL"]
    F --> L["返回结果"]
```

> **说明**：`#[cached]` 在注册的 `Cache<String, Vec<u8>>` 上调用 `set_bytes`，由其分发到所接入的后端（内存、Redis、ChainCache 或 BloomFilterBackend）。如果后端是 `ChainCache`，链按分数/回填策略内部处理 L1+L2 写入。

## 📐 一致性模型

### 单实例一致性

在单个 `Cache<K, V>` 实例内，配置的后端决定一致性：

- **仅 Moka / DashMap**：进程内强一致性。无跨实例协调。
- **仅 Redis**：通过 Redis 单线程命令执行实现强一致性。
- **ChainCache（Moka + Redis）**：进程内读后写一致性。L1 缓存同步更新；L2（Redis）写入在 `set` 返回前完成。如启用回填，L2 命中会异步填充 L1。

### 跨实例一致性

默认配置下，oxcache **不**自动做跨实例 L1 失效，多实例一致性由应用负责。两种路径：

**内建路径（`invalidation` 特性）**：

- Redis Pub/Sub 广播失效事件（key / namespace 粒度），各实例后台监听并失效本地 L1；消息带 `instance_id`，写实例自身免除自失效
- `KeyspaceNotificationListener` 键空间通知第二通道：把外部 `DEL`/`EXPIRED` 投影为本地失效（需 Redis `notify-keyspace-events "Egx"`）
- `InvalidatingBackend` 写路径装饰器透明接入

**应用自管路径**（不启用该特性时）：

- 使用 Redis 作为唯一真实来源（跳过 L1，或接受 L1 的短暂过期数据）
- 使用短 L1 TTL 限制数据过期程度
- 应用外部失效（如 Redis keyspace 通知、应用层 Pub/Sub）并在每个实例上调用 `cache.delete(key)`

对读-改-写竞争，`versioning` 特性提供 `compare_and_swap(key, expect_version, new_value)`（内存实现 + Redis WATCH/MULTI/EXEC 实现）。

### 单飞（缓存击穿保护）

`get_or`（异步）和 `get_or_sync`（同步）都实现单飞：当多个并发调用未命中同一键时，仅第一个调用者（"领导者"）执行 fallback；跟随者在 `tokio::sync::Notify`（异步）或 `std::sync::Condvar`（同步）上阻塞，直到领导者写入结果。panic 安全守卫确保即使领导者 panic 跟随者也能被释放。

飞行中注册表**分片**（64 个哈希桶通过 `DefaultHasher`）以减少并发下跨键锁竞争：`get_or_shard_index(key)` 将每个键路由到一个分片，每个分片拥有 `Mutex<HashMap<String, Arc<Notify>>>`（异步）/ `Mutex<HashMap<String, SyncFlight>>`（同步）。

## 🧯 故障处理

### Redis 故障

**检测**（通过 `CacheConnector::health_check()`）：

- 连接超时 / 拒绝
- `PING` 失败
- 连接被远端关闭

**恢复（应用驱动 + 可选自动降级）**：

1. `Cache::health_check().await` 返回 `Err(OxCacheError::*)` — 调用方可切换到回退代码路径
2. `ChainCache` 即使 L2 链接报错仍继续提供 L1 命中（未命中仅传播为 `None`）；L1 读取错误被记录并穿透到下一链接；L2 写入失败被记录，仅当所有后端都失败时写入才向调用方报错
3. **`degradation` 特性**：`DegradableBackend` 保护 L2，故障计数超阈值自动降级为 L1-only（返回 `Degraded` 错误供 ChainCache 回落），降级超时后进入半开探测，探测成功自动恢复、失败重新降级；状态变化回调 + 全局 degraded 指标
4. 应用也可以在缓存外包裹自己的熔断器/重试策略

重连后无 WAL 重放机制（WAL 层不存在于当前代码库）。

### 网络分区

- 每个实例继续使用本地 L1 缓存运行
- Redis 写入/读取将失败并表现为 `Err(OxCacheError::*)`
- 恢复后无自动数据协调，应用可发出 `cache.clear()` 或依赖 TTL 过期；启用 `invalidation` 时，恢复后的写事件会继续经 Pub/Sub 驱动各实例失效

### 后端 Trait 错误

所有后端错误通过 `OxCacheError` 传递（参见 `src/error.rs`）。完整错误码表（OXCACHE_001–OXCACHE_024）见 [API 参考](API_REFERENCE.md#-错误处理)。关键变体：

| 错误码 | 变体 | 含义 |
|--------|------|------|
| OXCACHE_001 | `NotFound` | 键未找到 |
| OXCACHE_002 | `Connection` | 网络连接失败 |
| OXCACHE_003 | `Serialization` | 序列化/反序列化失败 |
| OXCACHE_005 | `Degraded` | 缓存处于降级模式 |
| OXCACHE_007 | `L2Error` | L2（Redis）后端错误 |
| OXCACHE_009 | `NotSupported` | 此后端/配置不支持的操作 |
| OXCACHE_015 | `Timeout` | 操作超时 |
| OXCACHE_020 | `InvalidInput` | 错误的键/值/配置输入 |

## ⚡ 性能优化

### 优化技术

1. **Moka 单条目 TTL**：使用 `moka::Expiry` trait 实现真正的单条目 TTL（覆盖构建器的全局 TTL），避免独立过期跟踪的开销。
2. **连接池**：`RedisBackend::with_pool(url, pool_size)` 复用 Redis 连接。
3. **Pipeline / 批量操作**：`RedisBackend` 支持通过 Redis pipeline 的 `set_many` / `delete_many` / `get_many`（默认 trait 实现循环，但 `RedisBackend` 覆盖它们）。`batch` 特性添加 `BatchWriter` 缓冲 L2 写入（容量/时间双阈值刷盘）。
4. **无锁 L1**：Moka 的并发缓存设计（TinyLFU 准入、LRU 淘汰）。
5. **可插拔序列化**：默认 JSON；`serde-bincode` / `postcard` 二进制格式可显著降低 L2 传输体积（见[性能基线](PERFORMANCE.md)）。`MAX_JSON_DEPTH` 常量防止深层嵌套 JSON DoS。
6. **自适应压缩**：`compression` 特性的 `CompressingBackend` 按阈值触发 zstd，读取按魔数识别并兼容旧 gzip。
7. **单飞**：`get_or` / `get_or_sync` 对每键的并发 fallback 执行去重，防止缓存击穿惊群效应。
8. **布隆过滤器短路**：`BloomFilterBackend` 在键不在过滤器中时直接返回 `Ok(None)` 而不触及内部后端，适用于高负查询比工作负载。
9. **热路径借用键 API**：`get_by_str` / `set_by_str` 消除热路径上的多余分配（实测 get -6.7%、set -12.7%，见[性能基线](PERFORMANCE.md)）。

### 性能调优

```rust
use std::time::Duration;
use oxcache::Cache;

let cache: Cache<String, User> = Cache::builder()
    .capacity(10_000)              // L1 最大条目数
    .ttl(Duration::from_secs(600)) // 默认 TTL
    .tti(Duration::from_secs(300)) // 空闲 TTL（Moka）
    .build()
    .await?;

// Redis 连接池大小：
let redis = oxcache::backend::RedisBackend::with_pool(
    "rediss://localhost:6379", 16,
).await?;
```

### 基准测试结果

> 测试环境：M1 Pro，16GB RAM，macOS，Redis 7.0
>
> **注意**：性能因硬件、网络条件和数据大小而异。将这些视为数量级估计。

| 操作 | 吞吐量 | 延迟（P99） |
|------|--------|-------------|
| L1 读取 | 5-10M ops/sec | 50-100ns |
| L1 写入 | 2-5M ops/sec | 50-200ns |
| L2 读取 | 50-100K ops/sec | 1-5ms |
| L2 写入（批量） | 200-500K ops/sec | 1-10ms |

可复现的实测数据（序列化体积、压缩率、热路径 Criterion 基准）见[性能基线](PERFORMANCE.md)。

## 🔒 安全

### 威胁模型

1. **缓存穿透**：攻击者请求不存在的键 → 加载数据库
2. **缓存击穿**：热点键过期，大量请求同时打到数据库
3. **DoS 攻击**：高请求率压垮系统
4. **SQL 注入**：Redis 键中的恶意模式
5. **Lua 脚本注入**：Lua 脚本中的危险命令
6. **ReDoS**：恶意 SCAN 模式导致 CPU 耗尽
7. **深层嵌套 JSON DoS**：恶意 JSON 在反序列化时导致栈溢出

### 防御措施

上述威胁由多层防御缓解：单飞与空值哨兵/TTL 抖动（穿透与击穿防护）、`validate_redis_key` / `validate_lua_script` / `validate_scan_pattern` / `clamp_scan_count` 输入校验（含注释预处理防绕过）、`Redacted` 与 `redact_*` 系列脱敏、`MAX_JSON_DEPTH` + 64 MiB 上限 + 基于栈的递归、TLS 强制、`encrypt` / `integrity` 值保护。各机制的完整规则与配置摘要见[安全文档](SECURITY.md)，README 安全章节提供防线速览。

> **注意**：oxcache **无内置限流**。限流由应用或上游代理负责。

### 布隆过滤器穿透防护

`bloom` 特性的 `BloomFilterBackend` 在负查询到达内部后端前短路返回；机制图解、示例与穿透防护组合表见 [README 布隆过滤器章节](../README.md#-布隆过滤器与穿透防护)。

### 输入校验

`security` 模块（私有；通过 crate 根重导出消费，见 [API 参考](API_REFERENCE.md#-安全特性)）提供 `validate_redis_key` / `validate_lua_script` / `validate_scan_pattern` / `clamp_scan_count`，逐条校验规则与常量见[安全文档](SECURITY.md)。

### 最佳实践

1. **键设计**：使用稳定、可预测的键（使用 `KeyGenerator` 进行命名空间管理）
2. **TTL 策略**：根据数据易变性设置适当的 TTL；使用 `cache.ttl(&key)` 在保留 TTL 的更新工作流中读取已有 TTL
3. **访问控制**：使用 Redis AUTH + TLS（`rediss://` URL）
4. **监控**：通过 `get_enhanced_stats` / `export_prometheus_format` 跟踪 `CacheStats`（命中率、操作计数器、延迟直方图）
5. **同步 API**：为非异步调用场景启用 `sync_mode(true)`。在 `multi_thread` tokio 运行时上，Moka 的同步桥通过 `block_in_place` 复用当前运行时；否则它用 `noop` waker + 手动轮询驱动（运行时无关的）moka future，在任何运行时内外都安全，无全局运行时线程

## 📈 可扩展性

### 水平扩展

```mermaid
flowchart TD
    subgraph 应用实例
        I1["实例 1<br/>L1 Moka + L2 Redis"]
        I2["实例 2<br/>L1 Moka + L2 Redis"]
        I3["实例 3<br/>L1 Moka + L2 Redis"]
    end

    subgraph Redis层
        R["Redis 集群<br/>共享 L2"]
    end

    I1 --> R
    I2 --> R
    I3 --> R
```

每个实例维护自己的 L1（Moka）并共享 L2（Redis）。跨实例 L1 失效默认不自动，启用 `invalidation` 特性后经 Redis Pub/Sub 广播失效（参见[一致性模型](#-一致性模型)）。

### 垂直扩展

- 通过 `CacheBuilder::capacity(u64)` 增加 L1 容量（更多内存）
- 使用更快/专用的 Redis 实例
- 在 Redis 端启用 Redis 持久化（AOF + RDB）
- 通过 `RedisBackend::with_pool(url, pool_size)` 增加 Redis 连接池

### 分区

oxcache 不内置分区配置。应用可以通过将键路由到不同的 `Cache<K, V>` 实例（每个由不同的 Redis db / 集群支持）实现自己的分区；`TypedNamespace`（marker 类型 + `namespace!` 宏）提供编译期命名空间隔离。

## 🎨 特性标志

分层特性集（`minimal` / `core` / `full` 预设）与组件特性逐项说明见 [API 参考的特性要求](API_REFERENCE.md#-特性要求) 与 [README 特性标志](../README.md#-特性标志)。`bloom`、`kit` 及其余选择加入特性**不包含**在 `full` 中，需通过 `features = ["bloom"]` 等显式启用；`full` 的精确成员见 `Cargo.toml` 的 `[features]`。

## 🔮 未来增强

1. **自适应 TTL**：基于访问模式的 TTL 优化启发式
2. **地理分布**：多区域复制原语
3. **缓存预热**：智能预热策略
4. **`trait_upcasting` 迁移**：stable 后解除 `sync_mode + backend_arc` 互斥限制，使用户能注入自定义后端并同时使用同步 API

## 📚 参考资料

- [Moka 文档](https://github.com/moka-rs/moka)
- [Redis 文档](https://redis.io/docs/)
- [TinyLFU 论文](https://arxiv.org/abs/1512.00757)
- [布隆过滤器](https://en.wikipedia.org/wiki/Bloom_filter)
- [ISP 合规 trait 设计](https://en.wikipedia.org/wiki/Interface_segregation_principle)
- [📘 API 参考](API_REFERENCE.md) / [📖 用户指南](USER_GUIDE.md) / [📊 性能基线](PERFORMANCE.md)
