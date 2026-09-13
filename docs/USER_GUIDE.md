<div align="center">

# 📖 Oxcache 用户指南

### 高性能 Rust 双层缓存库完整使用指南

[🏠 首页](../README.md) • [📚 文档中心](../README.md#-文档) • [🎯 示例](../examples/) • [📘 API 参考](API_REFERENCE.md)

---

</div>

> **⚠️ 版本说明**：本文档基于 **Oxcache v0.5.0-rc.4** 编写。

## 📋 目录

<details open>
<summary>📑 目录</summary>

- [🧭 简介](#-简介)
- [🚀 快速入门](#-快速入门)
  - [先决条件](#先决条件)
  - [安装](#安装)
  - [第一步](#第一步)
- [🧱 核心概念](#-核心概念)
- [📖 基础用法](#-基础用法)
  - [使用缓存宏](#使用缓存宏)
  - [手动控制缓存](#手动控制缓存)
  - [序列化](#序列化)
- [⚡ 高级用法](#-高级用法)
  - [链式多层缓存（ChainCache）](#链式多层缓存chaincache)
  - [启用同步 API](#启用同步-api)
  - [布隆过滤器](#布隆过滤器)
  - [TTL 管理](#ttl-管理)
  - [Redis 模式配置](#redis-模式配置)
  - [监控指标](#监控指标)
  - [事件发射（EventPublisher）](#事件发射eventpublisher)
  - [优雅关闭](#优雅关闭)
  - [Valkey 后端](#valkey-后端)
  - [Dragonfly 后端](#dragonfly-后端)
  - [Aerospike 后端](#aerospike-后端)
- [🌟 最佳实践](#-最佳实践)
- [🔧 故障排除](#-故障排除)
- [🎯 后续步骤](#-后续步骤)

</details>

---

## 🧭 简介

**oxcache** 是一个高性能、生产级可用的 Rust 缓存库，提供 L1（进程内内存缓存，使用 Moka）+ L2（分布式 Redis 缓存）的双层架构。它通过
`#[cached]` 宏实现零侵入式缓存，并支持同步 API、布隆过滤器和链式多层后端。

| 学习板块 | 内容 | 收获 |
|---------|------|------|
| 🚀 快速入门 | 环境搭建与首个缓存 | 5 分钟内完成接入 |
| 🧱 核心概念 | 双层架构、链式缓存、同步 API | 理解设计模型 |
| 📖 基础用法 | 宏与手动控制、序列化 | 日常读写上手 |
| ⚡ 高级用法 | ChainCache、布隆、TTL、监控 | 生产级能力运用 |

主要特性包括：

- **🚀 极致性能**：L1 纳秒级响应（P99 < 100ns），L2 毫秒级响应（P99 < 5ms）
- **🔗 链式多层缓存**：`ChainCache` 按后端分数排序读写，支持回填（backfill）
- **⚡ 同步 API**：`sync_mode(true)` 启用 `get_sync`/`set_sync` 等同步方法
- **🛡️ 安全内置**：键/Lua/SCAN 校验、TLS 强制、敏感信息脱敏
- **📊 可观测性**：内置指标、EventPublisher 事件发射

> 💡 **提示**：本指南假设你具备基本的 Rust 知识。如果你是 Rust
> 新手，建议先阅读 [Rust 官方教程](https://doc.rust-lang.org/book/)。

---

## 🚀 快速入门

### 先决条件

在开始之前，请确保你已安装以下工具：

| 类别 | 工具 |
|------|------|
| **必选** | ✅ Rust 1.97.1+（stable）　✅ Cargo（随 Rust 一起安装）　✅ Git |
| **可选** | 🔧 支持 Rust 的 IDE（如 VS Code + rust-analyzer）　🔧 Docker（用于容器化部署）　🔧 Redis 6.0+（用于 L2 缓存测试） |

<details>
<summary><b>🔍 验证安装</b></summary>

```bash
# 检查 Rust 版本
rustc --version
# 预期: rustc 1.97.1 (或更高)

# 检查 Cargo 版本
cargo --version
# 预期: cargo 1.97.1 (或更高)
```

</details>

### 安装

在你的 `Cargo.toml` 中添加 `oxcache`：

```toml
[dependencies]
oxcache = "0.5.0-rc.4"
```

> **注意**：`default = ["minimal"]`，默认仅包含 L1 内存缓存。要使用完整功能，请显式启用 `features = ["full"]`。

> **特性**：要使用 `#[cached]` 宏，需要启用 `macros` 特性：`oxcache = { version = "0.5.0-rc.4", features = ["macros"] }`（`full` 已包含）。

#### 特性分层与依赖

分层预设（`minimal` / `core` / `full`）与完整特性清单见 [README 特性标志](../README.md#-特性标志)；特性前置依赖（如 `lua` 需 `redis`、`red-lock` 需 `lock`、`full` 不含 `bloom` / `kit` 等选择加入特性）见 [API 参考的特性要求](API_REFERENCE.md#-特性要求)。

如果需要最小依赖或自定义特性：

```toml
[dependencies]
oxcache = { version = "0.5.0-rc.4", default-features = false, features = ["core"] }
```

或者使用命令行：

```bash
cargo add oxcache
```

### 第一步

让我们通过一个简单的例子来验证安装。我们将使用 `Cache::builder()` 创建缓存：

#### 方法一：L1 内存缓存（推荐入门）

```rust
use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
    id: u64,
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 Cache::builder() 创建 L1 内存缓存（默认 Moka 后端）
    let cache: Cache<String, User> = Cache::builder()
        .capacity(10000)
        .ttl(Duration::from_secs(3600))
        .build()
        .await?;

    // 第一次调用：写入缓存
    let user = User { id: 1, name: "User 1".to_string() };
    cache.set(&"user:1".to_string(), &user).await?;
    println!("First call: {:?}", user);

    // 第二次调用：从缓存读取
    let cached_user: Option<User> = cache.get(&"user:1".to_string()).await?;
    println!("Cached call: {:?}", cached_user);

    Ok(())
}
```

#### 方法二：使用 #[cached] 宏

```rust
use oxcache::cached;  // 宏在 crate 根重导出
use oxcache::Cache;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
    id: u64,
    name: String,
}

// 使用 #[cached] 宏一行代码启用缓存
// 参数：service（注册名）、key（键模板）、ttl（秒）
#[cached(service = "user_cache", key = "user:{id}", ttl = 600)]
async fn get_user(id: u64) -> Result<User, String> {
    // 模拟耗时的数据库查询
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok(User {
        id,
        name: format!("User {}", id),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化缓存（默认 Moka 后端）
    let cache: Cache<String, User> = Cache::builder().build().await?;

    // 注册缓存实例到全局管理器（供宏通过 service 名查找）
    cache.register_for_macro("user_cache").await?;

    // 第一次调用：执行函数逻辑 + 缓存结果（~100ms）
    let user = get_user(1).await?;
    println!("First call: {:?}", user);

    // 第二次调用：直接从缓存返回（~0.1ms）
    let cached_user = get_user(1).await?;
    println!("Cached call: {:?}", cached_user);

    Ok(())
}
```

#### 方法三：L2 Redis 缓存

```rust
use oxcache::Cache;
use oxcache::backend::RedisBackend;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建 Redis 后端（生产环境请使用 rediss:// TLS 连接）
    // 开发环境需设置：OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS
    let redis = RedisBackend::new("rediss://127.0.0.1:6379").await?;

    // 通过 backend_arc 注入后端
    let cache: Cache<String, String> = Cache::builder()
        .backend_arc(Arc::new(redis))
        .build()
        .await?;

    cache.set(&"key".to_string(), &"value".to_string()).await?;

    let val: Option<String> = cache.get(&"key".to_string()).await?;
    println!("Value: {:?}", val);

    Ok(())
}
```

> **说明**：`Cache::builder()` 没有 `.redis(...)` 或 `.tiered(...)` 方法。要使用 Redis，请用 `RedisBackend::new(url).await?` 构造后端，再通过 `.backend_arc(Arc::new(redis))` 注入。要实现 L1+L2 分层，请使用 [ChainCache](#链式多层缓存chaincache)。

---

## 🧱 核心概念

理解这些核心概念将帮助你更有效地使用 `oxcache`。

### 双层缓存架构

`oxcache` 的核心是 L1 (Moka) + L2 (Redis) 两级缓存架构。L1 是本地内存缓存，访问速度极快；L2 是分布式缓存，支持多实例共享。

- **L1 (Moka)**：进程内高速缓存，使用 LRU/TinyLFU 淘汰策略，支持 per-entry TTL（通过 `moka::Expiry`）
- **L2 (Redis)**：分布式共享缓存，支持 Standalone/Sentinel/Cluster 模式

### 链式多层与回填

`ChainCache` 把多个后端按分数（score）从高到低排序：Moka=100、DashMap=90、Redis=50。
读取时从最高分后端开始查找；写入时写入所有后端。启用 `enable_backfill()` 后，低分后端命中会回填到更高分的后端，从而减少后续对低分后端的访问。

### 同步 API

通过 `Cache::builder().sync_mode(true)` 启用异步 API 的同步镜像。运行时要求与限制（multi_thread 运行时、不能与 `backend_arc(...)` 组合）见[启用同步 API](#启用同步-api) 与 [API 参考](API_REFERENCE.md#-同步-api)。

### 容错与单飞

- **Single-Flight**：`get_or` / `get_or_sync` 对同一 key 的并发请求去重，只执行一次计算
- **容错降级**：L2 不可用时，链式缓存的读取会跳过失败的后端，继续从其他后端取值

### 通用 per-entry TTL

所有后端（Moka / DashMap / Redis / Mock / Chain / Bloom）都支持 `set(key, value, Some(ttl))` 设置单条目 TTL；设置、读取与修改方法见 [TTL 管理](#ttl-管理)。

---

## 📖 基础用法

### 使用缓存宏

使用 `#[cached]` 宏为函数添加缓存功能（需要 `macros` 特性）：

```rust
use oxcache::cached;  // crate 根重导出
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    id: u64,
    name: String,
}

// 自动缓存结果，缓存键为 "user:{id}"
// 参数：service（注册名）、key（键模板）、ttl（秒）
#[cached(service = "user_cache", key = "user:{id}", ttl = 300)]
async fn get_user(id: u64) -> Result<User, String> {
    // 这里写你的业务逻辑，比如数据库查询
    database::query_user(id).await
}
```

宏通过 `service` 名从内部注册表查找 `Cache` 实例。若未注册，原函数照常执行（不缓存）。

宏参数（`service` / `ttl` / `key` / `key_prefix` / `sync` / `skip_cache_write` / `single_flight` / `strict` / `condition` / `cache_none`）的完整说明见 [API 参考](API_REFERENCE.md#-缓存宏)。

### 手动控制缓存

你也可以绕过宏，直接使用 `Cache` 实例进行缓存操作：

```rust
use oxcache::Cache;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建缓存实例（默认 Moka 后端）
    let cache: Cache<String, String> = Cache::builder()
        .capacity(10000)
        .ttl(Duration::from_secs(3600))
        .build()
        .await?;

    // 标准操作
    cache.set(&"key".to_string(), &"value".to_string()).await?;

    let val: Option<String> = cache.get(&"key".to_string()).await?;
    assert_eq!(val, Some("value".to_string()));

    // 带 per-entry TTL 的写入
    cache.set_with_ttl(&"key2".to_string(), &"value2".to_string(), Some(Duration::from_secs(60))).await?;

    // 删除缓存
    cache.delete(&"key".to_string()).await?;

    // 检查键是否存在
    let exists = cache.exists(&"key".to_string()).await?;
    println!("Key exists: {}", exists);

    // 清空缓存
    cache.clear().await?;

    Ok(())
}
```

#### 直接操作后端

如果需要更精细的控制，可以直接构造并操作底层后端：

```rust
use oxcache::backend::{MokaMemoryBackend, RedisBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // L1：Moka 内存后端
    let l1 = MokaMemoryBackend::builder()
        .capacity(10000)
        .ttl(Duration::from_secs(60))
        .build();

    // L2：Redis 后端
    let l2 = RedisBackend::new("rediss://127.0.0.1:6379").await?;

    // 直接写入 L1（临时数据）
    l1.set("temp_key", b"temp_data".to_vec(), Some(Duration::from_secs(60))).await?;

    // 直接写入 L2（共享数据）
    l2.set("shared_key", b"shared_data".to_vec(), Some(Duration::from_secs(3600))).await?;

    Ok(())
}
```

### 序列化

类型化 `Cache<K, V>` 的 `get`/`set`/`set_with_ttl` 均经序列化层：值类型 `V` 需实现
`serde::Serialize + serde::Deserialize<'de>`，键类型 `K` 需实现 `oxcache::CacheKey`。

| 格式 | 特性 | 说明 |
|------|------|------|
| **JSON** | 默认（`serialization`） | 可读性与跨语言互操作；`serde_stacker` 深度防护防嵌套 DoS |
| **bincode 1.x** | `serde-bincode` | 二进制紧凑格式，经 `CacheBuilder::serialization_format()` 切换 |
| **postcard** | `postcard` | varint 编码，混合负载下体积约为 JSON 的 46% |

```rust
use oxcache::infra::serialization::SerializationFormat;

// 需启用 serde-bincode / postcard 特性（serialization_format 方法需 serialization 或 full）
let cache: Cache<String, User> = Cache::builder()
    .serialization_format(SerializationFormat::Postcard)
    .build()
    .await?;
```

> **注意**：二进制格式无自描述头，**同一键前缀不得混用格式**。各格式的 L2 传输体积实测见 [性能基线](PERFORMANCE.md)。

---

## ⚡ 高级用法

### 链式多层缓存（ChainCache）

`ChainCache` 把多个后端组合成链，按分数排序读写：

```rust
use oxcache::cache::{ChainCache, ChainLink};
use oxcache::backend::{MokaMemoryBackend, RedisBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let l1 = MokaMemoryBackend::builder().capacity(10000).ttl(Duration::from_secs(300)).build();
    let l2 = RedisBackend::new("rediss://127.0.0.1:6379").await?;

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(l1))   // L1，score 100
        .link(ChainLink::from_backend(l2))   // L2，score 50
        .enable_backfill()                   // L2 命中时回填 L1
        .default_time_to_live(Duration::from_secs(600))
        .build();  // 同步构建

    // 写入所有后端（透传 TTL）
    chain.set("key", b"value".to_vec(), Some(Duration::from_secs(60))).await?;

    // 读取：从 L1 开始，未命中查 L2，命中则（启用回填时）回填 L1
    let v = chain.get("key").await?;
    println!("Value: {:?}", v);

    // 删除所有后端
    chain.delete("key").await?;

    Ok(())
}
```

**ChainCacheBuilder 方法**：`link(ChainLink)`、`links(Vec<ChainLink>)`、`backend(B)`、
`default_time_to_live(Duration)`、`enable_backfill()` / `disable_backfill()`、`build()`（同步）。

### 启用同步 API

在 multi_thread runtime 下，启用 `sync_mode(true)` 即可使用同步方法：

```rust
use oxcache::Cache;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await?;

    // 同步方法（无需 .await）
    cache.set_sync(&"k".to_string(), &"v".to_string())?;
    let v = cache.get_sync(&"k".to_string())?;
    assert_eq!(v, Some("v".to_string()));

    cache.delete_sync(&"k".to_string())?;
    Ok(())
}
```

完整同步方法表与未启用 `sync_mode` 时的 `NotSupported` 行为见 [API 参考](API_REFERENCE.md#-同步-api)。

### 布隆过滤器

启用 `bloom` 特性（不在 `full` 内）可进行负查询过滤。`BloomFilterBackend`
装饰任意 `CacheBackend`，当布隆过滤器判定 key 不存在时直接跳过内部后端：

```rust
use oxcache::backend::MokaMemoryBackend;
use oxcache::features::bloom_filter::{BloomFilterBackend, BloomFilter};

let bf = BloomFilter::new(100_000, 0.01); // 容量 10 万，误判率 1%
let inner = MokaMemoryBackend::new();
let backend = BloomFilterBackend::new(inner);  // 可作为 CacheBackend 使用
```

`BloomFilter` 与 `BloomFilterBackend` 的完整方法列表、builder 用法与语义见 [API 参考](API_REFERENCE.md#-布隆过滤器)。

### TTL 管理

```rust
use std::time::Duration;

// 设置 per-entry TTL
cache.set_with_ttl(&"k".to_string(), &v, Some(Duration::from_secs(60))).await?;

// 读取剩余 TTL（None 表示无 per-entry TTL 或 key 不存在）
let ttl = cache.ttl(&"k".to_string()).await?;

// 修改已存在 key 的 TTL（不改动值），返回 true 表示成功
let ok = cache.expire(&"k".to_string(), Duration::from_secs(120)).await?;

// 更新值但保留原 TTL
let original = cache.ttl(&"k".to_string()).await?;
cache.set_with_ttl(&"k".to_string(), &new_value, original).await?;
```

各后端 TTL 行为对照表见 [README](../README.md#️-ttl-行为对照表)。

### Redis 模式配置

oxcache 支持多种 Redis 部署模式。所有模式都建议使用 TLS（`rediss://`）：

#### Standalone 模式

```rust
use oxcache::backend::RedisBackend;

let backend = RedisBackend::new("rediss://127.0.0.1:6379").await?;
```

#### 通过 builder 指定模式

```rust
use oxcache::backend::{RedisBackend, RedisMode};

let backend = RedisBackend::builder()
    .connection_string("rediss://127.0.0.1:6379")
    .mode(RedisMode::Standalone)  // 或 Sentinel / Cluster / ValkeyStandalone
    .build()
    .await?;
```

> **安全**：非 TLS 连接会被拒绝，除非设置环境变量
> `OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS`（仅限开发环境）。

### 监控指标

启用 `metrics` 特性后，可使用 crate 根重导出的 `get_enhanced_stats` / `export_prometheus_format` / `export_json_format` 读取 `CacheStats`（命中率等）并导出 Prometheus / JSON 文本；更底层的 `MetricsCollector`（位于 `oxcache::infra::metrics::backend`）提供 L1/L2 命中/未命中计数和每操作延迟直方图。用法示例见 [API 参考的可观测性章节](API_REFERENCE.md#-可观测性)。

### 事件发射（EventPublisher）

oxcache 通过 `EventPublisher` trait（crate 根重导出，`core` 模块为私有）提供结构化事件发射机制：`ChainCache` 后端操作失败时通过配置的 `EventPublisher` 抛出事件，用户可自行决定处理方式（日志、metrics、告警或忽略）。实现与配置示例见 [API 参考的可观测性章节](API_REFERENCE.md#-可观测性)。

启用 `metrics` 特性会引入内置 metrics 实现（`serialization` + `chrono` + `dashmap`），
不依赖外部 OpenTelemetry crate。如需 OTLP 导出，由应用层统一处理。

### 优雅关闭

```rust
use oxcache::Cache;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, String> = Cache::builder()
        .capacity(10000)
        .ttl(Duration::from_secs(3600))
        .build()
        .await?;

    // 你的应用逻辑
    cache.set(&"key".to_string(), &"value".to_string()).await?;

    // 优雅关闭（释放后端资源）
    cache.shutdown().await;

    Ok(())
}
```

关闭机制会调用后端的 `shutdown()`，清理连接等资源。

### Valkey 后端

[Valkey](https://valkey.io/) 是 Redis 的开源分支，完全兼容 Redis 协议。
通过 `detect_valkey()` 可自动检测连接目标是 Redis 还是 Valkey：

```rust
use oxcache::backend::{RedisBackend, RedisMode};

// Valkey 使用与 Redis 相同的 API，通过 RedisMode::ValkeyStandalone 标识
let backend = RedisBackend::builder()
    .connection_string("redis://127.0.0.1:6379")
    .mode(RedisMode::ValkeyStandalone)
    .build()
    .await?;
```

### Dragonfly 后端

[Dragonfly](https://www.dragonflydb.io/) 是高性能 Redis/Memcached 替代品。
启用 `dragonfly` feature 后使用：

```toml
# Cargo.toml
[dependencies]
oxcache = { version = "0.5.0-rc.4", features = ["dragonfly"] }
```

```rust
use oxcache::backend::DragonflyBackend;

// 构造 Dragonfly 后端
let dragonfly = DragonflyBackend::new("redis://127.0.0.1:6379", 8).await?;
```

推荐与 Moka 组合为 ChainCache（组合示例与限制说明见 [API 参考](API_REFERENCE.md#-dragonflybackend)）。

### Aerospike 后端

[Aerospike](https://aerospike.com/) 是高性能分布式 KV 存储。
通过 `aerospike` feature 启用：

```toml
# Cargo.toml
[dependencies]
oxcache = { version = "0.5.0-rc.4", features = ["aerospike"] }
```

后端经 `AerospikeBackend::new(AerospikeConfig)` 构造，配置字段（`seed_nodes` / `namespace` / `set_name` / `default_ttl` / `ip_map`）见 [API 参考](API_REFERENCE.md#-aerospikebackend)。

> **Docker 环境注意**：Aerospike 容器内部 IP 与主机不同，需通过 `ip_map` 配置
> IP 地址转换表，或使用 `access-address` 配置服务器广播地址。

---

## 🌟 最佳实践

### ✅ 推荐做法

- **合理设置 TTL**：根据数据更新频率设置缓存过期时间，避免数据不一致。
- **使用 ChainCache 做分层**：L1 用 Moka 缓存热数据，L2 用 Redis 共享数据，并启用回填。
- **监控缓存命中率**：定期检查命中率，及时调整容量和 TTL。
- **使用单飞防击穿**：热点 key 用 `get_or` / `get_or_sync` 避免重复回源。
- **分离冷热数据**：L1 缓存热数据，L2 缓存共享数据。

### ❌ 避免做法

- **缓存过大数据**：避免缓存 large object，优先缓存元数据和 ID。
- **忽略过期策略**：合理设置 TTL，避免缓存脏数据。
- **单点故障**：生产环境务必使用 Sentinel 或 Cluster 模式。
- **忽视监控**：启用指标收集和监控，及时发现问题。

### 🔒 安全最佳实践

oxcache 内置多层安全防护机制，建议在生产环境中遵循以下安全最佳实践：

#### 1. 敏感信息保护

- **使用环境变量存储密码**：永远不要在配置文件中硬编码密码

  ```toml
  # ✅ 正确做法
  connection_string = "rediss://:${REDIS_PASSWORD}@localhost:6379/0"

  # ❌ 错误做法
  connection_string = "rediss://:mypassword123@localhost:6379/0"
  ```

- **启用日志脱敏**：oxcache 会自动脱敏日志中的敏感信息，确保生产环境日志不包含密码

#### 2. 连接安全

- **使用 TLS 加密**：生产环境必须使用 `rediss://`，否则 oxcache 会拒绝连接

  ```rust
  // ✅ 生产环境
  let backend = RedisBackend::new("rediss://:${REDIS_PASSWORD}@localhost:6380").await?;

  // ❌ 开发环境需显式确认风险
  // OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS
  let backend = RedisBackend::new("redis://127.0.0.1:6379").await?;
  ```

- **使用强密码**：Redis 密码至少 32 位，包含大小写字母、数字和特殊字符

#### 3. 访问控制

- **配置网络隔离**：使用防火墙限制 Redis 访问来源
- **使用 ACL**：生产环境建议使用 Redis ACL 限制用户权限
- **定期轮换密码**：定期更换 Redis 密码，建议每 90 天更换一次

#### 4. 审计与监控

- **启用安全日志**：监控认证失败和异常访问
- **配置告警**：对连接失败和认证失败设置告警
- **定期审计**：定期检查日志和安全配置

#### 5. 输入校验

- **键名校验**：oxcache 自动校验 Redis 键（拒绝空键、超长键、含 `\r` / `\n` / `\0` 与命令注入字符的键，扫描 SQL 注入与路径遍历模式）
- **Lua 脚本校验**：执行前自动拦截 `FLUSHALL`/`FLUSHDB`/`KEYS`/`SHUTDOWN` 等危险命令
- **SCAN 模式校验**：自动限制通配符数量与长度，防止 ReDoS

完整机制见 [🔒 安全文档](SECURITY.md)。

---

## 🔧 故障排除

<details>
<summary><b>❓ 问题：缓存未命中率高</b></summary>

**解决方案**：

1. 检查 TTL 设置是否过短
2. 确认数据是否被频繁更新
3. 启用 `ChainCache` 的 `enable_backfill()`，让 L2 命中回填 L1
4. 调整 L1 缓存容量大小

</details>

<details>
<summary><b>❓ 问题：Redis 连接失败</b></summary>

**解决方案**：

1. 检查连接字符串是否以 `rediss://` 开头（TLS）
2. 若开发环境必须用非 TLS，设置 `OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS`
3. 确认 Redis 服务是否正常运行
4. 检查网络连接和防火墙设置
5. 验证用户名密码是否正确

</details>

<details>
<summary><b>❓ 问题：同步 API 返回 NotSupported</b></summary>

**解决方案**：

1. 确认 `Cache::builder().sync_mode(true)` 已启用
2. 确认未同时使用 `backend_arc(...)`（`sync_mode` 仅支持默认 Moka 后端）
3. 确认运行在 `multi_thread` Tokio runtime（`#[tokio::main(flavor = "multi_thread")]`）
4. current-thread runtime 下同步 API 会返回 `Err(NotSupported)`

</details>

<details>
<summary><b>❓ 问题：性能下降</b></summary>

**解决方案**：

1. 检查是否存在内存泄漏
2. 调整 L1 缓存容量是否合理
3. 对大量写入使用 Redis pipeline（`set_many_pipeline` 等）
4. 分析慢查询日志

</details>

<div align="center">

**💬 仍然需要帮助？** [提交 Issue](https://github.com/Kirky-X/oxcache/issues) 或 [访问文档中心](https://docs.rs/oxcache)

</div>

---

## 🎯 后续步骤

| 去向 | 内容 |
|------|------|
| [📘 API 参考](API_REFERENCE.md) | 详细的接口说明与错误码表 |
| [🏗️ 架构设计](ARCHITECTURE.md) | 深入了解内部机制与数据流 |
| [🧪 测试场景矩阵](TEST_SCENARIOS.md) | 功能域到测试落点的映射与执行口径 |
| [💻 示例代码](../examples/) | 真实场景的代码样例 |
| [📊 性能基线](PERFORMANCE.md) | 序列化体积与热路径基准 |
| [🔒 安全文档](SECURITY.md) | 威胁模型与安全配置 |

<div align="center">

**[📖 API 文档](https://docs.rs/oxcache)** • **[❓ 常见问题](https://github.com/Kirky-X/oxcache/wiki)** • **[🐛 报告问题](https://github.com/Kirky-X/oxcache/issues)**

由 oxcache Team 用 ❤️ 制作

[⬆ 回到顶部](#-oxcache-用户指南)

</div>
