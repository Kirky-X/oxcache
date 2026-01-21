# Oxcache 特性标志分析

## 概述

Oxcache 支持 20+ 个特性标志，分为三个层级：minimal、core 和 full。

## 特性层级

### 1. Minimal 特性集

仅提供 L1 内存缓存，不依赖 Redis 或外部服务。

**依赖特性**：
- `l1-moka` - L1 缓存实现（Moka）
- `tokio/time` - Tokio 时间功能
- `dep:tracing` - 日志追踪
- `metrics` - 基础指标
- `serialization` - 序列化支持
- `chrono` - 时间处理

**适用场景**：
- 单机应用，不需要分布式缓存
- 对性能要求极高的场景
- 不需要持久化或跨实例共享缓存

### 2. Core 特性集

提供 L1 + L2 基本功能（Redis）。

**依赖特性**：
- `minimal` 的所有依赖
- `l2-redis` - L2 缓存实现（Redis）
- `futures` - 异步工具

**适用场景**：
- 需要分布式缓存的应用
- 需要跨实例共享缓存
- 需要数据持久化

### 3. Full 特性集

启用所有功能。

**依赖特性**：
- `core` 的所有依赖
- `macros` - 缓存宏
- `bloom-filter` - 布隆过滤器
- `rate-limiting` - 速率限制
- `wal-recovery` - 预写日志恢复
- `database` - 数据库集成
- `cli` - CLI 工具
- `full-metrics` - 完整指标
- `batch-write` - 批量写入
- `confers` - 配置管理
- `codegen` - 代码生成
- `compression` - 压缩
- `smart-strategy` - 智能策略
- `redis-native` - Redis 原生支持
- `enhanced-stats` - 增强统计
- `ttl-control` - TTL 控制
- `tiered-cache` - 分层缓存
- `extra-serialization` - 额外序列化
- `config-dynamic` - 动态配置
- `http-cache` - HTTP 缓存
- `serialization-cache` - 序列化缓存

**适用场景**：
- 生产环境
- 需要所有功能的应用
- 需要高级特性的应用

## 组件特性

### 缓存实现
- `l1-moka` - L1 缓存实现（Moka）
- `l2-redis` - L2 缓存实现（Redis）
- `tiered-cache` - 分层缓存（L1 + L2）

### 序列化
- `serialization` - 基础序列化（serde + serde_json）
- `bincode` - 二进制序列化
- `compression` - 压缩（flate2）
- `extra-serialization` - 额外序列化（MessagePack, CBOR）
- `serialization-cache` - 序列化缓存

### 指标和可观测性
- `metrics` - 基础指标
- `full-metrics` - 完整指标（OpenTelemetry）
- `enhanced-stats` - 增强统计

### 高级特性
- `bloom-filter` - 布隆过滤器
- `rate-limiting` - 速率限制
- `wal-recovery` - 预写日志恢复
- `batch-write` - 批量写入
- `smart-strategy` - 智能策略

### 配置
- `confers` - 配置管理
- `config-dynamic` - 动态配置
- `cli` - CLI 工具

### 其他
- `macros` - 缓存宏
- `database` - 数据库集成
- `codegen` - 代码生成
- `redis-native` - Redis 原生支持
- `ttl-control` - TTL 控制
- `http-cache` - HTTP 缓存

## 特性依赖关系图

```
minimal
├── l1-moka
│   ├── dep:moka
│   └── dep:dashmap
├── tokio/time
├── dep:tracing
├── metrics
│   └── opentelemetry
│       ├── dep:opentelemetry
│       ├── dep:opentelemetry_sdk
│       └── dep:tracing-subscriber
├── serialization
│   ├── dep:serde
│   └── dep:serde_json
└── chrono

core
├── minimal
├── l2-redis
│   └── dep:redis
└── futures

full
├── core
├── macros (dep:oxcache_macros)
├── bloom-filter (dep:bloomfilter, dep:murmur3)
├── rate-limiting
├── wal-recovery
├── database (dep:sea-orm, dep:sqlx, dep:regex)
├── cli (dep:clap, dep:dashmap, dep:tracing, metrics)
├── full-metrics (metrics, dep:tracing-opentelemetry, dep:opentelemetry-otlp)
├── batch-write (dep:tokio-util)
├── confers (dep:confers, dep:toml)
├── codegen (dep:anyhow)
├── compression (dep:flate2)
├── smart-strategy (compression, metrics, l1-moka)
├── redis-native (l2-redis)
├── enhanced-stats (metrics)
├── ttl-control (l2-redis)
├── tiered-cache (l1-moka, l2-redis)
├── extra-serialization (dep:rmp-serde, dep:ciborium)
├── config-dynamic (confers)
├── http-cache (serialization, tiered-cache, dep:md5, dep:http, dep:tower, dep:axum, dep:hyper, dep:http-body-util)
└── serialization-cache (serialization, dep:dashmap)
```

## 关键特性组合

### 1. L1 Only
```
features = ["minimal"]
```

### 2. L2 Only
```
features = ["l2-redis", "serialization"]
```

### 3. Tiered Cache
```
features = ["core"]
```

### 4. Tiered Cache with Macros
```
features = ["core", "macros"]
```

### 5. Full Features
```
features = ["full"]
```

### 6. Minimal with Macros
```
features = ["minimal", "macros"]
```

### 7. Core with Metrics
```
features = ["core", "full-metrics"]
```

### 8. Core with Batch Write
```
features = ["core", "batch-write"]
```

### 9. Core with Bloom Filter
```
features = ["core", "bloom-filter"]
```

### 10. Core with Database
```
features = ["core", "database"]
```

## 已知问题

1. **skip_broken 特性**
   - 文件：`tests/database_partitioning_tests.rs`
   - 原因：数据库分区测试需要外部数据库连接
   - 解决方案：移除 skip_broken，改用环境变量或配置文件控制

2. **特性组合测试覆盖不足**
   - CI 只测试 `--all-features`
   - 缺少对其他特性组合的测试

3. **特性依赖验证不足**
   - 缺少编译时特性依赖验证
   - 可能导致用户启用不兼容的特性组合

## 推荐改进

1. 移除 `skip_broken` 特性标志
2. 添加特性依赖验证
3. 创建特性组合测试矩阵
4. 改进 CI 配置，测试多个特性组合
5. 创建特性组合验证工具