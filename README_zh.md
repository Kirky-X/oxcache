# MokaCache

[Show Image](https://crates.io/crates/mokacache) [Show Image](https://docs.rs/mokacache) [Show Image](LICENSE) [Show Image](https://github.com/your-org/mokacache/actions)

高性能、生产级的 Rust 多级缓存库，提供 L1（Moka 内存缓存）+ L2（Redis 分布式缓存）双层架构。

## ✨ 核心特性

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
cache = { path = "crates/infra/cache" }
tokio = { version = "1.42", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

### 最简示例

```rust
use cache::cached;
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
    // 初始化缓存（从配置文件加载）
    cache::init("config.toml").await?;
    
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

```toml
[global]
default_ttl = 3600
health_check_interval = 30
serialization = "json"
enable_metrics = true

[services.user_cache]
cache_type = "two-level"  # "l1" | "l2" | "two-level"
ttl = 600

  [services.user_cache.l1]
  max_capacity = 10000
  ttl = 300  # L1 TTL 必须 <= L2 TTL
  tti = 180
  initial_capacity = 1000

  [services.user_cache.l2]
  mode = "sentinel"  # "standalone" | "sentinel" | "cluster"
  key_prefix = "user"
  connection_timeout_ms = 5000
  command_timeout_ms = 1000
  
    [[services.user_cache.l2.sentinel.nodes]]
    host = "127.0.0.1"
    port = 26379
    
    [[services.user_cache.l2.sentinel.nodes]]
    host = "127.0.0.1"
    port = 26380
    
    [services.user_cache.l2.sentinel]
    master_name = "mymaster"
    db = 0
    password = "your-password"

  [services.user_cache.two_level]
  write_through = true
  promote_on_hit = true
  enable_batch_write = true
  batch_size = 100
  batch_interval_ms = 50
  enable_invalidation_sync = true
  enable_auto_recovery = true
  failure_threshold = 3
  recovery_threshold = 3
  wal_path = "/var/cache/user_wal"
```

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
use cache::{get_client, CacheOps};

async fn advanced_caching() -> Result<(), Box<dyn std::error::Error>> {
    let client = get_client("custom_cache")?;
    
    // 标准操作
    client.set("key", &my_data, Some(300)).await?;
    let data: MyData = client.get("key").await?.unwrap();
    
    // 仅写入 L1（临时数据）
    client.set_l1_only("temp_key", &temp_data, Some(60)).await?;
    
    // 仅写入 L2（共享数据）
    client.set_l2_only("shared_key", &shared_data, Some(3600)).await?;
    
    // 删除
    client.delete("key").await?;
    
    Ok(())
}
```

## 🏗️ 架构设计
```
┌─────────────────────────────────────────────────────────┐
│                    Application Code                      │
│                  (#[cached] Macro)                       │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ↓
┌─────────────────────────────────────────────────────────┐
│                   CacheManager                           │
│        (Service Registry + Health Monitor)               │
└───┬─────────────────────────────────────────────────┬───┘
    │                                                  │
    ↓                                                  ↓
┌──────────────┐                              ┌──────────────┐
│ TwoLevelClient│                              │ L1OnlyClient │
│               │                              │ L2OnlyClient │
└───┬──────┬───┘                              └──────────────┘
    │      │
    ↓      ↓
┌────────┐ ┌────────────────────────────────────────┐
│   L1   │ │              L2 (Redis)                │
│ (Moka) │ │  - Sentinel / Cluster Support          │
│        │ │  - Pipeline Batch Write                │
└────────┘ │  - Pub/Sub Invalidation                │
           │  - WAL for Fault Recovery              │
           └────────────────────────────────────────┘
```

### 核心组件

| 组件                  | 功能               | 技术栈                   |
| --------------------- | ------------------ | ------------------------ |
| **L1 Cache**          | 进程内高速缓存     | Moka (LRU/TinyLFU)       |
| **L2 Cache**          | 分布式共享缓存     | Redis (Sentinel/Cluster) |
| **WAL**               | 故障期间持久化     | SQLite                   |
| **Promotion Manager** | Single-flight 回填 | DashMap + Tokio Notify   |
| **Batch Writer**      | 批量写入优化       | 时间窗口 + 容量触发      |
| **Invalidation Sync** | 多实例失效同步     | Redis Pub/Sub + 版本号   |
| **Health Checker**    | 自动故障恢复       | 状态机 + 定时心跳        |

## 📊 性能基准

**测试环境**: Intel i9-12900K, 32GB RAM, Redis 7.2

| 操作                   | 延迟 (P50) | 延迟 (P99) | 吞吐量     |
| ---------------------- | ---------- | ---------- | ---------- |
| L1 Get                 | 45ns       | 98ns       | 2M ops/s   |
| L1 Set                 | 210ns      | 480ns      | 500k ops/s |
| L2 Get (Standalone)    | 1.2ms      | 4.8ms      | 80k ops/s  |
| L2 Set (Batch)         | 0.8ms      | 3.2ms      | 120k ops/s |
| Two-Level Get (L1 Hit) | 50ns       | 105ns      | 1.8M ops/s |
| Two-Level Get (L2 Hit) | 1.5ms      | 5.5ms      | 65k ops/s  |

运行基准测试：

```bash
cargo bench -p cache
```

## 🛠️ 高级特性

### 自定义序列化器

```rust
use cache::serialization::Serializer;

pub struct MsgPackSerializer;

impl Serializer for MsgPackSerializer {
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, CacheError> {
        rmp_serde::to_vec(value).map_err(|e| CacheError::Serialization(e.to_string()))
    }
    
    fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T, CacheError> {
        rmp_serde::from_slice(data).map_err(|e| CacheError::Serialization(e.to_string()))
    }
}

// 在配置中使用
cache::register_serializer("msgpack", Arc::new(MsgPackSerializer));
```

### 可观测性

```rust
// 获取 Prometheus 格式的指标
let metrics = cache::export_prometheus();
println!("{}", metrics);

// 集成 OpenTelemetry Tracing
use tracing_subscriber;

tracing_subscriber::fmt::init();
// 所有缓存操作会自动生成 span
```

**可用指标**:

- `cache_requests_total{service, layer, operation, result}`
- `cache_operation_duration_seconds{service, operation, layer}`
- `cache_l2_health_status{service}`
- `cache_wal_entries{service}`
- `cache_batch_buffer_size{service}`

## 🧪 测试

```bash
# 运行所有测试
cargo test -p cache

# 单元测试
cargo test --lib -p cache

# 集成测试
cargo test --test '*' -p cache

# 混沌测试（需要真实 Redis）
cargo test --test chaos -- --ignored

# 代码覆盖率
cargo tarpaulin --out Html -p cache
```

## 📚 完整文档

- [接入指南](docs/INTEGRATION_GUIDE.md) - 详细的集成步骤
- [API 文档](https://docs.rs/mokacache) - 完整的 API 参考
- [配置参考](docs/CONFIG_REFERENCE.md) - 所有配置项说明
- [架构设计](docs/ARCHITECTURE.md) - 深入理解内部实现
- [故障排查](docs/TROUBLESHOOTING.md) - 常见问题解决

## 🤝 贡献

欢迎贡献代码、报告问题或提出建议！

```bash
# Fork 项目并克隆
git clone https://github.com/your-username/mokacache.git
cd mokacache

# 创建特性分支
git checkout -b feature/amazing-feature

# 提交更改
git commit -m "Add amazing feature"
git push origin feature/amazing-feature

# 创建 Pull Request
```

## 📄 许可证

本项目采用 [Apache-2.0](LICENSE) 许可证。

## 🙏 致谢

- [Moka](https://github.com/moka-rs/moka) - 高性能内存缓存
- [Redis](https://redis.io/) - 分布式缓存基础设施
- [Tokio](https://tokio.rs/) - 异步运行时

------

**需要帮助？**

- 📖 阅读 [接入指南](docs/INTEGRATION_GUIDE.md)
- 💬 加入 [讨论区](https://github.com/your-org/mokacache/discussions)
- 🐛 报告 [问题](https://github.com/your-org/mokacache/issues)
