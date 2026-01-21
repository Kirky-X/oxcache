# Oxcache 现代化 API 设计方案

> 版本: 0.2.0
> 日期: 2026-01-21

## 1. 设计目标

- **独立实例** - 无全局状态，每个 `Cache` 都是独立的
- **统一接口** - 单一 `Cache<K, V>` 类型，无 `SimpleCache` 区分
- **简洁 API** - 减少样板代码，智能默认值
- **类型安全** - 泛型设计，默认实现常见类型
- **可扩展** - 后端策略可插拔

## 2. 核心 API

### 2.1 统一 Cache 结构

```rust
pub struct Cache<K, V> {
    backend: Arc<dyn CacheBackend>,
    config: CacheConfig,
    _phantom: PhantomData<(K, V)>,
}
```

### 2.2 使用示例

```rust
// 简单场景（String 键）
let cache = Cache::new().await;
cache.set("user:1", &user).await;
let user: Option<User> = cache.get("user:1").await?;

// 泛型键场景
let cache: Cache<u64, User> = Cache::new().await;
cache.set(&1, &user).await;
let user = cache.get(&1).await?;

// Builder 配置
let cache = Cache::builder()
    .redis("redis://localhost:6379")
    .ttl(Duration::from_secs(3600))
    .capacity(10_000)
    .build()
    .await?;

// 双层缓存
let cache = Cache::tiered()
    .memory()
    .redis("redis://localhost:6379")
    .auto_promote(true)
    .build()
    .await?;
```

## 3. Trait 设计

### 3.1 CacheKey（键类型约束）

```rust
pub trait CacheKey: Send + Sync + 'static {
    fn to_key_string(&self) -> String;
}

// 默认实现：常见类型无需手动实现
impl CacheKey for String { fn to_key_string(&self) -> String { self.clone() } }
impl CacheKey for &str { fn to_key_string(&self) -> String { (*self).to_string() } }
impl CacheKey for u64 { fn to_key_string(&self) -> String { self.to_string() } }
impl CacheKey for u128 { fn to_key_string(&self) -> String { self.to_string() } }
impl CacheKey for i64 { fn to_key_string(&self) -> String { self.to_string() } }
impl CacheKey for i32 { fn to_key_string(&self) -> String { self.to_string() } }
impl CacheKey for uuid::Uuid { fn to_key_string(&self) -> String { self.to_string() } }

// 用户自定义类型（需要手动实现）
#[derive(Hash, Eq, PartialEq)]
struct UserKey { tenant_id: u64, user_id: u64 }

impl CacheKey for UserKey {
    fn to_key_string(&self) -> String {
        format!("{}:{}", self.tenant_id, self.user_id)
    }
}
```

### 3.2 Cacheable（值类型约束）

```rust
pub trait Cacheable: Serialize + DeserializeOwned + Send + Sync + 'static {}
```

## 4. 后端策略模式

```rust
#[async_trait]
trait CacheBackend: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<bool>;
    async fn exists(&self, key: &str) -> Result<bool>;
    async fn clear(&self) -> Result<()>;
    async fn close(&self) -> Result<()>;
}

// 实现
struct MemoryBackend { ... }
struct RedisBackend { ... }
struct TieredBackend { ... }
```

## 5. 层次化错误处理

```rust
#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error("operation failed: {message}")]
    Operation {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("connection failed: {message}")]
    Connection {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("serialization failed: {message}")]
    Serialization {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("configuration error: {message}")]
    Config { message: String },

    #[error("key not found")]
    NotFound,

    #[error("operation not supported: {operation}")]
    NotSupported { operation: String },

    #[error("timeout")]
    Timeout,

    #[error("backend degraded: {message}")]
    Degraded { message: String },
}

// 使用
match cache.get(&key).await {
    Ok(Some(value)) => Ok(value),
    Ok(None) => Err(CacheError::NotFound),
    Err(e) => {
        if let Some(source) = e.source() {
            eprintln!("底层错误: {}", source);
        }
        Err(e)
    }
}
```

## 6. Builder 模式

```rust
pub struct CacheBuilder<K, V> {
    backend: BackendBuilder,
    config: CacheConfig,
    _phantom: PhantomData<(K, V)>,
}

impl<K, V> CacheBuilder<K, V> {
    pub fn new() -> Self { Self::default() }
    pub fn memory() -> Self { Self::new().backend(BackendBuilder::memory()) }
    pub fn redis(conn_str: &str) -> Self { Self::new().backend(BackendBuilder::redis(conn_str)) }
    pub fn tiered() -> TieredCacheBuilder<K, V> { TieredCacheBuilder::new(self) }

    pub fn ttl(mut self, ttl: Duration) -> Self { self.config.ttl = Some(ttl); self }
    pub fn capacity(mut self, capacity: u64) -> Self { self.config.capacity = Some(capacity); self }
    pub fn batch_writes(mut self, enable: bool) -> Self { self.config.batch_writes = enable; self }
    pub fn auto_promote(mut self, enable: bool) -> Self { self.config.auto_promote = enable; self }

    pub async fn build(self) -> Result<Cache<K, V>> {
        let backend = self.backend.build().await?;
        Ok(Cache::from_backend(backend, self.config))
    }
}
```

