# MokaCache 接入指南

**版本**: 3.0
 **更新日期**: 2024-12-11

本文档提供 MokaCache 的完整接入指南，涵盖从环境准备到生产部署的全流程。

------

## 📋 目录

1. [环境准备](#1-环境准备)
2. [基础接入](#2-基础接入)
3. [配置详解](#3-配置详解)
4. [使用模式](#4-使用模式)
5. [高级特性](#5-高级特性)
6. [生产部署](#6-生产部署)
7. [监控告警](#7-监控告警)
8. [故障排查](#8-故障排查)
9. [性能调优](#9-性能调优)
10. [最佳实践](#10-最佳实践)

------

## 1. 环境准备

### 1.1 系统要求

| 组件     | 版本要求            | 说明                      |
| -------- | ------------------- | ------------------------- |
| Rust     | ≥ 1.75              | 支持最新 async/await 特性 |
| Tokio    | ≥ 1.42              | 异步运行时                |
| Redis    | ≥ 6.0               | 建议 7.0+ 以获得更好性能  |
| 操作系统 | Linux/macOS/Windows | 生产环境推荐 Linux        |

### 1.2 依赖安装

**步骤 1**: 在 `Cargo.toml` 中添加依赖

```toml
[dependencies]
# 核心依赖
cache = { path = "crates/infra/cache" }
tokio = { version = "1.42", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }

# 可选：序列化优化
bincode = { version = "1.3", optional = true }

# 可选：可观测性
tracing = "0.1"
tracing-subscriber = "0.3"
```

**步骤 2**: 启用 Feature Flags（如需要）

```toml
[features]
default = ["json-serialization"]
json-serialization = []
bincode-serialization = ["bincode"]
metrics = []
```

### 1.3 Redis 部署

#### 选项 A: Docker 快速启动（开发环境）

```bash
# Standalone 模式
docker run -d --name redis \
  -p 6379:6379 \
  redis:7.2-alpine

# Sentinel 模式
docker-compose up -d
```

`docker-compose.yml` 示例：

```yaml
version: '3.8'
services:
  redis-master:
    image: redis:7.2-alpine
    ports:
      - "6379:6379"
    command: redis-server --appendonly yes

  redis-slave:
    image: redis:7.2-alpine
    command: redis-server --slaveof redis-master 6379 --appendonly yes
    depends_on:
      - redis-master

  redis-sentinel:
    image: redis:7.2-alpine
    command: >
      bash -c "echo 'sentinel monitor mymaster redis-master 6379 2
               sentinel down-after-milliseconds mymaster 5000
               sentinel parallel-syncs mymaster 1
               sentinel failover-timeout mymaster 10000' > /tmp/sentinel.conf &&
               redis-sentinel /tmp/sentinel.conf"
    ports:
      - "26379:26379"
    depends_on:
      - redis-master
```

#### 选项 B: 生产环境部署

参考 [Redis 官方文档](https://redis.io/docs/management/sentinel/) 配置 Sentinel 或 Cluster。

------

## 2. 基础接入

### 2.1 最小化配置

**步骤 1**: 创建配置文件 `config.toml`

```toml
[global]
default_ttl = 3600

[services.default]
cache_type = "two-level"

  [services.default.l1]
  max_capacity = 1000
  ttl = 300

  [services.default.l2]
  mode = "standalone"
  key_prefix = "app"
  
    [services.default.l2.standalone]
    host = "127.0.0.1"
    port = 6379
    db = 0

  [services.default.two_level]
  write_through = true
  promote_on_hit = true
```

**步骤 2**: 初始化缓存

```rust
use cache;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 从文件加载配置
    cache::init("config.toml").await?;
    
    // 或使用 Builder 模式
    use cache::{Config, CacheType};
    cache::init_with_config(Config::builder()
        .service("default")
        .cache_type(CacheType::TwoLevel)
        .l1_max_capacity(1000)
        .l2_url("redis://127.0.0.1:6379")
        .build()
    ).await?;
    
    // 启动应用逻辑
    run_app().await?;
    
    Ok(())
}
```

**步骤 3**: 使用宏启用缓存

```rust
use cache::cached;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Product {
    id: u64,
    name: String,
    price: f64,
}

#[cached(ttl = 600)]
async fn get_product(id: u64) -> Result<Product, String> {
    // 模拟数据库查询
    database::find_product(id).await
}

// 使用
let product = get_product(123).await?;
```

### 2.2 验证接入

**测试代码**:

```rust
#[tokio::test]
async fn test_cache_integration() {
    cache::init("config.toml").await.unwrap();
    
    // 第一次调用（缓存未命中）
    let start = std::time::Instant::now();
    let result = get_product(1).await.unwrap();
    let first_duration = start.elapsed();
    
    // 第二次调用（缓存命中）
    let start = std::time::Instant::now();
    let cached_result = get_product(1).await.unwrap();
    let second_duration = start.elapsed();
    
    assert_eq!(result.id, cached_result.id);
    assert!(second_duration < first_duration / 10); // 至少快 10 倍
}
```

------

## 3. 配置详解

### 3.1 全局配置 (`[global]`)

```toml
[global]
# 默认 TTL（秒），当服务未指定时使用
default_ttl = 3600

# 健康检查间隔（秒）
health_check_interval = 30

# 全局序列化方式："json" | "bincode"
serialization = "json"

# 是否启用 Metrics 收集
enable_metrics = true
```

### 3.2 服务配置 (`[services.xxx]`)

#### 3.2.1 缓存类型

```toml
[services.my_service]
# 缓存类型："l1" | "l2" | "two-level"
cache_type = "two-level"

# 服务级默认 TTL（覆盖全局配置）
ttl = 600

# 服务级序列化方式（覆盖全局配置）
serialization = "bincode"
```

#### 3.2.2 L1 配置

```toml
[services.my_service.l1]
# 最大条目数（LRU 淘汰）
max_capacity = 10000

# 过期时间（秒）
ttl = 300

# 空闲淘汰时间（秒），超过此时间未访问则淘汰
tti = 180

# 初始容量（预分配，减少 rehash）
initial_capacity = 1000
```

**容量规划建议**:

- 小型应用: `max_capacity = 1000`
- 中型应用: `max_capacity = 10000`
- 大型应用: `max_capacity = 100000`
- 内存估算: 平均每条目 ~500 bytes（含开销）

#### 3.2.3 L2 配置

**Standalone 模式**:

```toml
[services.my_service.l2]
mode = "standalone"
key_prefix = "myapp"  # Redis key 前缀，建议设置避免冲突
connection_timeout_ms = 5000
command_timeout_ms = 1000

  [services.my_service.l2.standalone]
  host = "127.0.0.1"
  port = 6379
  db = 0
  password = "your-password"  # 可选
```

**Sentinel 模式**:

```toml
[services.my_service.l2]
mode = "sentinel"
key_prefix = "myapp"

  [[services.my_service.l2.sentinel.nodes]]
  host = "192.168.1.10"
  port = 26379
  
  [[services.my_service.l2.sentinel.nodes]]
  host = "192.168.1.11"
  port = 26379
  
  [[services.my_service.l2.sentinel.nodes]]
  host = "192.168.1.12"
  port = 26379
  
  [services.my_service.l2.sentinel]
  master_name = "mymaster"
  db = 0
  password = "your-password"
```

**Cluster 模式**:

```toml
[services.my_service.l2]
mode = "cluster"
key_prefix = "myapp"

  [[services.my_service.l2.cluster.nodes]]
  host = "192.168.1.20"
  port = 7000
  
  [[services.my_service.l2.cluster.nodes]]
  host = "192.168.1.21"
  port = 7001
  
  # ... 更多节点
```

#### 3.2.4 双层缓存配置

```toml
[services.my_service.two_level]
# 写操作是否同步写入 L2（true=强一致性，false=最终一致性）
write_through = true

# L2 命中时是否回填 L1
promote_on_hit = true

# 是否启用批量写入优化
enable_batch_write = true

# 批量写入缓冲区大小
batch_size = 100

# 批量写入时间窗口（毫秒）
batch_interval_ms = 50

# 是否启用多实例失效同步
enable_invalidation_sync = true

# 是否启用自动故障恢复
enable_auto_recovery = true

# 连续失败多少次后降级
failure_threshold = 3

# 连续成功多少次后恢复
recovery_threshold = 3

# WAL 文件路径
wal_path = "/var/cache/my_service_wal"
```

### 3.3 完整配置示例

```toml
[global]
default_ttl = 3600
health_check_interval = 30
serialization = "json"
enable_metrics = true

# 用户服务缓存（双层 + 高一致性）
[services.user_cache]
cache_type = "two-level"
ttl = 600

  [services.user_cache.l1]
  max_capacity = 10000
  ttl = 300
  tti = 180

  [services.user_cache.l2]
  mode = "sentinel"
  key_prefix = "user"
  
    [[services.user_cache.l2.sentinel.nodes]]
    host = "127.0.0.1"
    port = 26379
    
    [services.user_cache.l2.sentinel]
    master_name = "mymaster"
    db = 0

  [services.user_cache.two_level]
  write_through = true
  promote_on_hit = true
  enable_batch_write = true
  enable_invalidation_sync = true
  enable_auto_recovery = true

# 会话缓存（仅 L1）
[services.session_cache]
cache_type = "l1"
ttl = 60

  [services.session_cache.l1]
  max_capacity = 50000
  ttl = 60
  tti = 30

# 配置缓存（仅 L2）
[services.config_cache]
cache_type = "l2"
ttl = 7200

  [services.config_cache.l2]
  mode = "standalone"
  key_prefix = "config"
  
    [services.config_cache.l2.standalone]
    host = "127.0.0.1"
    port = 6379
```

------

## 4. 使用模式

### 4.1 基础宏用法

#### 4.1.1 简单缓存

```rust
// 使用默认 service
#[cached]
async fn get_user(id: u64) -> Result<User, Error> {
    database::query("SELECT * FROM users WHERE id = ?", id).await
}
```

#### 4.1.2 指定 Service 和 TTL

```rust
#[cached(service = "user_cache", ttl = 600)]
async fn get_user_profile(user_id: u64) -> Result<UserProfile, Error> {
    database::query_user_profile(user_id).await
}
```

#### 4.1.3 自定义 Key

```rust
// 单参数
#[cached(service = "order_cache", key = "order_{order_id}")]
async fn get_order(order_id: u64) -> Result<Order, Error> {
    database::find_order(order_id).await
}

// 多参数
#[cached(service = "product_cache", key = "product_{category}_{id}")]
async fn get_product_by_category(category: String, id: u64) -> Result<Product, Error> {
    database::find_product(category, id).await
}
```

#### 4.1.4 指定缓存层

```rust
// 仅 L1（临时数据）
#[cached(service = "temp_cache", cache_type = "l1", ttl = 60)]
async fn get_temp_data(key: String) -> Result<Data, Error> {
    compute_temp_data(key).await
}

// 仅 L2（共享数据）
#[cached(service = "shared_cache", cache_type = "l2", ttl = 3600)]
async fn get_shared_config(key: String) -> Result<Config, Error> {
    fetch_from_config_center(key).await
}
```

### 4.2 手动API 用法

#### 4.2.1 获取 Client

```rust
use cache::{get_client, CacheOps};

let client = get_client("user_cache")?;
```

#### 4.2.2 基础操作

```rust
// 写入
client.set("user:123", &user, Some(600)).await?;

// 读取
let user: User = client.get("user:123").await?.unwrap();

// 删除
client.delete("user:123").await?;

// 判断存在
let exists = client.exists("user:123").await?;
```

#### 4.2.3 指定缓存层

```rust
// 仅写入 L1
client.set_l1_only("session:abc", &session, Some(60)).await?;

// 仅写入 L2
client.set_l2_only("config:db", &db_config, Some(3600)).await?;

// 同时写入，但使用不同 TTL
client.set_both(
    "key",
    &value,
    Some(300),  // L1 TTL
    Some(3600), // L2 TTL
).await?;
```

### 4.3 批量操作

```rust
use cache::{get_client, CacheOps};

async fn batch_load_users(ids: Vec<u64>) -> Result<Vec<User>, Error> {
    let client = get_client("user_cache")?;
    let mut users = Vec::new();
    
        for id in ids {
            let key = format!("user:{}", id);

        // 尝试从缓存获取
        if let Some(user) = client.get::<User>(&key).await? {
            users.push(user);
        } else {
            // 缓存未命中，从数据库加载
            let user = database::find_user(id).await?;

            // 异步写入缓存（不阻塞）
            let client_clone = client.clone();
            let key_clone = key.clone();
            let user_clone = user.clone();
            tokio::spawn(async move {
                let _ = client_clone.set(&key_clone, &user_clone, Some(600)).await;
            });

            users.push(user);
        }
    }
    Ok(users)
}

```

---

## 5. 高级特性

### 5.1 自定义序列化器

**实现 Serializer Trait**:

```rust
use cache::serialization::Serializer;
use serde::{Serialize, de::DeserializeOwned};
use cache::CacheError;

pub struct MsgPackSerializer;

impl Serializer for MsgPackSerializer {
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, CacheError> {
        rmp_serde::to_vec(value)
            .map_err(|e| CacheError::Serialization(e.to_string()))
    }
    
    fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T, CacheError> {
        rmp_serde::from_slice(data)
            .map_err(|e| CacheError::Serialization(e.to_string()))
    }
}
```

**注册并使用**:

```rust
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 注册自定义序列化器
    cache::register_serializer("msgpack", Arc::new(MsgPackSerializer));
    
    // 在配置中使用
    cache::init("config.toml").await?;
    
    Ok(())
}
```

**配置文件**:

```toml
[services.my_service]
serialization = "msgpack"
# ...
```

### 5.2 条件缓存

```rust
#[cached(service = "product_cache", ttl = 600)]
async fn get_product(id: u64, include_details: bool) -> Result<Product, Error> {
    if include_details {
        // 详细信息不缓存
        return database::query_product_with_details(id).await;
    }
    
    // 基础信息缓存
    database::query_product_basic(id).await
}
```

### 5.3 缓存穿透防护

```rust
use cache::{get_client, CacheOps};

async fn get_user_safe(id: u64) -> Result<Option<User>, Error> {
    let client = get_client("user_cache")?;
    let key = format!("user:{}", id);
    
    // 尝试从缓存获取
    if let Some(user) = client.get::<User>(&key).await? {
        return Ok(Some(user));
    }
    
    // 从数据库查询
    let user_opt = database::find_user(id).await?;
    
    if let Some(ref user) = user_opt {
        // 用户存在，缓存
        client.set(&key, user, Some(600)).await?;
    } else {
        // 用户不存在，缓存空值（防止穿透）
        client.set(&key, &Option::<User>::None, Some(60)).await?;
    }
    
    Ok(user_opt)
}
```

### 5.4 缓存预热

```rust
async fn warmup_cache() -> Result<(), Error> {
    let client = get_client("product_cache")?;
    
    // 查询热门商品 ID
    let hot_product_ids = database::query_hot_products(100).await?;
    
    // 批量预热
    for id in hot_product_ids {
        let product = database::find_product(id).await?;
        client.set(&format!("product:{}", id), &product, Some(3600)).await?;
    }
    
    Ok(())
}
```

---

## 6. 生产部署

### 6.1 容器化部署

**Dockerfile**:

```dockerfile
FROM rust:1.75-alpine AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p your-app

FROM alpine:latest
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/target/release/your-app /usr/local/bin/
COPY config.toml /etc/your-app/config.toml
ENV CONFIG_PATH=/etc/your-app/config.toml
CMD ["your-app"]
```

**docker-compose.yml**:

```yaml
version: '3.8'
services:
  app:
    image: your-app:latest
    environment:
      - CONFIG_PATH=/etc/config.toml
      - RUST_LOG=info
    volumes:
      - ./config.toml:/etc/config.toml:ro
      - ./wal:/var/cache/wal
    depends_on:
      - redis
      
  redis:
    image: redis:7.2-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data
      
volumes:
  redis-data:
```

### 6.2 Kubernetes 部署

**ConfigMap**:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: cache-config
data:
  config.toml: |
    [global]
    default_ttl = 3600
    
    [services.user_cache]
    cache_type = "two-level"
    # ...
```

**Deployment**:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: your-app
spec:
  replicas: 3
  selector:
    matchLabels:
      app: your-app
  template:
    metadata:
      labels:
        app: your-app
    spec:
      containers:
      - name: app
        image: your-app:latest
        env:
        - name: CONFIG_PATH
          value: /etc/config/config.toml
        volumeMounts:
        - name: config
          mountPath: /etc/config
          readOnly: true
        - name: wal
          mountPath: /var/cache/wal
      volumes:
      - name: config
        configMap:
          name: cache-config
      - name: wal
        emptyDir: {}
```

### 6.3 环境变量覆盖

```rust
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config.toml".to_string());
    
    cache::init(&config_path).await?;
    
    // 或使用环境变量直接构建配置
    let redis_url = env::var("REDIS_URL")?;
    cache::init_with_config(Config::builder()
        .l2_url(&redis_url)
        .build()
    ).await?;
    
    Ok(())
}
```

---

## 7. 监控告警

### 7.1 Prometheus 集成

**暴露指标端点**:

```rust
use axum::{Router, routing::get};

async fn metrics_handler() -> String {
    cache::export_prometheus()
}

#[tokio::main]
async fn main() {
    cache::init("config.toml").await.unwrap();
    
    let app = Router::new()
        .route("/metrics", get(metrics_handler));
    
    axum::Server::bind(&"0.0.0.0:9090".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
```

**Prometheus 配置** (`prometheus.yml`):

```yaml
scrape_configs:
  - job_name: 'your-app'
    static_configs:
      - targets: ['localhost:9090']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

### 7.2 关键指标

| 指标名称                           | 类型      | 说明                                              |
| ---------------------------------- | --------- | ------------------------------------------------- |
| `cache_requests_total`             | Counter   | 请求总数 (按 service/layer/operation/result 分组) |
| `cache_operation_duration_seconds` | Histogram | 操作延迟分布                                      |
| `cache_l2_health_status`           | Gauge     | L2 健康状态 (1=健康, 0=降级)                      |
| `cache_wal_entries`                | Gauge     | WAL 条目数量                                      |
| `cache_batch_buffer_size`          | Gauge     | 批量写入缓冲区大小                                |

### 7.3 Grafana Dashboard

**示例 PromQL 查询**:

```promql
# L1 命中率
sum(rate(cache_requests_total{layer="l1",result="hit"}[5m])) 
/ 
sum(rate(cache_requests_total{layer="l1"}[5m]))

# P99 延迟
histogram_quantile(0.99, sum(rate(cache_operation_duration_seconds_bucket[5m])) by (le, operation))

# 降级实例数
count(cache_l2_health_status == 0)
```

### 7.4 告警规则

**Prometheus Alert Rules**:

```yaml
groups:
- name: cache_alerts
  rules:
  - alert: CacheL1HitRateLow
    expr: |
      sum(rate(cache_requests_total{layer="l1",result="hit"}[5m])) 
      / 
      sum(rate(cache_requests_total{layer="l1"}[5m])) < 0.8
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "L1 缓存命中率低于 80%"
      
  - alert: CacheL2Degraded
    expr: cache_l2_health_status == 0
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "Redis 缓存已降级"
      
  - alert: CacheWALBacklog
    expr: cache_wal_entries > 1000
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "WAL 积压超过 1000 条"
```

---

## 8. 故障排查

### 8.1 常见问题

#### 问题 1: 配置文件加载失败

**错误信息**:

````
Error: ConfigError("Failed to parse config.toml: ...")
````

**解决方法**:

```bash
# 验证 TOML 语法
toml-fmt config.toml --check

# 检查文件路径
ls -l config.toml

# 检查文件权限
chmod 644 config.toml
```

#### 问题 2: Redis 连接失败

**错误信息**:

````
Error: L2Error("Failed to connect to Redis: Connection refused")
````

**排查步骤**:

```bash
# 1. 检查 Redis 服务状态
redis-cli ping

# 2. 检查防火墙
telnet 127.0.0.1 6379

# 3. 检查配置中的地址和端口
grep -A 5 "\[services.*.l2\]" config.toml

# 4. 查看应用日志
RUST_LOG=debug cargo run
```

#### 问题 3: 缓存命中率低

**排查步骤**:

```rust
// 1. 检查 TTL 配置是否过短
// 2. 查看是否频繁删除
// 3. 检查 key 生成逻辑

// 添加日志
#[cached(service = "test", ttl = 600)]
async fn get_data(id: u64) -> Result<Data, Error> {
    tracing::info!("Cache miss for id: {}", id);
    database::query(id).await
}
```

#### 问题 4: 内存占用过高

**解决方法**:

```toml
# 减小 L1 容量
[services.xxx.l1]
max_capacity = 1000  # 从 10000 减小到 1000

# 启用 TTI 自动清理
tti = 120
```

### 8.2 调试模式

**启用详细日志**:

```bash
RUST_LOG=cache=debug,your_app=info cargo run
```

**日志输出示例**:

````
[DEBUG cache::client::two_level] L1 miss for key: user:123 [DEBUG cache::client::two_level] L2 hit for key: user:123, promoting to L1 [INFO  cache::recovery::health] L2 health check passed
````

### 8.3 性能分析

```bash
# 使用 flamegraph 分析
cargo flamegraph --bin your-app

# 使用 perf
cargo build --release
perf record --call-graph dwarf ./target/release/your-app
perf report
```

---

## 9. 性能调优

### 9.1 L1 调优

**场景 1: 高并发读取**

```toml
[services.xxx.l1]
max_capacity = 100000  # 增大容量
initial_capacity = 50000  # 预分配
ttl = 600  # 适度延长 TTL
```

**场景 2: 内存受限**

```toml
[services.xxx.l1]
max_capacity = 1000  # 减小容量
tti = 60  # 启用空闲淘汰
```

### 9.2 L2 调优

**场景 1: 高吞吐写入**

```toml
[services.xxx.two_level]
enable_batch_write = true
batch_size = 500  # 增大批量大小
batch_interval_ms = 100  # 延长时间窗口
```

**场景 2: 低延迟要求**

```toml
[services.xxx.two_level]
enable_batch_write = false  # 禁用批量写入
promote_on_hit = false  # 禁用回填，减少写入

[services.xxx.l2]
command_timeout_ms = 500  # 减小超时
```

### 9.3 序列化调优

**JSON vs Bincode 对比**:

| 序列化方式 | 性能 | 空间 | 兼容性            |
| ---------- | ---- | ---- | ----------------- |
| JSON       | 中等 | 较大 | 优秀（跨语言）    |
| Bincode    | 快   | 小   | 一般（Rust 专用） |

**切换到 Bincode**:

```toml
[services.xxx]
serialization = "bincode"
```

### 9.4 连接池调优

```rust
// 自定义 Redis 连接池配置
use cache::Config;

let config = Config::builder()
    .l2_pool_size(50)  // 连接池大小
    .l2_pool_timeout_ms(1000)  // 获取连接超时
    .build();
```

---

## 10. 最佳实践

### 10.1 Key 设计

**推荐模式**:

````
{service}:{entity}:{id} {service}:{entity}:{id}:{field}
````

**示例**:

```rust
// ✅ 好的设计
"user:profile:123"
"product:detail:456:price"

// ❌ 不好的设计
"user_123"  // 缺少命名空间
"very_long_key_with_redundant_information_123"  // 过长
```

### 10.2 TTL 设计

| 数据类型 | 建议 TTL   | 说明             |
| -------- | ---------- | ---------------- |
| 用户信息 | 10-30 分钟 | 平衡一致性和性能 |
| 商品详情 | 1-6 小时   | 较少变化         |
| 配置信息 | 24 小时    | 极少变化         |
| 会话数据 | 1-5 分钟   | 临时数据         |
| 统计数据 | 5-15 分钟  | 允许延迟         |

### 10.3 错误处理

```rust
#[cached(service = "user_cache", ttl = 600)]
async fn get_user(id: u64) -> Result<User, AppError> {
    database::find_user(id).await.map_err(|e| {
        tracing::error!("Failed to load user {}: {}", id, e);
        AppError::DatabaseError(e)
    })
}

// 调用方
match get_user(123).await {
    Ok(user) => { /* ... */ }
    Err(e) => {
        // 缓存失败不影响业务逻辑
        tracing::warn!("User load error: {}", e);
        // 降级处理
    }
}
```

### 10.4 安全建议

**1. 敏感数据加密**:

```rust
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use cache::serialization::Serializer;

pub struct EncryptedSerializer {
    inner: Box<dyn Serializer>,
    cipher: Aes256Gcm,
}

impl Serializer for EncryptedSerializer {
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, CacheError> {
        let plaintext = self.inner.serialize(value)?;
        // 加密逻辑
        Ok(ciphertext)
    }
    
    // ...
}
```

**2. 访问控制**:

```toml
[services.sensitive_cache.l2.standalone]
password = "${REDIS_PASSWORD}"  # 从环境变量读取
```

**3. TLS 连接**:

```toml
[services.xxx.l2]
enable_tls = true
tls_cert_path = "/etc/certs/redis.crt"
```

### 10.5 容量规划

**估算公式**:

```
L1 内存占用 ≈ max_capacity × 平均 value 大小 × 1.5 (开销) L2 内存占用 ≈ 预期 key 数量 × 平均 value 大小 × 1.2 (开销)
```

**示例**:

```
假设：
- L1 max_capacity = 10000
- 平均 value 大小 = 500 bytes

L1 内存 ≈ 10000 × 500 × 1.5 = 7.5 MB
```

---

## 附录

### A. 完整配置模板

参见 `config.toml.example`

### B. 故障排查清单

```
□ 检查配置文件语法 
□ 验证 Redis 连接 
□ 查看应用日志 (RUST_LOG=debug) 
□ 检查 Prometheus 指标 
□ 验证 TTL 配置 
□ 检查内存使用 
□ 查看 WAL 文件大小
□ 确认网络连接
````

### C. 性能基准参考

运行本地基准测试：

```bash
cd crates/infra/cache
cargo bench
```

查看报告：`target/criterion/report/index.html`

---

**技术支持**:

- 📖 完整文档: https://docs.rs/mokacache
- 💬 讨论区: https://github.com/your-org/mokacache/discussions
- 🐛 问题报告: https://github.com/your-org/mokacache/issues

**版本历史**:

- v3.0 (2024-12-11): 初始发布