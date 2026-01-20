<div align="center">

# 📖 Oxcache 用户指南

### 高性能 Rust 双层缓存库完整使用指南

[🏠 首页](../README.md) • [📚 文档](README.md) • [🎯 示例](../examples/) • [❓ 常见问题](FAQ.md)

---

</div>

## 📋 目录

- [简介](#简介)
- [快速入门](#快速入门)
    - [先决条件](#先决条件)
    - [安装](#安装)
    - [第一步](#第一步)
- [核心概念](#核心概念)
- [基础用法](#基础用法)
    - [配置文件](#配置文件)
    - [使用缓存宏](#使用缓存宏)
    - [手动控制缓存](#手动控制缓存)
    - [序列化配置](#序列化配置)
- [高级用法](#高级用法)
    - [Redis 模式配置](#redis-模式配置)
    - [批量写入优化](#批量写入优化)
    - [监控指标](#监控指标)
    - [分布式追踪](#分布式追踪)
    - [优雅关闭](#优雅关闭)
- [最佳实践](#最佳实践)
- [故障排除](#故障排除)
- [后续步骤](#后续步骤)

---

## 简介

<div align="center">

### 🎯 你将学到什么

</div>

<table>
<tr>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/rocket.png" width="64"><br>
<b>快速入门</b><br>
5 分钟内完成环境搭建
</td>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/settings.png" width="64"><br>
<b>双层缓存</b><br>
L1 内存 + L2 分布式
</td>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/code.png" width="64"><br>
<b>宏支持</b><br>
一行代码启用缓存
</td>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/rocket-take-off.png" width="64"><br>
<b>高级特性</b><br>
故障恢复与监控
</td>
</tr>
</table>

**oxcache** 是一个高性能、生产级可用的 Rust 双层缓存库，提供 L1（进程内内存缓存，使用 Moka）+ L2（分布式 Redis 缓存）的双层架构。它通过
`#[cached]` 宏实现零侵入式缓存，并通过 Pub/Sub 机制确保多实例缓存一致性。

主要特性包括：

- **🚀 极致性能**：L1 纳秒级响应（P99 < 100ns），L2 毫秒级响应（P99 < 5ms）
- **🔄 自动故障恢复**：Redis 故障时自动降级，恢复后自动重放 WAL
- **🌐 多实例同步**：基于 Pub/Sub + 版本号的失效同步机制
- **🛡️ 生产级可靠**：完整的可观测性、健康检查、混沌测试验证

> 💡 **提示**: 本指南假设你具备基本的 Rust 知识。如果你是 Rust
> 新手，建议先阅读 [Rust 官方教程](https://doc.rust-lang.org/book/)。

---

## 快速入门

### 先决条件

在开始之前，请确保你已安装以下工具：

<table>
<tr>
<td width="50%">

**必选**

- ✅ Rust 1.75+ (stable)
- ✅ Cargo (随 Rust 一起安装)
- ✅ Git

</td>
<td width="50%">

**可选**

- 🔧 支持 Rust 的 IDE (如 VS Code + rust-analyzer)
- 🔧 Docker (用于容器化部署)
- 🔧 Redis 6.0+ (用于 L2 缓存测试)

</td>
</tr>
</table>

<details>
<summary><b>🔍 验证安装</b></summary>

```bash
# 检查 Rust 版本
rustc --version
# 预期: rustc 1.75.0 (或更高)

# 检查 Cargo 版本
cargo --version
# 预期: cargo 1.75.0 (或更高)
```

</details>

### 安装

在你的 `Cargo.toml` 中添加 `oxcache`：

```toml
[dependencies]
oxcache = "0.1.3"
```

> **注意**：`tokio` 和 `serde` 已默认包含，无需单独添加。

> **特性**：要使用 `#[cached]` 宏，需要启用 `macros` 特性：`oxcache = { version = "0.1.3", features = ["macros"] }`

#### 特性分层选择

```toml
# 完整特性（推荐）
oxcache = { version = "0.1.3", features = ["full"] }

# 核心功能（L1 + L2 缓存）
oxcache = { version = "0.1.3", features = ["core"] }

# 最小特性（仅 L1 缓存）
oxcache = { version = "0.1.3", features = ["minimal"] }

# 自定义选择
oxcache = { version = "0.1.3", features = ["core", "macros", "metrics"] }
```

#### 特性依赖说明

某些特性需要其他特性作为前置条件：

| 特性 | 前置要求 | 说明 |
|------|----------|------|
| `bloom-filter` | `l1-moka` | 缓存穿透保护 |
| `rate-limiting` | `l1-moka` | DoS 防护 |
| `wal-recovery` | `l2-redis` | 预写日志持久化 |
| `batch-write` | `l2-redis` | 优化的批量写入 |
| `cli` | `confers` | 命令行界面 |
| `database` | `l2-redis` | 数据库集成 |

如果需要最小依赖或自定义特性：

```toml
[dependencies]
oxcache = { version = "0.1.2", default-features = false }
```

或者使用命令行：

```bash
cargo add oxcache
```

### 第一步

让我们通过一个简单的例子来验证安装。我们将使用 `#[cached]` 宏来为函数添加缓存功能：

```rust
use oxcache::macros::cached;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
    id: u64,
    name: String,
}

// 使用 #[cached] 宏一行代码启用缓存
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
    // 初始化缓存（从配置文件加载）
    oxcache::init_from_file("config.toml").await?;
    
    // 第一次调用：执行函数逻辑 + 缓存结果（~100ms）
    let user = get_user(1).await?;
    println!("First call: {:?}", user);
    
    // 第二次调用：直接从缓存返回（~0.1ms）
    let cached_user = get_user(1).await?;
    println!("Cached call: {:?}", cached_user);
    
    Ok(())
}
```

创建对应的 `config.toml`：

```toml
[global]
default_ttl = 3600
health_check_interval = 30
serialization = "json"
enable_metrics = true

[services.user_cache]
cache_type = "two-level"
ttl = 600

  [services.user_cache.l1]
  max_capacity = 10000
  ttl = 300
  tti = 180
  initial_capacity = 1000

  [services.user_cache.l2]
  mode = "standalone"
  connection_string = "redis://127.0.0.1:6379"

  [services.user_cache.two_level]
  write_through = true
  promote_on_hit = true
```

---

## 核心概念

理解这些核心概念将帮助你更有效地使用 `oxcache`。

### 1️⃣ 双层缓存架构

`oxcache` 的核心是 L1 (Moka) + L2 (Redis) 两级缓存架构。L1 是本地内存缓存，访问速度极快；L2 是分布式缓存，支持多实例共享。

- **L1 (Moka)**: 进程内高速缓存，使用 LRU/TinyLFU 淘汰策略
- **L2 (Redis)**: 分布式共享缓存，支持 Sentinel/Cluster 模式

### 2️⃣ 缓存提升策略

当 L2 缓存中的数据被频繁访问时，会自动"提升"到 L1 缓存，减少 L2 的访问压力，提升整体性能。

### 3️⃣ 灵活的缓存类型

你可以配置不同的缓存类型：

- **two-level**: 双层缓存（L1 + L2）
- **l1-only**: 仅 L1 内存缓存
- **l2-only**: 仅 L2 分布式缓存

### 4️⃣ 容错与恢复

- **容错降级**: 当 L2 不可用时，自动降级到 L1 仅缓存模式
- **WAL 恢复**: 通过预写日志确保数据持久化
- **Single-Flight**: 防止缓存击穿（重复请求去重）

### 5️⃣ 缓存一致性

- **Pub/Sub 失效**: 基于 Redis Pub/Sub + 版本号的失效同步机制
- **手动控制**: 支持单独操作 L1 或 L2 缓存层

---

## 基础用法

### 配置文件

`oxcache` 使用 TOML 配置文件来管理缓存服务配置：

```toml
[global]
default_ttl = 300
health_check_interval = 60
serialization = "json"
enable_metrics = true

[services.my_service]
cache_type = "two-level"
promote_on_hit = true

  [services.my_service.l1]
  max_capacity = 10000
  ttl = 60

  [services.my_service.l2]
  mode = "standalone"
  connection_string = "redis://127.0.0.1:6379"

  [services.my_service.two_level]
  write_through = true
  promote_on_hit = true
  enable_batch_write = true
```

### 使用缓存宏

使用 `#[cached]` 宏为函数添加缓存功能：

```rust
use oxcache::macros::cached;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    id: u64,
    name: String,
}

// 自动缓存结果，缓存键为 "user:{id}"
#[cached(service = "user_cache", key = "user:{id}", ttl = 300)]
async fn get_user(id: u64) -> Result<User, String> {
    // 这里写你的业务逻辑，比如数据库查询
    database::query_user(id).await
}
```

### 手动控制缓存

你也可以绕过宏，直接使用客户端进行缓存操作：

```rust
use oxcache::{get_client, CacheOps};

#[tokio::main]
async fn main() {
    oxcache::init_from_file("config.toml").await.unwrap();
    
    let client = get_client("my_service").unwrap();
    
    // 标准操作：同时写入 L1 和 L2
    client.set("key", &"value", None).await.unwrap();
    
    let val: Option<String> = client.get("key").await.unwrap();
    assert_eq!(val, Some("value".to_string()));
    
    // 仅写入 L1（临时数据）
    client.set_l1_only("temp_key", &temp_data, Some(60)).await?;
    
    // 仅写入 L2（共享数据）
    client.set_l2_only("shared_key", &shared_data, Some(3600)).await?;
    
    // 删除缓存
    client.delete("key").await?;
    
    // 检查键是否存在
    let exists = client.exists("key").await?;
}
```

### 序列化配置

`oxcache` 支持多种序列化方式：

```toml
[global]
serialization = "json"  # 或 "bincode"
```

---

## 高级用法

### Redis 模式配置

oxcache 支持多种 Redis 部署模式：

#### Standalone 模式

```toml
[services.my_service.l2]
mode = "standalone"
connection_string = "redis://127.0.0.1:6379"
```

#### Sentinel 模式

```toml
[services.my_service.l2]
mode = "sentinel"

  [services.my_service.l2.sentinel]
  master_name = "mymaster"
  db = 0
  password = "your-password"

  [[services.my_service.l2.sentinel.nodes]]
  host = "127.0.0.1"
  port = 26379

  [[services.my_service.l2.sentinel.nodes]]
  host = "127.0.0.1"
  port = 26380
```

#### Cluster 模式

```toml
[services.my_service.l2]
mode = "cluster"

  [[services.my_service.l2.cluster.nodes]]
  host = "127.0.0.1"
  port = 6379

  [[services.my_service.l2.cluster.nodes]]
  host = "127.0.0.1"
  port = 6380
```

### 批量写入优化

启用批量写入可以显著提升写入性能：

```toml
[services.my_service.two_level]
enable_batch_write = true
batch_size = 100
batch_interval_ms = 50
```

### 监控指标

启用 `metrics` 特性后，可以获取缓存的运行指标：

```rust
use oxcache::metrics::MetricsCollector;

let metrics = MetricsCollector::new();
metrics.start_collection();

// 获取指标
let hit_rate = metrics.get_hit_rate()?;
let ops_count = metrics.get_ops_count()?;
```

**可用指标**:

- `cache_requests_total{service, layer, operation, result}`
- `cache_operation_duration_seconds{service, operation, layer}`
- `cache_l2_health_status{service}`
- `cache_wal_entries{service}`
- `cache_batch_buffer_size{service}`

### 分布式追踪

启用 OpenTelemetry 追踪：

```rust
use oxcache::telemetry::init_tracing;

init_tracing("my_app", Some("http://localhost:4317"));
```

### 优雅关闭

```rust
use oxcache::manager::shutdown_all;

#[tokio::main]
async fn main() {
    // 初始化缓存
    oxcache::init_from_file("config.toml").await.expect("Failed to init cache");
    
    // 你的应用逻辑
    
    // 优雅关闭
    shutdown_all().await.expect("Failed to shutdown cache clients");
}
```

关闭机制确保：

- 正确清理所有缓存客户端
- 资源释放
- 后台任务终止
- 错误聚合和报告

---

## 最佳实践

<div align="center">

### 🌟 推荐的设计模式

</div>

### ✅ 推荐做法

- **合理设置 TTL**: 根据数据更新频率设置缓存过期时间，避免数据不一致。
- **使用批量操作**: 对于大量写入场景，启用批量写入优化。
- **监控缓存命中率**: 定期检查缓存命中率，及时调整配置。
- **配置健康检查**: 启用健康检查以实现自动故障恢复。
- **分离冷热数据**: 使用 L1-only 缓存热数据，L2 缓存共享数据。

### ❌ 避免做法

- **缓存过大数据**: 避免缓存 large object，优先缓存元数据和 ID。
- **忽略过期策略**: 合理设置 TTL，避免缓存脏数据。
- **单点故障**: 生产环境务必使用 Sentinel 或 Cluster 模式。
- **忽视监控**: 启用指标收集和监控，及时发现问题。

### 🔒 安全最佳实践

OxCache 内置多层安全防护机制，建议在生产环境中遵循以下安全最佳实践：

#### 1. 敏感信息保护

- **使用环境变量存储密码**: 永远不要在配置文件中硬编码密码
  ```toml
  # ✅ 正确做法
  [services.my_cache]
  connection_string = "redis://:${REDIS_PASSWORD}@localhost:6379/0"
  
  # ❌ 错误做法
  connection_string = "redis://:mypassword123@localhost:6379/0"
  ```

- **启用日志脱敏**: OxCache 会自动脱敏日志中的敏感信息，确保生产环境日志不包含密码

#### 2. 连接安全

- **使用 TLS 加密**: 生产环境务必启用 TLS
  ```toml
  [services.my_cache.l2]
  connection_string = "rediss://:${REDIS_PASSWORD}@localhost:6380"
  enable_tls = true
  ```

- **使用强密码**: Redis 密码至少 32 位，包含大小写字母、数字和特殊字符

#### 3. 访问控制

- **配置网络隔离**: 使用防火墙限制 Redis 访问来源
- **使用 ACL**: 生产环境建议使用 Redis ACL 限制用户权限
- **定期轮换密码**: 定期更换 Redis 密码，建议每 90 天更换一次

#### 4. 审计与监控

- **启用安全日志**: 监控认证失败和异常访问
- **配置告警**: 对连接失败和认证失败设置告警
- **定期审计**: 定期检查日志和安全配置

#### 5. 开发环境安全

- **测试环境分离**: 使用独立的测试数据库，避免测试数据污染生产数据
- **清理测试数据**: 测试完成后及时清理测试数据
- **使用 SecretString**: 在代码中使用 `secrecy::SecretString` 存储敏感信息

---

## 故障排除

<details>
<summary><b>❓ 问题：缓存未命中率高</b></summary>

**解决方案**：

1. 检查 TTL 设置是否过短
2. 确认数据是否被频繁更新
3. 检查 promote_on_hit 是否启用
4. 调整 L1 缓存容量大小

</details>

<details>
<summary><b>❓ 问题：Redis 连接失败</b></summary>

**解决方案**：

1. 检查连接字符串是否正确
2. 确认 Redis 服务是否正常运行
3. 检查网络连接和防火墙设置
4. 验证用户名密码是否正确

</details>

<details>
<summary><b>❓ 问题：缓存数据不一致</b></summary>

**解决方案**：

1. 确认 Pub/Sub 机制是否正常
2. 检查版本号配置是否正确
3. 考虑使用较短的 TTL
4. 实现缓存更新时主动失效机制

</details>

<details>
<summary><b>❓ 问题：性能下降</b></summary>

**解决方案**：

1. 检查是否存在内存泄漏
2. 调整批量写入配置
3. 检查 L1 缓存容量是否合理
4. 分析慢查询日志

</details>

<div align="center">

**💬 仍然需要帮助？** [提交 Issue](https://github.com/Kirky-X/oxcache/issues) 或 [访问文档中心](https://docs.rs/oxcache)

</div>

---

## 后续步骤

<div align="center">

### 🎯 继续探索

</div>

<table>
<tr>
<td width="33%" align="center">
<a href="API_REFERENCE.md">
<img src="https://img.icons8.com/fluency/96/000000/graduation-cap.png" width="64"><br>
<b>📚 API 参考</b>
</a><br>
详细的接口说明
</td>
<td width="33%" align="center">
<a href="ARCHITECTURE.md">
<img src="https://img.icons8.com/fluency/96/000000/settings.png" width="64"><br>
<b>🔧 架构设计</b>
</a><br>
深入了解内部机制
</td>
<td width="33%" align="center">
<a href="../examples/">
<img src="https://img.icons8.com/fluency/96/000000/code.png" width="64"><br>
<b>💻 示例代码</b>
</a><br>
真实场景的代码样例
</td>
</tr>
</table>

---

<div align="center">

**[📖 API 文档](https://docs.rs/oxcache)** • **[❓ 常见问题](FAQ.md)** • *
*[🐛 报告问题](https://github.com/Kirky-X/oxcache/issues)**

由 oxcache Team 用 ❤️ 制作

[⬆ 回到顶部](#-用户指南)

</div>