## 7. 设计模式应用

| 模式 | 组件 | 职责 |
|------|------|------|
| **Facade** | `Cache<K, V>` | 统一接口，隐藏后端复杂性 |
| **Builder** | `CacheBuilder` | 流畅配置，支持链式调用 |
| **Strategy** | `CacheBackend` trait | 可插拔后端实现 |
| **Factory** | `BackendBuilder` | 创建不同类型后端 |
| **Decorator** | `MetricsBackend` | 可选添加监控功能 |
| **Chain of Resp.** | `TieredBackend` | L1 → L2 顺序查找 |

## 8. 迁移计划

### 阶段 1: 基础架构
- [ ] 定义 `CacheKey`, `Cacheable` traits
- [ ] 实现 `CacheBackend` trait
- [ ] 实现 `MemoryBackend`

### 阶段 2: Redis 支持
- [ ] 实现 `RedisBackend`
- [ ] 支持单机/哨兵/集群模式

### 阶段 3: 双层缓存
- [ ] 实现 `TieredBackend`
- [ ] 实现自动提升和降级

### 阶段 4: 高级特性
- [ ] 批量操作
- [ ] 布隆过滤器
- [ ] WAL 恢复

### 阶段 5: 迁移
- [ ] 标记旧 API 为 `#[deprecated]`
- [ ] 提供迁移指南

## 9. 代码结构

```
src/
├── lib.rs                    # 主入口
├── cache.rs                  # Cache 结构体和主要 API
├── error.rs                  # CacheError 定义
├── traits/
│   ├── mod.rs
│   ├── cache_key.rs          # CacheKey trait + blanket impl
│   └── cacheable.rs          # Cacheable trait
├── builder/
│   ├── mod.rs                # CacheBuilder
│   ├── backend_builder.rs    # BackendBuilder（工厂模式）
│   └── tiered_builder.rs     # TieredCacheBuilder
├── backend/
│   ├── mod.rs                # CacheBackend trait
│   ├── memory.rs             # MemoryBackend 实现
│   ├── redis.rs              # RedisBackend 实现
│   ├── tiered.rs             # TieredBackend 实现
│   └── metrics.rs            # 装饰器：添加监控
└── utils/
    └── mod.rs
```

## 10. 向后兼容性

### 旧 API 标记

```rust
#[deprecated(since = "0.2.0", note = "请使用 Cache 或 CacheBuilder 替代")]
pub async fn init(config: OxcacheConfig) -> Result<()> { ... }

#[deprecated(since = "0.2.0", note = "请使用 Cache::builder() 替代")]
pub fn get_client(service: &str) -> Result<Arc<dyn CacheOps>> { ... }
```

### 迁移对照表

| 旧 API | 新 API |
|--------|--------|
| `init(config)` | `Cache::builder()...build()` |
| `get_client("name")` | `Cache::new()` 或 `Cache::redis()` |
| `client.get::<T>(k)` | `cache.get(&k)` |
| `client.set(k, v, ttl)` | `cache.set(&k, &v)` |
| `client.delete(k)` | `cache.delete(&k)` |
| `client.get_or_fetch(k, ttl, f)` | `cache.get_or(&k, f)` |

## 11. 移除全局状态

新设计完全移除全局 `MANAGER`，所有状态封装在 `Cache` 实例中：

```rust
// 旧设计（有全局状态）
lazy_static! {
    pub static ref MANAGER: Arc<DashMap<String, Arc<dyn CacheOps>>> =
        Arc::new(DashMap::new());
}

// 新设计（无全局状态）
pub struct Cache<K, V> {
    backend: Arc<dyn CacheBackend>,
    // 无全局状态
}
```

## 12. 实施优先级

| 优先级 | 功能 | 描述 |
|--------|------|------|
| P0 | CacheBuilder | 配置构建器 |
| P0 | CacheKey trait | 键类型约束 + 默认实现 |
| P0 | MemoryBackend | 内存后端 |
| P1 | Cache<K, V> | 统一缓存接口 |
| P1 | RedisBackend | Redis 后端 |
| P1 | TieredBackend | 双层缓存后端 |
| P2 | 批量操作 | set_many, get_many |
| P2 | 错误层次化 | 带 source 的 CacheError |
| P3 | 装饰器 | MetricsBackend |
