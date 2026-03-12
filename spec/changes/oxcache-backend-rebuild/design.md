# Design: OxCache 后端重构 - 分数排序系统

## Architecture Overview

### 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                      OxCacheBuilder                          │
│  .backend(MokaBackend).backend(RedisBackend).build()       │
└─────────────────────────┬───────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    BackendSorter                             │
│  (内部模块 - 自动修正顺序)                                   │
│  输入: [Redis(50), Moka(100), SQLite(70)]                 │
│  输出: [Moka(100), SQLite(70), Redis(50)]                │
└─────────────────────────┬───────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                      ChainCache                             │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐                   │
│  │  L1     │→ │  L2     │→ │  L3     │→ ...            │
│  │ Moka    │  │ SQLite  │  │  Redis  │                  │
│  │ (100)   │  │  (70)   │  │  (50)   │                  │
│  └─────────┘  └─────────┘  └─────────┘                   │
└─────────────────────────────────────────────────────────────┘
```

### 模块关系

```
┌─────────────────────────────────────────────────────────────┐
│                         lib.rs                              │
│  exports: Cache, OxCacheBuilder, BackendConfig            │
└─────────────────────────┬───────────────────────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        ▼                 ▼                 ▼
┌───────────────┐  ┌───────────────┐  ┌───────────────┐
│   backend/    │  │   builder/    │  │    chain.rs   │
│               │  │               │  │               │
│ - score.rs    │  │ - mod.rs      │  │ ChainCache   │
│ - moka.rs     │  │ - config.rs   │  │ ChainLink    │
│ - redis.rs    │  │ - sorter.rs   │  │               │
│ - sqlite.rs   │  │               │  │               │
│ - lmdb.rs     │  │               │  │               │
└───────────────┘  └───────────────┘  └───────────────┘
```

## Component Design

### 1. 分数管理系统 (score.rs)

```rust
/// 后端分数 trait - 每个后端必须实现
pub trait BackendScore: Send + Sync + 'static {
    fn score(&self) -> u8;
    fn is_persistent(&self) -> bool;
}

/// 内置后端分数常量
pub struct Scores;
impl Scores {
    pub const MOKA: u8 = 100;
    pub const DASHMAP: u8 = 90;
    pub const LMDB: u8 = 80;
    pub const SQLITE: u8 = 70;
    pub const REDIS: u8 = 50;
    pub const MEMCACHED: u8 = 40;
}
```

### 2. 后端配置 (config.rs)

```rust
/// 后端配置 - 用户不输入分数
pub struct BackendConfig<B> {
    pub backend: B,
    pub is_persistent: Option<bool>, // 可选覆盖
}

impl<B: BackendScore> BackendConfig<B> {
    pub fn new(backend: B) -> Self;
    pub fn persistent(mut self) -> Self; // 标记持久化
}
```

### 3. 自动排序器 (sorter.rs)

```rust
/// 后端排序器 - 内部模块
pub struct BackendSorter;

impl BackendSorter {
    /// 根据配置顺序 + 内置分数自动排序
    pub fn sort(
        backends: Vec<BackendConfig<dyn CacheBackend>>
    ) -> Vec<ChainLink>;

    /// 修正不正确的顺序
    fn correct(links: &mut Vec<ChainLink>);
}
```

### 4. 链式缓存 (chain.rs)

```rust
/// 链式缓存项
pub struct ChainLink {
    pub backend: Arc<dyn CacheBackend>,
    pub score: u8,
    pub is_persistent: bool,
}

/// 链式缓存
pub struct ChainCache {
    links: Vec<ChainLink>, // 已按分数排序
}

impl ChainCache {
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    pub async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()>;
    pub async fn delete(&self, key: &str) -> Result<()>;
    // ... 其他 CacheBackend 方法
}
```

### 5. Builder (oxcache_builder.rs)

```rust
pub struct OxCacheBuilder {
    backends: Vec<Box<dyn CacheBackend>>,
    default_ttl: Duration,
    max_capacity: Option<u64>,
}

impl OxCacheBuilder {
    /// 按用户配置顺序添加后端
    pub fn backend<B: BackendScore + CacheBackend + 'static>(
        mut self,
        backend: B
    ) -> Self;

    pub fn ttl(mut self, ttl: Duration) -> Self;
    pub fn max_capacity(mut self, capacity: u64) -> Self;

    pub async fn build(self) -> Result<Cache>;
}
```

## Data Model

### ChainLink 结构

```rust
pub struct ChainLink {
    /// 后端实例
    pub backend: Arc<dyn CacheBackend>,
    /// 内置分数
    pub score: u8,
    /// 是否持久化
    pub is_persistent: bool,
}
```

### 排序规则

1. **分数优先**: 按分数降序排列
2. **持久化修正**: 持久化后端不放最前（除非全是持久化）
3. **用户顺序保留**: 同分数时保留用户配置顺序

## API Design

### 新 API

```rust
use oxcache::{Cache, builder::*, backend::*};

// 纯内存缓存
let cache = OxCacheBuilder::new()
    .backend(MokaBackend::new())
    .ttl(Duration::from_secs(3600))
    .build()
    .await?;

// 多后端链式缓存
let cache = OxCacheBuilder::new()
    .backend(RedisBackend::connect(url).await?)
    .backend(MokaBackend::new())
    .backend(SQLiteBackend::new("cache.db").await?)
    .build()
    .await?;
```

### 旧 API (标记 deprecated)

```rust
#[deprecated(use OxCacheBuilder instead)]
pub struct TieredBackend { ... }
```

## Error Handling

### 错误类型

```rust
#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error("Backend error: {0}")]
    Backend(String),

    #[error("No backends configured")]
    NoBackends,

    #[error("Invalid backend configuration: {0}")]
    InvalidConfig(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}
```

### 降级策略

- 某个后端失败时，记录日志并继续尝试下一个后端
- 所有后端都失败时返回错误

## Migration Plan

### 阶段 1: 新增接口

1. 创建 `score.rs` 分数系统
2. 实现 `BackendScore` trait 到现有后端
3. 创建 `chain.rs` 链式缓存
4. 创建 `OxCacheBuilder`

### 阶段 2: 新增后端

5. 实现 `SQLiteBackend`
6. 实现 `LMDBBackend`

### 阶段 3: 替换旧接口

7. 修改 `lib.rs` 导出新 API
8. 标记旧 API 为 deprecated
9. 更新文档和示例

### 阶段 4: 清理

10. 移除 TieredBackend 代码
11. 清理废弃接口

## Testing Strategy

### 单元测试

- BackendScore 分数正确性测试
- BackendSorter 排序逻辑测试
- ChainCache 链式调用测试

### 集成测试

- 多后端链式读写测试
- 故障降级测试
- 性能基准测试

### 回归测试

- 所有现有测试必须通过
- API 兼容性测试
