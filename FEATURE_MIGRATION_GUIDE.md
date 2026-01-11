# Oxcache 特性系统重构指南

## 概述

本文档描述了如何将 oxcache 重构为具有激进的最小化特性系统的新架构。

## Cargo.toml 变更

### 新特性结构

```toml
[features]

# 默认特性：最大兼容性
default = ["full"]

# --------------------------------------------------------------------
# 分层特性 (Tiered Features)
# --------------------------------------------------------------------

# Minimal: 仅 L1 内存缓存 - 无 Redis，无外部服务
minimal = ["l1-moka", "dashmap"]

# Core: L1 + L2 Redis + metrics - 生产就绪的基础设置
core = [
    "minimal",
    "l2-redis", 
    "metrics",
]

# Full: 启用所有功能
full = [
    "core",
    "bloom-filter",
    "rate-limiting",
    "batch-write",
    "wal-recovery",
    "serialization",
    "compression",
    "database",
    "cli",
    "opentelemetry",
]

# --------------------------------------------------------------------
# 独立可选特性 (Individual Optional Features)
# --------------------------------------------------------------------

# L1 缓存层
l1-moka = ["moka", "dashmap"]

# L2 缓存层
l2-redis = ["redis"]

# 可观测性
metrics = ["tracing"]
opentelemetry = [
    "opentelemetry",
    "opentelemetry_sdk", 
    "tracing-opentelemetry",
    "opentelemetry-otlp",
]

# 高级缓存特性
bloom-filter = ["bloomfilter", "murmur3"]
rate-limiting = ["governor"]
batch-write = ["tokio-util", "futures"]

# 持久化与恢复
wal-recovery = ["crc32fast"]

# 数据格式支持
serialization = ["serde_json", "bincode"]
compression = ["flate2"]

# 数据库集成
database = ["sea-orm", "sqlx", "chrono"]

# CLI 工具
cli = ["clap", "confers", "toml"]

# 开发与测试
memory-profiling = ["jemalloc-ctl"]
test-helpers = []
```

## 使用示例

### 1. 最小化安装 (仅 L1 内存缓存)

```toml
[dependencies]
oxcache = { version = "0.1", default-features = false, features = ["minimal"] }
```

### 2. 核心安装 (L1 + L2 Redis + Metrics)

```toml
[dependencies]
oxcache = { version = "0.1", default-features = false, features = ["core"] }
```

### 3. 完整安装 (所有功能)

```toml
[dependencies]
oxcache = { version = "0.1", features = ["full"] }
```

### 4. 自定义组合

```toml
[dependencies]
oxcache = { 
    version = "0.1", 
    default-features = false, 
    features = ["l1-moka", "l2-redis", "metrics", "wal-recovery"] 
}
```

## 依赖管理

### 核心依赖 (必需)

```toml
[dependencies]
tokio = { version = "1.42", default-features = false, features = ["sync", "rt", "macros"] }
thiserror = "1.0"
async-trait = "0.1"
uuid = { version = "1.0", default-features = false, features = ["v4"] }
lazy_static = "1.4"
ahash = { version = "0.8.12", default-features = false, features = ["std"] }
secrecy = { version = "0.10.3", default-features = false, features = ["serde"] }
```

### L1 依赖 (内存缓存)

```toml
[dependencies]
moka = { version = "0.12", features = ["future"], optional = true }
dashmap = { version = "6.0", optional = true }
```

### L2 依赖 (Redis 分布式缓存)

```toml
[dependencies]
redis = { version = "0.27", optional = true }
```

### 可选依赖

```toml
[dependencies]
# 序列化
serde = { version = "1.0", default-features = false, features = ["derive", "alloc"] }
serde_json = { version = "1.0", optional = true }
bincode = { version = "2.0", optional = true }

# 压缩
flate2 = { version = "1.0", optional = true }

# 可观测性
tracing = { version = "0.1", optional = true }
opentelemetry = { version = "0.22", optional = true }

# 数据库
sea-orm = { version = "1.0.14", default-features = false, optional = true }
sqlx = { version = "0.8", default-features = false, optional = true }

# CLI
clap = { version = "4.4", default-features = false, features = ["derive"], optional = true }
confers = { version = "0.1.1", optional = true }

# 高级特性
bloomfilter = { version = "2.0", optional = true }
governor = { version = "0.6", default-features = false, features = ["std", "alloc"], optional = true }
crc32fast = { version = "1.3", optional = true }
```

