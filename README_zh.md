<div align="center">

<img src="docs/image/oxcache.png" alt="Oxcache Logo" width="250">

[![CI](https://github.com/Kirky-X/oxcache/actions/workflows/ci.yml/badge.svg)](https://github.com/Kirky-X/oxcache/actions/workflows/ci.yml)[![Crates.io](https://img.shields.io/crates/v/oxcache.svg)](https://crates.io/crates/oxcache)[![Documentation](https://docs.rs/oxcache/badge.svg)](https://docs.rs/oxcache)[![Downloads](https://img.shields.io/crates/d/oxcache.svg)](https://crates.io/crates/oxcache)[![codecov](https://codecov.io/gh/Kirky-X/oxcache/branch/main/graph/badge.svg)](https://codecov.io/gh/Kirky-X/oxcache)[![Dependency Status](https://deps.rs/repo/github/Kirky-X/oxcache/status.svg)](https://deps.rs/repo/github/Kirky-X/oxcache)[![License](https://img.shields.io/crates/l/oxcache.svg)](https://github.com/Kirky-X/oxcache/blob/main/LICENSE)[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)

[English](../README.md) | 简体中文

高性能、生产级的 Rust 双层缓存库，提供 L1（Moka 内存缓存）+ L2（Redis 分布式缓存）双层架构。

</div>

## ✨ 核心特性

<div align="center">

<table>
<tr>
<td width="20%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/rocket.png" width="48"><br>
<b>极致性能</b><br>L1 纳秒级响应
</td>
<td width="20%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/magic-wand.png" width="48"><br>
<b>零侵入式</b><br>一行代码启用缓存
</td>
<td width="20%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/cloud.png" width="48"><br>
<b>自动故障恢复</b><br>Redis 故障自动降级
</td>
<td width="20%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/synchronize.png" width="48"><br>
<b>多实例同步</b><br>基于 Pub/Sub 机制
</td>
<td width="20%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/lightning.png" width="48"><br>
<b>批量优化</b><br>智能批量写入
</td>
</tr>
</table>

</div>

- **🚀 极致性能**: L1 纳秒级响应（P99 < 100ns），L2 毫秒级响应（P99 < 5ms）
- **🎯 零侵入式**: 通过 `#[cached]` 宏一行代码启用缓存
- **🔄 自动故障恢复**: Redis 故障时自动降级，恢复后自动重放 WAL
- **🌐 多实例同步**: 基于 Pub/Sub + 版本号的失效同步机制
- **⚡ 批量优化**: 智能批量写入，大幅提升吞吐量
- **🛡️ 生产级可靠**: 完整的可观测性、健康检查、混沌测试验证

## 📦 快速开始

### 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
oxcache = "0.2.0"
```

> **注意**：`tokio` 和 `serde` 已默认包含。如果需要最小依赖，可以使用
`oxcache = { version = "0.2.0", default-features = false }` 手动添加。

> **特性**：要使用 `#[cached]` 宏，需要启用 `macros` 特性：`oxcache = { version = "0.2.0", features = ["macros"] }`

#### 特性分层

```toml
# 完整特性（推荐）
oxcache = { version = "0.2.0", features = ["full"] }

# 核心功能（L1 + L2 缓存）
oxcache = { version = "0.2.0", features = ["core"] }

# 最小特性（仅 L1 缓存）
oxcache = { version = "0.2.0", features = ["minimal"] }

# 自定义选择
oxcache = { version = "0.2.0", features = ["core", "macros", "metrics"] }
```

#### 可用特性

| 层级 | 包含特性 | 描述 |
|------|----------|------|
| **minimal** | `moka`, `serialization`, `metrics` | 仅 L1 缓存 |
| **core** | `minimal` + `redis` | L1 + L2 缓存 |
| **full** | `core` + 所有高级特性 | 完整功能 |

**高级特性**（包含在 `full` 中）：
- `macros` - `#[cached]` 属性宏
- `batch-write` - 优化的批量写入
- `wal-recovery` - 预写日志持久化
- `bloom-filter` - 缓存穿透保护
- `rate-limiting` - DoS 防护
- `database` - 数据库集成（SeaORM/SQLx）
- `cli` - 命令行界面
- `full-metrics` - OpenTelemetry 集成
- `smart-strategy` - 基于熵压缩的智能缓存策略
- `redis-native` - 原生 Redis 操作支持
- `compression` - 数据压缩（flate2）
- `lua-script` - Lua 脚本执行支持
- `extra-serialization` - MessagePack、CBOR 序列化
- `config-dynamic` - 动态配置

### 最简示例

```rust
use oxcache::macros::cached;
use oxcache::{Cache, CacheBuilder};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
}

// 一行代码启用缓存
#[cached(service = "user_cache", ttl = 600)]
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
    // 使用 Builder 模式初始化缓存
    let cache: Cache<String, User> = Cache::builder()
        .redis("redis://127.0.0.1:6379")
        .build()
        .await?;

    // 注册缓存实例供宏使用
    cache.register_for_macro("user_cache").await;

    // 第一次调用：执行函数逻辑 + 缓存结果（~100ms）
    let user = get_user(1).await?;
    println!("First call: {:?}", user);

    // 第二次调用：直接从缓存返回（~0.1ms）
    let cached_user = get_user(1).await?;
    println!("Cached call: {:?}", cached_user);

    Ok(())
}
```

### 配置文件

创建 `config.toml`：

> **重要**：要从配置文件初始化，需要启用 `confers` 特性：
> ```toml
> oxcache = { version = "0.2.0", features = ["confers"] }
> ```

```toml
[global]
default_ttl = 3600
health_check_interval = 30
serialization = "json"
enable_metrics = true

# 双层缓存 (L1 + L2)
[services.user_cache]
cache_type = "two-level"  # "l1" | "l2" | "two-level"
ttl = 600

  [services.user_cache.l1]
  max_capacity = 10000
  ttl = 300  # L1 TTL 必须 <= L2 TTL
  tti = 180
  initial_capacity = 1000

  [services.user_cache.l2]
  mode = "standalone"  # "standalone" | "sentinel" | "cluster"
  connection_string = "redis://127.0.0.1:6379"

  [services.user_cache.two_level]
  write_through = true
  promote_on_hit = true
  enable_batch_write = true
  batch_size = 100
  batch_interval_ms = 50

# 仅 L1 缓存 (仅内存)
[services.session_cache]
cache_type = "l1"
ttl = 300

  [services.session_cache.l1]
  max_capacity = 5000
  ttl = 300
  tti = 120

# 仅 L2 缓存 (仅 Redis)
[services.shared_cache]
cache_type = "l2"
ttl = 7200

  [services.shared_cache.l2]
  mode = "standalone"
  connection_string = "redis://127.0.0.1:6379"
```

### 类型安全配置 API（推荐）

Oxcache 提供**类型安全的构建器 API** 用于配置，支持编译时类型检查和更好的 IDE 支持。对于大多数用例，推荐使用此方式而非 TOML 配置。

> **注意**：要使用类型安全配置 API，需要启用 `confers` 特性：
> ```toml
> oxcache = { version = "0.2.0", features = ["confers"] }
> ```

#### 仅内存缓存 (L1)

```rust
use oxcache::config::UnifiedConfigBuilder;
use oxcache::{Cache, CacheBuilder};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 使用构建器 API 创建类型安全配置
    let config = UnifiedConfigBuilder::memory_only()
        .with_ttl(3600)           // 默认 TTL（秒）
        .with_l1_capacity(10000)  // L1 缓存容量
        .build();

    // 从配置直接创建缓存
    let cache: Cache<String, User> = CacheBuilder::from_unified_config(&config)
        .build()
        .await?;

    // 使用缓存
    let user = User {
        id: 1,
        name: "Alice".to_string(),
    };

    cache.set(&"user:1".to_string(), &user).await?;
    let cached: Option<User> = cache.get(&"user:1".to_string()).await?;

    println!("User: {:?}", cached);
    Ok(())
}
```

#### 分层缓存 (L1 + L2)

```rust
use oxcache::config::UnifiedConfigBuilder;
use oxcache::{Cache, CacheBuilder};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建分层缓存配置
    let config = UnifiedConfigBuilder::tiered()
        .with_ttl(7200)            // 默认 TTL（秒）
        .with_l1_capacity(10000)   // L1 内存缓存容量
        .with_redis_url("redis://localhost:6379")  // L2 Redis 连接
        .with_redis_mode("standalone")  // Redis 模式
        .build();

    // 从配置直接创建缓存
    let cache: Cache<String, User> = CacheBuilder::from_unified_config(&config)
        .build()
        .await?;

    // 使用缓存（同时写入 L1 和 L2）
    let user = User {
        id: 1,
        name: "Alice".to_string(),
    };

    cache.set(&"user:1".to_string(), &user).await?;
    let cached: Option<User> = cache.get(&"user:1".to_string()).await?;

    println!("User: {:?}", cached);
    Ok(())
}
```

#### 配置构建器方法

| 方法 | 描述 |
|--------|------|
| `Cache::builder()` | 创建新的缓存构建器 |
| `.ttl(Duration)` | 设置缓存条目的默认 TTL |
| `.capacity(u64)` | 设置内存缓存容量 |
| `.redis(url)` | 配置 Redis 后端 |
| `.redis_with_mode(url, mode)` | 配置 Redis（支持 Standalone/Sentinel/Cluster 模式）|
| `.tiered(l1_capacity, url)` | 配置分层缓存（L1 + L2）|
| `.with_backend(backend)` | 使用自定义后端 |
| `.batch_writes(bool)` | 启用/禁用批量写入 |
| `.auto_promote(bool)` | 启用/禁用从 L2 到 L1 的自动提升 |
| `.build()` | 构建 `Cache<K, V>` 实例 |

#### 类型安全 API 的优势

- **编译时验证**：配置错误在编译时被捕获
- **IDE 支持**：完整的自动补全和类型提示
- **无运行时解析**：消除 TOML 解析开销
- **更好的错误信息**：类型错误而非配置解析错误
- **重构友好**：重命名重构可在配置中生效

## 🎨 使用场景

### 场景 1: 用户信息缓存

```rust
#[cached(service = "user_cache", ttl = 600)]
async fn get_user_profile(user_id: u64) -> Result<UserProfile, Error> {
    database::query_user(user_id).await
}
```

### 场景 2: API 响应缓存

```rust
#[cached(
    service = "api_cache",
    ttl = 300,
    key = "api_{endpoint}_{version}"
)]
async fn fetch_api_data(endpoint: String, version: u32) -> Result<ApiResponse, Error> {
    http_client::get(&format!("/api/{}/{}", endpoint, version)).await
}
```

### 场景 3: 仅 L1 热数据缓存

```rust
#[cached(service = "session_cache", cache_type = "l1", ttl = 60)]
async fn get_user_session(session_id: String) -> Result<Session, Error> {
    session_store::load(session_id).await
}
```

### 场景 4: 手动控制缓存

```rust
use oxcache::{Cache, CacheBuilder};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct MyData {
    field: String,
}

async fn advanced_caching() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 Builder 模式初始化缓存
    let cache: Cache<String, MyData> = Cache::builder()
        .redis("redis://127.0.0.1:6379")
        .build()
        .await?;

    let my_data = MyData {
        field: "value".to_string(),
    };

    // 标准操作
    cache.set(&"key".to_string(), &my_data).await?;

    let data: Option<MyData> = cache.get(&"key".to_string()).await?;
    println!("Data: {:?}", data);

    // 删除
    cache.delete(&"key".to_string()).await?;

    Ok(())
}
```

## 🏗️ 架构设计

```mermaid
graph TD
    A[Application Code<br/>#[cached] Macro] --> B[Cache&lt;K, V&gt;<br/>统一缓存接口]

    B --> C[ChainCache<br/>分层后端]
    B --> D[MokaMemoryBackend<br/>仅 L1]
    B --> E[RedisBackend<br/>仅 L2]

    C --> F[L1 Cache<br/>Moka]
    C --> G[L2 Cache<br/>Redis]

    D --> F
    E --> G

    style A fill:#e1f5fe
    style B fill:#f3e5f5
    style C fill:#e8f5e8
    style D fill:#fff3e0
    style E fill:#fce4ec
    style F fill:#f1f8e9
    style G fill:#fdf2e9
```

**L1**: 进程内高速缓存，使用 LRU/TinyLFU 淘汰策略
**L2**: 分布式共享缓存，支持 Sentinel/Cluster 模式

## 📊 性能基准

> 测试环境: M1 Pro, 16GB RAM, macOS, Redis 7.0
>
> **注意**: 性能因硬件、网络条件和数据大小而异。

```mermaid
xychart-beta
    title "单线程延迟测试 (P99)"
    x-axis ["L1 缓存", "L2 缓存", "数据库"]
    y-axis "延迟时间" 0 --> 60
    bar [50, 3, 30]
    line [50, 3, 30]
```

```mermaid
xychart-beta
    title "吞吐量测试 (batch_size=100)"
    x-axis ["L1 操作", "L2 单次写入", "L2 批量写入"]
    y-axis "操作数/秒" 0 --> 600
    bar [7500, 75, 350]
```

**性能数据总结**:
- **L1 缓存**: 50-100ns (内存访问)
- **L2 缓存**: 1-5ms (Redis, 本地)
- **数据库**: 10-50ms (典型 SQL 查询)
- **L1 操作**: 5-10M ops/sec
- **L2 单次写入**: 50-100K ops/sec
- **L2 批量写入**: 200-500K ops/sec

## 🛡️ 可靠性

- ✅ 单次请求去重 (Single-Flight)
- ✅ 预写日志 (WAL) 持久化
- ✅ Redis 故障自动降级
- ✅ 优雅关闭机制
- ✅ 健康检查与自动恢复

## 🔐 安全性

Oxcache 实现了多项安全措施以防范常见攻击：

### 输入验证

所有用户输入在传递给 Redis 之前都会进行验证：

- **键验证**：键不能为空、不能超过 512KB、不能包含危险字符（`\r`、`\n`、`\0`），以防止 Redis 协议注入攻击。
- **Lua 脚本验证**：脚本验证包括：
  - 最大长度 10KB
  - 最多 100 个键
  - 阻止危险命令：`FLUSHALL`、`FLUSHDB`、`KEYS`、`SHUTDOWN`、`DEBUG`、`CONFIG`、`SAVE`、`BGSAVE`、`MONITOR`
  - 注释和字符串内容预处理，防止通过注释绕过检测
- **SCAN 模式验证**：模式验证以防止 ReDoS 攻击：
  - 最大长度 256 个字符
  - 最多 10 个通配符（`*`）字符
  - count 参数限制在安全范围内（1-1000）
- **SQL/路径遍历检测**：Redis 键会扫描潜在的 SQL 注入和路径遍历模式

### 安全 API（公共函数）

对于高级用例，您可以直接使用安全验证函数：

```rust
use oxcache::security::{validate_redis_key, validate_lua_script, validate_scan_pattern};

// 验证 Redis 键
validate_redis_key("user:123").expect("无效的键");

// 验证 Lua 脚本
validate_lua_script("return redis.call('GET', KEYS[1])", 1).expect("无效的脚本");

// 验证 SCAN 模式
validate_scan_pattern("user:*").expect("无效的模式");
```

### 超时保护

长时间运行的操作有超时保护：

- **Lua 脚本**：30 秒超时，防止 Redis 阻塞
- **SCAN 操作**：30 秒超时，防止扫描挂起

### 安全锁值

分布式锁使用库自动生成的加密安全 UUID v4 值，消除锁值预测攻击的风险。

### 连接字符串脱敏

连接字符串中的密码在日志中默认脱敏，以防止凭据泄露。使用 `normalize_connection_string_with_redaction()` 进行安全日志记录。

### 最佳实践

1. **使用库的键验证** - 不要绕过 `validate_redis_key()` 函数
2. **避免自定义 Lua 脚本** - 尽可能使用内置缓存操作
3. **设置适当的超时** - 不要禁用 30 秒默认超时
4. **轮换锁值** - 库会自动处理
5. **永远不要记录连接字符串** - 使用脱敏工具进行调试

更多详情请参阅 [安全文档](docs/SECURITY.md)。

## 📚 文档

- [📖 用户指南](docs/USER_GUIDE.md)
- [📘 API 文档](https://docs.rs/oxcache)
- [💻 示例代码](../examples/)

## 🤝 贡献

欢迎提交 Pull Request 和 Issue！

## 📝 更新日志

详见 [CHANGELOG.md](../CHANGELOG.md)

## 📄 许可证

本项目采用 MIT 许可证。详见 [LICENSE](../LICENSE) 文件。

---

<div align="center">

**如果这个项目对你有帮助，请给个 ⭐ Star 支持一下！**

Made with ❤️ by Kirky.X

</div>
