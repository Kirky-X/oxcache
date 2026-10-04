# 📘 Oxcache API 参考

> **⚠️ API 版本说明**
>
> 本文档描述 **Oxcache v0.5.0-rc.7** 的 API。

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
- [📢 Redis Pub/Sub 广播通道](#-redis-pubsub-广播通道pubsub-特性)
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
- **`full`**：全量预设（core + macros + compression + batch + lua + testing + dragonfly + aerospike + lock + offload + disk + stale；不含 `bloom`、`kit` 等选择加入特性）

### 组件特性

组件特性逐项说明（含默认值与是否包含在 `full` 中）见 [README 特性标志](../README.md#-特性标志)；关键门控：`bloom`、`kit` 等选择加入特性**不包含**在 `full` 中，配置示例同见该章节。

### 特性依赖

部分特性需要或隐含其他特性：

| 特性 | 所需特性 | 说明 |
|------|----------|------|
| `minimal` | `memory`, `metrics`, `serialization` | 最小预设（L1 memory + 指标） |
| `core` | `minimal`, `redis` | 核心 L1 + L2 缓存 |
| `full` | `core`, `macros`, `compression`, `batch`, `lua`, `testing`, `dragonfly`, `aerospike`, `lock`, `offload`, `disk`, `stale` | 全量预设（注意：`bloom`、`kit` 不在 `full` 中） |
| `memory` | `serialization` | backend 与 cache 核心 API 的编译基线 |
| `redis` | `serialization` | L2 Redis 后端 |
| `dragonfly` | `redis` | Dragonfly 兼容模式后端 |
| `aerospike` | — | Aerospike 后端（自管依赖） |
| `serialization` | — | 序列化门面（JSON 基线） |
| `serde-bincode` | `serialization` | bincode 二进制格式 |
| `postcard` | `serialization` | postcard 二进制格式 |
| `metrics` | `serialization`, `chrono`, `dashmap` | 内置指标 |
| `telemetry` | — | tracing 门面（重连/失效等日志路径） |
| `macros` | `minimal` | 属性宏 |
| `batch` | `memory` | BatchWriter（包装 `CacheBackend`；缓冲满默认自动刷盘，`reject_when_full(true)` 改为拒绝并返回 `BufferFull`/`OXCACHE_019`） |
| `lua` | `redis` | Lua 脚本执行 |
| `test-util` | — | 测试辅助（`#[cfg(feature)]` 专用桩） |
| `testing` | `test-util` | 测试实现 |
| `bloom` | — | 布隆过滤器（选择加入，不在 `full` 中） |
| `trait-kit` / `kit` | `memory` | kit 集成（引用 `crate::backend`；`kit` 隐含 `trait-kit`，选择加入，不在 `full` 中） |
| `lock` | `redis` | 分布式锁 |
| `redlock` | `lock` | RedLock 实现 |
| `red-lock` | `redlock` | RedLock 多节点多数派锁（别名） |
| `compression` | `memory` | 压缩装饰器（引用 backend 与序列化面） |
| `offload` | `dashmap` | 进程内后台任务执行器 |
| `disk` | `memory`, `serialization` | redb 嵌入式 L3 |
| `stale` | `memory`, `offload`, `serialization` | SWR 三态过期 |
| `invalidation` | `redis`, `serialization` | 跨实例失效广播总线 |
| `pubsub` | `redis` | Redis Pub/Sub 通用广播通道 |
| `encrypt` | `memory` | 加密装饰器（引用 `crate::backend`） |
| `integrity` | `encrypt`, `hmac`, `sha2` | 完整性签名层（模块寄生在 encryption 内） |
| `config-confers` | `memory`, `serialization` | 配置中枢 |
| `degradation` | `memory` | 降级装饰器（引用 `crate::backend`） |
| `audit` | — | 审计事件流 |
| `inklog` | `audit` | 审计事件 → inklog 结构化日志发布器 |
| `versioning` | — | 值版本化 |
| `hotkey` | `dashmap` | 热 key 采样观测 |
| `byte-weight` | `moka` | 按 byte 权重容量（moka 同步面） |
| `adaptive-ttl` | `memory` | 自适应 TTL 装饰器（引用 `crate::backend`） |
| `warmup` | `memory` | 智能预热（操作 `ChainCache`） |

上表「所需特性」列只列 cargo feature（`dep:` 依赖不重复罗列；`dashmap`/`moka`/
`hmac` 等词为对应可选依赖激活，非本 crate 特性）。`default` 预设即
`minimal`。

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
| `skip` | 参数名列表，如 `skip(password)` | 否 | — | 被点名参数不进入默认缓存 key；与 `key` 模板互斥（编译期报错）；未知参数名编译期报错 |
| `cache_none` | （标志） | 否 | `false` | 返回类型为 `Result<Option<T>, E>` 时生效：默认仅缓存 `Ok(Some)`（`Ok(None)` 不回写）；开启后 `Ok(None)` 以 `null` 缓存并在读取时还原。返回类型非 Option 时无效果 |

该宏通过 `oxcache::__internal_get_cache(service)` 从内部注册表获取 `Cache` 实例。
如果 `service` 下未注册缓存，则原始函数不经缓存直接执行（`strict` 模式改为 panic）。
使用 `sync` 标志时，`Cache` 必须通过 `sync_mode(true)` 构建。

**示例（异步）：**

```rust
// Cargo.toml: oxcache = { version = "0.5.0-rc.7", features = ["macros"] }
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
| `backend_arc` | `(backend: Arc<dyn CacheBackend>) -> Self` | 添加预构建的后端（仅 async 面） |
| `sync_backend_arc` | `(backend: Arc<dyn SyncCacheBackend>) -> Self` | 添加预构建的**原生同步**后端（配合 `sync_mode(true)` 启用同步 API） |
| `ttl` | `(ttl: Duration) -> Self` | 缓存条目的默认 TTL |
| `tti` | `(tti: Duration) -> Self` | 内存后端的默认 TTI（time-to-idle） |
| `capacity` | `(capacity: u64) -> Self` | 内存后端的容量（默认 10000） |
| `sync_mode` | `(enabled: bool) -> Self` | 启用同步 API（参见[同步 API](#-同步-api)） |
| `serialization_format` | `(format: SerializationFormat) -> Self` | 切换序列化格式（JSON/Bincode/Postcard，需 `serde-bincode`/`postcard`） |
| `null_cache_ttl` | `(ttl: Duration) -> Self` | 空值哨兵 TTL（穿透防护） |
| `ttl_jitter` | `(factor: f64) -> Self` | TTL 抖动系数（防批量同时过期） |
| `metrics` | `(recorder: Arc<dyn MetricsRecorder>) -> Self` | 注入 `MetricsRecorder`（覆盖纯 L1 路径指标） |
| `audit_publisher` | `(publisher: Arc<dyn AuditEventPublisher>) -> Self` | 注入审计事件发布器（`audit` 特性，hit/miss/set/delete 结构化事件、键脱敏）；内置实现：`NoOpAuditPublisher` / `InMemoryAuditPublisher`（有界环形）/ `TracingAuditPublisher`（`telemetry`）/ `InklogAuditPublisher`（`inklog`，事件→`LogRecord` 经有界通道（1024）由单一 writer task 保序写 `LogSink`，写操作 INFO/读探测 DEBUG；无 runtime/通道过载/通道关闭/关停滞留均计入 `dropped_count`，写失败计入 `write_failure_count`） |
| `build` | `async (self) -> Result<Cache<K, V>>` | 构建缓存实例（异步包装，内部无 await） |
| `build_sync` | `(self) -> Result<Cache<K, V>>` | 同步构建缓存实例（无需运行时） |

> **注意：** `CacheBuilder` 没有 `.redis(...)`、`.tiered(...)`、`.with_backend(...)`、
> `.batch_writes(...)` 或 `.auto_promote(...)` 方法。使用
> `.backend_arc(Arc::new(...))` 插入 `RedisBackend` 或其他后端。

**默认 Moka 路径**（未调用 `backend_arc` 时）：使用给定的 `capacity`/`ttl`/`tti`
构建 `MokaMemoryBackend`。`sync_mode(true)` 的三路配置见「同步 API 与显式后端」。

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

**同步 API 与显式后端：** `sync_mode(true)` 支持三种配置——默认 Moka 后端（原生
同步，运行时无关）；`sync_backend_arc(Arc<dyn SyncCacheBackend>)` 注入的原生同步
后端（同步 API 直连，异步 API 经 `SyncBackendAdapter` 门面呈现）；以及
`backend_arc(Arc<dyn CacheBackend>)`（具体类型的同步实现已被擦除，sync API 经通用
`AsyncToSyncBridge` 桥出——每个同步调用阻塞于 async 面，**要求调用时处于多线程
Tokio runtime**，runtime 之外或 current_thread runtime 上逐调用返回
`Err(OxCacheError::NotSupported)`）。需要运行时无关 sync API 的内存型后端请改用
`sync_backend_arc` 注入其原生同步面。

> **⚠️ 阻塞警告：** 两条路径存在阻塞语义——`SyncBackendAdapter` 的 async 方法
> 同步完成（包装 `RedisBackend` 等网络型同步后端时每次 async API 调用阻塞一个
> executor 线程）；`AsyncToSyncBridge` 的 sync 方法阻塞于 async 面（网络型后端
> 亦然）。网络型后端的推荐用法：经 `backend_arc(...)` 注入并走 async API；内存型
> （Moka / DashMap）不受影响。

## 🔌 后端层

`backend` 模块暴露后端 trait 和实现。

### 后端 Trait

后端 trait 层级（`CacheReader` / `CacheWriter` / `CacheConnector`，blanket impl 自动组合为 `CacheBackend`；批量条目类型 `CacheSetItem`；同步镜像 trait 与 `AtomicCacheWriter`）的完整定义见[架构文档的后端层章节](ARCHITECTURE.md#3-后端层)。

### 后端类型

后端类型、模块路径与所属特性见[架构文档](ARCHITECTURE.md#3-后端层)。

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
| `connection_string(&str)` | 设置 Redis 连接字符串（Sentinel 模式为逗号分隔的 sentinel URL 列表） |
| `mode(RedisMode)` | 设置 Redis 模式（`Standalone`/`Sentinel`/`Cluster`/`ValkeyStandalone`） |
| `sentinel_master_name(impl Into<String>)` | Sentinel 模式监控的 master 名（默认 `"mymaster"`） |
| `sentinel_addr_map(impl IntoIterator<Item=(String,String)>)` | Sentinel 模式 NAT 地址映射（`ip:port` → 客户端可达 `host:port`），未命中原样使用 |
| `build().await` | 构建 `RedisBackend`（2 秒连接超时） |

**Sentinel 模式行为：** 连接串指向 sentinel 节点列表，master 经
`SENTINEL get-master-addr-by-name` 依次发现（首个成功应答者胜出）；
failover 后数据连接遇可恢复错误（断连/`READONLY`/`MASTERDOWN` 等）自动
重新发现新 master 并重试一次，同一后端实例无感切换。

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

`eval_lua` 与 `script_load` 在执行/缓存前通过 `validate_lua_script` 校验脚本；`eval_sha` 仅校验 SHA1 格式与键名，不重复校验脚本内容——脚本在 `script_load` 时已校验。`eval_sha` 遇 NOSCRIPT（脚本未缓存）不回退，显性返回错误，需调用方重新 `script_load` 或改用 `eval_lua`。

## 🐉 DragonflyBackend

`DragonflyBackend` 包装 `RedisBackend`，提供对 Dragonfly 服务器的缓存支持。
Dragonfly 兼容 Redis 协议，因此复用 Redis 的全部读写路径。

```rust
use oxcache::backend::DragonflyBackend;
use std::sync::Arc;

// 构造 Dragonfly 后端（需要 dragonfly feature）
// 构造复用 RedisBackend 的 TLS 强制：连接串须为 rediss://；
// 开发环境确需 redis:// 时，须设置 OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS
let backend = DragonflyBackend::new("rediss://localhost:6379", 8).await?;

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
| `with_invalidation(Arc<InvalidationBus>)` | 启用写路径失效广播（`invalidation` feature）：构建时将持久层（`is_persistent == true`）link 以 `InvalidatingBackend` 包装，set/delete/clear/set_many/delete_many 成功后经总线广播，其他实例失效各自本地缓存；expire 不广播（装饰器自身可用 `InvalidatingBackend::with_expire_broadcast` 开启 expire 广播，链集成默认关闭）；包装层不支持 sync API |
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

### 批量读取 `iter_entries`

`iter_entries(&self, keys: &[&str]) -> Vec<(String, Option<Vec<u8>>)>`：
按分数从高到低逐层批量读取尚未命中的键，每层一次 `get_many` 调用
（`RedisBackend` 的 `get_many` 即 pipeline 批量读），命中键由最高分
命中层应答；输出顺序与输入一致，不触发回填。某层批量读整批失败时
错误按键拆分映射（每键一条错误事件 + warn），键继续降级到下一层；
全部层处理完仍未命中的键（miss 或各层失败）以 `None` 结束，方法本身
不返回 `Err`。

```rust
// entries: Vec<(String, Option<Vec<u8>>)>，方法本身不返回 Err
let entries = chain.iter_entries(&["k1", "k2", "k3"]).await;
```

### 一站式失效广播装配

`tiered_with_invalidation(l1, l2, bus)`（`memory` + `invalidation` feature）
等价于 `ChainBuilder::new().l1(l1).l2(l2).with_invalidation(bus)`：
持久层（L2 需 `.persistent(true)`）写成功后经 `bus` 广播失效事件，
其他实例的监听任务失效各自本地 L1；expire 不广播。

### ChainCache 同步 API

`ChainCache` 暴露 `get_sync`/`set_sync`/`delete_sync`。这些方法要求**每个**
链接都支持 `SyncCacheBackend`（即通过 `from_sync_backend` 构建）；
否则返回 `Err(OxCacheError::NotSupported)`。

## 📢 Redis Pub/Sub 广播通道（`pubsub` 特性）

通用频道广播设施（`oxcache::pubsub::RedisPubSub`），与失效广播
（`invalidation` 特性的 `InvalidationBus`，只承载缓存失效事件）不同，
它面向任意跨实例消息语义：分布式会话/登录态同步（SSO kickout 广播）、
业务事件扇出等。

```rust,no_run
# use std::sync::Arc;
# async fn example() -> Result<(), oxcache::OxCacheError> {
let ps = oxcache::pubsub::RedisPubSub::new("redis://127.0.0.1:6379").await?;
ps.subscribe("sso:kickout", Arc::new(|msg| {
    println!("kickout broadcast: {msg}");
})).await?;
let receivers = ps.publish("sso:kickout", "user-42").await?;
ps.shutdown();
# Ok(())
# }
```

### 行为契约

- **构造期 fail-fast**：`new` 预热发布连接（`ConnectionManager`），URL 非法
  或端口不可达时立即返回 `Err`，不留"假成功"实例；
- **独占订阅连接**：每个 `subscribe` 现场创建一条专用 Pub/Sub 连接
  （`SUBSCRIBE` 会独占连接，无法复用多路复用连接），首次订阅成功经
  oneshot 回传，`subscribe` 返回 `Ok(())` 即订阅已在服务端生效；
- **publish 返回接收端数量**（无订阅者时为 0）；
- **handler panic 隔离**：回调以 `catch_unwind` 包裹，panic 只记 warn 日志，
  后续消息继续投递、订阅不中断；
- **断线重连**：订阅连接断开后线性退避重连并重新 SUBSCRIBE（默认 5 次，
  基数 500ms、封顶 5s；`with_reconnect_attempts(0)` 关闭重连，退化为
  一次性订阅语义）；
- **任务回收**：后台订阅任务统一登记，`shutdown()` / `Drop` 时全部 abort。

### API 一览

| 方法 | 签名 | 说明 |
|---|---|---|
| `new` | `new(url: &str) -> OxCacheResult<Self>` | 构造并预热发布连接（fail-fast） |
| `with_reconnect_attempts` | `with_reconnect_attempts(self, attempts: usize) -> Self` | 重连次数（默认 5；0 = 不重连） |
| `publish` | `publish(&self, channel: &str, message: &str) -> OxCacheResult<i64>` | 发消息，返回接收端数量 |
| `subscribe` | `subscribe(&self, channel: &str, handler: Arc<dyn Fn(String) + Send + Sync>) -> OxCacheResult<()>` | 订阅频道，handler 在后台任务逐条接收 |
| `shutdown` | `shutdown(&self) -> usize` | abort 全部后台订阅任务，返回停止数 |

## 🔄 同步 API

同步 API 镜像异步 API。运行时要求按配置通路区分（与 README「运行时注意」、架构文档及下文 CacheBuilder 章节的三环境矩阵一致）：

- **默认 Moka 路径（含 `sync_backend_arc` 注入的原生同步面）**：同步方法直连原生同步实现，运行时无关——runtime 之外可直接调用（临时 current-thread runtime 兜底驱动）；`multi_thread` runtime 上经 `tokio::task::block_in_place` 复用当前 runtime；仅 current-thread runtime 的**异步上下文内**显性返回 `Err(NotSupported)`（tokio 禁止嵌套阻塞驱动）
- **`backend_arc(...)` + `sync_mode(true)` 桥接路径（`AsyncToSyncBridge`）**：每个同步调用经 `block_in_place` + `block_on` 阻塞于异步面，要求调用时处于**多线程** Tokio 运行时（I/O 型后端需 ≥2 worker）；runtime 之外或 current-thread runtime 上逐调用返回 `Err(NotSupported)`

### `SyncCacheBackend` Trait 层级

同步 trait 层级（`SyncCacheReader` / `SyncCacheWriter` / `SyncCacheConnector` / `SyncCacheBackend`）的完整定义见[架构文档的后端层章节](ARCHITECTURE.md#3-后端层)。

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

注入自定义同步后端时使用 `sync_backend_arc`（后端以 `Arc<dyn SyncCacheBackend>`
保存：同步 API 直连，异步 API 经 `SyncBackendAdapter` 门面桥出）：

```rust
use oxcache::backend::{MokaMemoryBackend, SyncCacheBackend};

let moka = MokaMemoryBackend::builder().capacity(10_000).build();
let sync_backend: std::sync::Arc<dyn SyncCacheBackend> = std::sync::Arc::new(moka);
let cache: Cache<String, String> = Cache::builder()
    .sync_backend_arc(sync_backend)
    .sync_mode(true)
    .build_sync()?;
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

审计事件：`*_sync` 路径与异步路径同语义发布（`audit` 特性）；发布器若依赖 tokio runtime（如 `InklogAuditPublisher`），在 runtime 之外的同步调用中事件无法落盘、逐条计入 `dropped_count()`（空转无留痕）——runtime 外的同步场景请选运行时无关发布器（`InMemoryAuditPublisher` / `TracingAuditPublisher`）。

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

所有后端（Moka、DashMap、Redis、Valkey、Dragonfly、Aerospike、Disk、Mock、Chain、Bloom）都通过
`set(key, value, Some(ttl))` 支持单条目 TTL。

- **Moka** 使用 `moka::Expiry` trait 实现真正的单条目 TTL，覆盖构建器设置的全局 TTL。
- **DashMap** 使用懒过期（条目在访问时过期）。
- **Redis** 使用 `SET key value PX <ms>`（毫秒精度，`expire` 为 `PEXPIRE`）。
- **Disk（RedbDiskBackend）** 使用懒过期（读路径惰性判定 + 物理删除）；`with_default_ttl` 提供后端默认 TTL，`set(ttl=None)` 沿用之。

各后端的完整行为对照表见 [README](../README.md#️-ttl-行为对照表)。

### TTL 方法

| 方法 | 异步 | 同步 |
|------|------|------|
| 读取剩余 TTL | `cache.ttl(&key).await` | `cache.ttl_sync(&key)` |
| 更新已有键的 TTL | `cache.expire(&key, d).await` | `cache.expire_sync(&key, d)` |
| 设置显式 TTL | `cache.set_with_ttl(&key, &v, Some(d)).await` | `cache.set_with_ttl_sync(&key, &v, Some(d))` |

## ⏳ SWR 三态过期（`stale`）

- `StaleWhileRevalidateBackend`（`oxcache::features::stale`）— 装饰任意 `CacheBackend`：双时间戳 envelope（`expire_at`/`stale_at`），读取三态 Actual/Stale/Expired；非 envelope 旧数据按新鲜透传；物理 TTL = `ttl + stale_ttl`；`get_with_state(key)` 返回 `(payload, StaleState)`
- `StalePolicy` — `Return`（旧值兜底，默认）/ `Revalidate`（同步回源）/ `OffloadRevalidate`（立即回旧值 + 后台刷新）；经 `CacheBuilder::stale_ttl(Duration)` + `stale_policy(StalePolicy)` 启用
- `Cache::get_or_refresh(key, ttl, fallback)` — `get_or` 的后台刷新变体（`OffloadRevalidate` 生效；fallback 需 `Send + 'static`）；`get_or` 在该策略下按 Return 降级
- `Cache::offload_manager()` — Offload 管理器访问器（优雅关闭前 `wait_all` 用）

## 📡 Offload 后台任务（`offload`）

- `OffloadManager::new(max_concurrent)` / `with_policy(max, TimeoutPolicy)` — 同 key 去重 + 信号量并发上限
- `spawn(key, future) -> bool` — `false` 表示同 key 在飞（去重）或许可已满（超限丢弃，不排队）
- `is_in_flight` / `in_flight_count` / `cancel_all` / `wait_all(timeout) -> usize`
- `TimeoutPolicy` — `None` / `Cancel(Duration)`（超时取消）/ `Warn(Duration)`（默认 `Warn(30s)`，仅告警）
- 指标：`oxcache_offload_{spawned,deduplicated,completed,timeout}_total` + gauge `oxcache_offload_active`

## 💾 磁盘持久化后端（`disk`）

- `RedbDiskBackend`（`oxcache::backend::disk`）— redb 4.3 嵌入式磁盘持久化（纯安全 Rust，ACID + WAL）；`create(path)` / `open(path)` / `with_default_ttl` / `with_max_entries`
- 懒过期：读路径惰性判定 + 物理删除（与 DashMap 同口径）；`max_entries` 超限清扫先删过期、按 seq 升序删最旧
- `BackendKind::Disk`；`BackendScore`：score 85（`Scores::REDB`）/ persistent / name `"disk"`；经 `ChainLink::from_arc` 或 `ChainBuilder::extra_backend` 挂链为 L3

## 🔗 链路读策略（`ChainReadStrategy`）

- `Sequential`（默认，逐层命中即返回）/ `Race`（并发全读取最高分命中）/ `ParallelFreshest`（并发全读后按剩余 TTL 择新，None 最低优先、并列取最高分）
- `ChainCacheBuilder::read_strategy(strategy)`；`enable_race_read()` / `disable_race_read()` 为兼容别名

## 🔁 自适应 TTL（`adaptive-ttl`）

- `AdaptiveTtlBackend`（`oxcache::features::adaptive_ttl`）— 装饰任意 `CacheBackend`：按访问模式调整条目 TTL。hot 键（命中计数 ≥ `hot_threshold`）set 时 TTL 乘 `hot_ttl_multiplier`，get 时受 `adjust_interval` 限速把已存条目 `expire` 调整到 `clamp(hot_ttl_multiplier × 剩余 TTL)`（方向不限，限制写放大）；已知键最近访问早于 `cold_idle_after` 时 set 的 TTL 除以 `cold_ttl_divisor`；hot 与 cold 同时满足时 hot 优先
- `AdaptiveTtlConfig` — 全部阈值显式常量（无黑盒启发式）：调整结果钳制在 `[min_ttl, max_ttl]`（默认 1s..1h）；`None`（永不过期）不参与调整原样透传；追踪表上限 `max_tracked_keys`（默认 65 536，0 在 `validate()` 显性拒绝），满时新键按普通键透传；配置经 `validate()` 在构建期校验，`min_ttl > max_ttl`、`hot_ttl_multiplier` 非有限正数（0/负/NaN/inf）、`cold_ttl_divisor = 0`、`max_tracked_keys = 0` 均显性 `Err(InvalidInput)`（而非请求路径 `Duration::clamp` panic 或经乘除静默畸变）
- 启用：`CacheBuilder::adaptive_ttl(AdaptiveTtlConfig)`；与 `sync_mode(true)` / `stale_ttl` 组合在构建期显性拒绝（`Err(NotSupported)`）
- 观测：`stats()` / `reset_stats()` 暴露追踪键数与延长/缩短计数；get 路径主动调整遇后端 `expire` 故障不阻断命中、以 `failed_adjustments` 显性计数；后端 `stats()` 附加 `adaptive_tracked_keys` / `adaptive_hot_extensions` / `adaptive_cold_shortenings` / `adaptive_failed_adjustments`

## 🔥 智能预热（`warmup`）

- `WarmupLoader`（`oxcache::cache::warmup`，crate 根重导出）— 预热数据源端口（对象安全，`Arc<dyn WarmupLoader>` 注入）：`load_hot_keys()` 返回 `Vec<WarmupEntry>`；实现来源由调用方决定（业务预测、`HotKeyTracker` 快照、上游分析导出等）。端口失败显性 `Err` 中止本轮预热，缓存本体不受影响
- `WarmupEntry` — 单条热 key：`value = Some(bytes)` 为直供值回填（源数据/快照路径）；`None` 为仅 key，经 `ChainCache::iter_entries` 批量读从低层晋升（链上无值计 `missing`）
- `Warmup::builder(chain).concurrency(n).build()` — 回填写入并发上界（默认 8，0 视为 1 串行）；`run(loader)` 异步执行，写入经 `ChainCache::set` 落全部后端，Semaphore 滑窗恒定在飞数（任一时刻在飞任务数 ≤ 上界）；`.max_value_bytes(n)` 直供值单值大小上限（默认与序列化写入面 `MAX_JSON_SIZE` 同口径，0 = 不限），`.max_entries(n)` 单轮条目数上限（默认 100_000，0 = 不限，去重后按 loader 顺序保留前 N 条）
- `WarmupReport` — 结果全量显性计数：`loader_entries`（去重前）/ `deduped`（key 首次出现生效）/ `warmed`（直供回填成功）/ `promoted`（低层晋升成功，回填透传链上剩余 TTL，不重置源条目过期语义）/ `missing`（链上无值）/ `failed` + `failures`（逐条写入失败明细，不中断整批）/ `peak_concurrency`（实测并发峰值，≤ 配置上界）/ `dropped_value_too_large`（超单值上限显性丢弃）/ `dropped_over_entry_cap`（超条目数上限显性丢弃）

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

校验规则（512 KB 长度上限、`\r` / `\n` / `\0` 与命令注入字符、控制字符、SQL 注入与路径遍历模式扫描）见[安全文档](SECURITY.md#-键校验)。

### Lua 脚本校验（validate_lua_script）

校验 Lua 脚本的安全性。

```rust
use oxcache::validate_lua_script;

validate_lua_script("return redis.call('GET', KEYS[1])", 1).expect("合法的脚本");
```

校验规则（10 KB 脚本与 100 键上限、危险命令黑名单、沙箱逃逸调用拦截、注释预处理防绕过）见[安全文档](SECURITY.md#-lua-脚本沙箱)。

### SCAN 校验（validate_scan_pattern 与 clamp_scan_count）

校验 SCAN 模式以防止 ReDoS 攻击。

```rust
use oxcache::validate_scan_pattern;

validate_scan_pattern("user:*").expect("合法的模式");
```

校验规则（256 字符与 10 通配符上限）与 `clamp_scan_count` 的 1–1000 钳制见[安全文档](SECURITY.md#-scan-模式限制)。

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

**per-service 维度（R9，默认关闭）**：builder 上显式 `.service_name("orders")`
后，该缓存的操作计数附加 `service` 标签——`export_prometheus_standard()`
出现 `oxcache_service_operations_total{service="orders"}` 行，JSON 导出
（`export_json_format` / `snapshot()`）出现 `service_operations` 段；标签
基数上限 `MetricsConfig::max_service_labels`（默认 64），超限归因计入
`oxcache_service_labels_overflow_total`。未设置时导出与既有格式逐字节兼容；
显式 `.metrics(...)` 注入优先于 `service_name`。

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

## ⚙️ 统一配置中枢（CacheConfig）

`oxcache::config::CacheConfig` 以单一结构承载缓存构建全量参数，支持三条配置通路：
程序化 builder、`OXCACHE_*` 环境变量、confers 配置源（`config-confers` feature）。

```rust
use oxcache::config::CacheConfig;
use std::time::Duration;

// 程序化构建
let config = CacheConfig::builder()
    .capacity(10_000)
    .ttl(Duration::from_secs(60))
    .backend("redis")
    .redis_url("redis://127.0.0.1:6379")
    .build();

// 一致性检查（值域 / 参数组合 / feature 可用性）
config.validate()?;

// 应用到既有构建器
let builder = config.apply_to_cache_builder(oxcache::CacheBuilder::<String, String>::default()).await?;
```

### 环境变量约定（`CacheConfig::try_from_env()`）

| 环境变量 | 类型 | 映射字段 | Feature 前提 |
| --- | --- | --- | --- |
| `OXCACHE_CAPACITY` | u64 | `capacity` | — |
| `OXCACHE_TTL_MS` | u64（毫秒） | `ttl` | — |
| `OXCACHE_TTI_MS` | u64（毫秒） | `tti` | — |
| `OXCACHE_NULL_CACHE_TTL_MS` | u64（毫秒） | `null_cache_ttl` | — |
| `OXCACHE_TTL_JITTER_FACTOR` | f64 | `ttl_jitter_factor` | — |
| `OXCACHE_SYNC_MODE` | bool | `sync_mode` | — |
| `OXCACHE_BACKEND` | 枚举串（moka/dashmap/redis/...） | `backend` | `memory`/`redis`/`disk` 任一 |
| `OXCACHE_METRICS` | bool | `metrics_enabled` | `metrics` |
| `OXCACHE_SERIALIZATION_FORMAT` | 枚举串（json/bincode/postcard） | `serialization_format` | `serialization`（bincode/postcard 各需子 feature） |
| `OXCACHE_REDIS_URL` | 字符串 | `redis_url` | `redis`/`dragonfly` 构建时消费 |
| `OXCACHE_DISK_PATH` | 字符串 | `disk_path` | `disk` 构建时消费 |
| `OXCACHE_CONNECTION_POOL_SIZE` | usize | `connection_pool_size` | `redis`/`dragonfly` 构建时消费（缺省 8） |
| `OXCACHE_CIRCUIT_BREAKER_FAILURE_THRESHOLD` | u64 | `circuit_breaker_failure_threshold` | `redis` 构建时消费 |
| `OXCACHE_CIRCUIT_BREAKER_RESET_TIMEOUT_MS` | u64（毫秒） | `circuit_breaker_reset_timeout` | `redis` 构建时消费 |
| `OXCACHE_SERVICE_NAME` | 字符串 | `service_name` | metrics 构建时消费（R9 service 维度；空串显性拒绝） |

未设置的键回落底层构建器默认（零行为漂移）；解析失败与「键已设置但对应 feature
未启用」均显性报错（附变量名与原始值），布尔值接受 `true/1/yes/on` 与
`false/0/no/off`。`redis_url` 可能携带凭证，`Debug` 输出脱敏为 `"***"`。

### confers 配置源（`config-confers` feature）

`CacheConfig::try_from_confers(&OxcacheConfig)` 将 confers 快照映射为统一配置，
键约定见 `oxcache::features::confers_config` 模块文档。注意：`OxcacheConfig`
的 `capacity`/`default_ttl_ms` 内置默认（10000 / 60000ms），confers 通路映射后
恒为已设置值——即使未配置对应键，也会以该默认覆盖底层构建器默认（与 env 通路
的「未设置 = 零行为漂移」不同）。`OXCACHE_SYNC_MODE` 同样适用于 confers 通路
的 `cache.sync_mode`：与 `cache.backend` 可组合（配置后端的 sync API 经
`AsyncToSyncBridge` 桥出，要求调用时处于多线程 runtime）。

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