## 向后兼容性

### 废弃别名

```rust
#[deprecated(since = "0.1.2", note = "Use 'l1-moka' feature instead")]
#[cfg(any(feature = "backend", feature = "l1-moka", feature = "minimal", feature = "core", feature = "full"))]
pub use backend;

#[deprecated(since = "0.1.2", note = "Use 'bloom-filter' feature instead")]
#[cfg(any(feature = "bloom_filter", feature = "bloom-filter", feature = "full"))]
pub use bloom_filter;
```

## 源代码适配

### 必需的条件编译

在 `src/error.rs` 中：

```rust
#[cfg(feature = "database")]
impl From<sea_orm::DbErr> for CacheError {
    fn from(e: sea_orm::DbErr) -> Self {
        CacheError::DatabaseError(e.to_string())
    }
}
```

在 `src/lib.rs` 中：

```rust
#[cfg(any(feature = "l1-moka", feature = "minimal", feature = "core", feature = "full"))]
pub mod backend;

#[cfg(any(feature = "serialization", feature = "full"))]
pub mod serialization;

#[cfg(any(feature = "bloom-filter", feature = "full"))]
pub mod bloom_filter;

#[cfg(any(feature = "metrics", feature = "core", feature = "full"))]
pub mod metrics;

#[cfg(any(feature = "rate-limiting", feature = "full"))]
pub mod rate_limiting;

#[cfg(any(feature = "wal-recovery", feature = "full"))]
pub mod recovery;

#[cfg(any(feature = "l2-redis", feature = "full"))]
pub mod sync;

#[cfg(any(feature = "database", feature = "full"))]
pub mod database;

#[cfg(any(feature = "cli", feature = "full"))]
pub mod cli;

#[cfg(any(feature = "opentelemetry", feature = "full"))]
pub mod telemetry;
```

## 特性可用性检查

创建 `src/features.rs` 提供编译时特性检查：

```rust
pub struct FeatureSet {
    pub l1_available: bool,
    pub l2_available: bool,
    pub metrics_available: bool,
    pub bloom_available: bool,
    pub rate_limiting_available: bool,
    pub batch_write_available: bool,
    pub wal_recovery_available: bool,
    pub serialization_available: bool,
    pub compression_available: bool,
    pub database_available: bool,
    pub cli_available: bool,
    pub opentelemetry_available: bool,
}

impl FeatureSet {
    pub fn current() -> Self {
        Self {
            l1_available: cfg!(feature = "l1-moka"),
            l2_available: cfg!(feature = "l2-redis"),
            // ... 其他特性
        }
    }

    pub fn tier_name(&self) -> &'static str {
        if self.opentelemetry_available && self.database_available && self.cli_available {
            "full"
        } else if self.l2_available && self.metrics_available {
            "core"
        } else if self.l1_available {
            "minimal"
        } else {
            "core"
        }
    }
}
```

## 迁移步骤

1. **更新 Cargo.toml**: 将依赖移动到正确的分类
2. **添加特性定义**: 创建分层和独立特性
3. **更新 lib.rs**: 为所有可选模块添加条件编译
4. **更新错误处理**: 为数据库错误添加条件编译
5. **创建 features.rs**: 提供特性可用性检查
6. **测试编译**: 验证每个特性组合可以编译
7. **更新文档**: 更新 README 和用户指南

## 注意事项

- 确保所有可选依赖都标记为 `optional = true`
- 使用 `cfg!` 和 `#[cfg(...)]` 进行条件编译
- 保持向后兼容性，提供废弃别名
- 测试所有特性组合的编译
- 更新依赖版本以兼容新特性系统
