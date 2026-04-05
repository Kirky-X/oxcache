# Oxcache 架构修复实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 按照 report.md 的 7 个 FIX 项修复 oxcache 架构，使其符合积木架构规范。

**Architecture:** 分三阶段执行：基础层修复（不破坏 API）→ 接口层重构（breaking change）→ 行为层修复（影响宏和应用集成）。

**Tech Stack:** Rust, thiserror, serde, async-trait, moka, redis (optional)

**参考文档:** `/home/dev/projects/oxcache/temp/report.md`

---

## 优先级总览

| 任务       | FIX 编号 | 优先级 | 描述                                           |
| ---------- | -------- | ------ | ---------------------------------------------- |
| Task 1-2   | FIX-02   | P0     | 移除 `as_any()` 类型擦除逃生口                 |
| Task 3-4   | FIX-04   | P1     | 添加 `new_in_memory()` 工厂函数                |
| Task 5-6   | FIX-03   | P1     | 错误类型拆分（CacheConfigError vs CacheError） |
| Task 7-8   | FIX-01   | P0     | 生命周期接口签名对齐（health_check/shutdown）  |
| Task 9-10  | FIX-05   | P2     | 接口臃肿修复（ISP 违反）                       |
| Task 11-12 | FIX-06   | P2     | 全局注册表反模式修复                           |
| Task 13-14 | FIX-07   | P3     | 配置结构体化与 Builder 职责分离                |

---

## Phase 1: 基础层修复（不破坏外部 API）

### Task 1: 移除 `as_any()` 和 `is()` 方法

**Files:**

- Modify: `src/backend/interface.rs`
- Test: `tests/backend_interface_test.rs`

**目标:** 移除 `as_any()` 和 `is()` 方法，这两个方法破坏了接口抽象。

- [ ] **Step 1: 创建测试文件，验证移除后编译正常**

```rust
// tests/backend_interface_test.rs
// 此测试验证 trait 签名变化后编译正常

use oxcache::backend::CacheBackend;

#[cfg(feature = "moka")]
#[tokio::test]
async fn test_backend_trait_compiles() {
    // 验证 trait 可以被使用
    fn _assert_backend<B: CacheBackend>() {}
    fn _assert_moka<B: CacheBackend>() where B: oxcache::backend::MokaMemoryBackend {}
}
```

- [ ] **Step 2: 运行现有测试确认当前状态**

Run: `cargo test --features moka --lib -- backend 2>&1 | head -50`
Expected: 测试可能通过或有现有错误

- [ ] **Step 3: 修改 interface.rs，移除 as_any 和 is 方法**

```rust
// src/backend/interface.rs — 移除以下方法
// 删除第 169 行: fn as_any(&self) -> &dyn Any;
// 删除第 189-194 行: fn is<T: Any>(&self) -> bool

// 修改后的 trait 应该不再包含：
// - as_any(&self) -> &dyn Any
// - is<T: Any>(&self) -> bool
// 同时移除 use std::any::{Any, TypeId};
```

- [ ] **Step 4: 检查所有使用 as_any 的地方**

Run: `grep -r "as_any" src/ --include="*.rs"`
Expected: 列出所有调用点，需要逐一修复

- [ ] **Step 5: 提交更改**

```bash
git add src/backend/interface.rs
git commit -m "refactor(backend)!: remove as_any() and is() methods from CacheBackend trait

BREAKING CHANGE: as_any() and is() methods removed from CacheBackend trait.
These methods violated the interface abstraction principle.
Use builder pattern for backend-specific configuration instead."
```

---

### Task 2: 更新所有 as_any 调用点

**Files:**

- Search and update all files using `as_any` or `is<T>`

- [ ] **Step 1: 搜索所有调用点**

Run: `grep -rn "as_any\|\.is::<" src/ --include="*.rs"`

- [ ] **Step 2: 逐一修复调用点**

如果找到调用点，需要重构：

```rust
// ❌ 旧方式（运行时向下转型）
if let Some(moka) = backend.as_any().downcast_ref::<MokaBackend>() {
    moka.set_eviction_listener(listener);
}

// ✅ 新方式（构建时配置）
let backend = MokaMemoryBackend::builder()
    .eviction_listener(listener)
    .build();
```

- [ ] **Step 3: 运行测试验证**

Run: `cargo test --all-features`
Expected: 所有测试通过

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "refactor: remove all as_any() usages, use builder pattern instead"
```

---

### Task 3: 创建 InMemoryBackend 和 new_in_memory() 工厂函数

**Files:**

- Create: `src/backend/client/memory/mod.rs`
- Create: `src/backend/client/memory/backend.rs`
- Modify: `src/lib.rs`

**目标:** 提供零配置、零外部依赖的内存缓存实例。

- [ ] **Step 1: 创建内存后端模块**

```rust
// src/backend/client/memory/mod.rs
//! In-memory backend implementation for zero-dependency caching

mod backend;
pub use backend::InMemoryBackend;
```

- [ ] **Step 2: 实现 InMemoryBackend**

```rust
// src/backend/client/memory/backend.rs
//! Simple in-memory cache backend using RwLock<HashMap>

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use async_trait::async_trait;

use crate::backend::interface::CacheBackend;
use crate::error::Result;

/// In-memory cache backend using RwLock<HashMap>
///
/// This is a simple, zero-dependency implementation suitable for:
/// - Unit tests
/// - Development environments
/// - Single-instance applications
pub struct InMemoryBackend {
    storage: RwLock<HashMap<String, (Vec<u8>, Option<Instant>)>>,
    capacity: u64,
}

impl InMemoryBackend {
    /// Create a new in-memory backend with default capacity (10000)
    pub fn new() -> Self {
        Self::with_capacity(10000)
    }

    /// Create a new in-memory backend with specified capacity
    pub fn with_capacity(capacity: u64) -> Self {
        Self {
            storage: RwLock::new(HashMap::new()),
            capacity,
        }
    }

    /// Remove expired entries
    fn evict_expired(&self) {
        let mut storage = self.storage.write().unwrap();
        let now = Instant::now();
        storage.retain(|_, (_, expires_at)| {
            expires_at.map_or(true, |exp| exp > now)
        });
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CacheBackend for InMemoryBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let storage = self.storage.read().unwrap();
        if let Some((value, expires_at)) = storage.get(key) {
            if expires_at.map_or(true, |exp| exp > Instant::now()) {
                return Ok(Some(value.clone()));
            }
        }
        Ok(None)
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        let mut storage = self.storage.write().unwrap();

        // Evict if at capacity
        if storage.len() as u64 >= self.capacity {
            self.evict_expired();
            if storage.len() as u64 >= self.capacity {
                // Simple eviction: remove oldest entry
                if let Some(first_key) = storage.keys().next().cloned() {
                    storage.remove(&first_key);
                }
            }
        }

        let expires_at = ttl.map(|d| Instant::now() + d);
        storage.insert(key.to_string(), (value, expires_at));
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut storage = self.storage.write().unwrap();
        storage.remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let storage = self.storage.read().unwrap();
        if let Some((_, expires_at)) = storage.get(key) {
            Ok(expires_at.map_or(true, |exp| exp > Instant::now()))
        } else {
            Ok(false)
        }
    }

    async fn clear(&self) -> Result<()> {
        let mut storage = self.storage.write().unwrap();
        storage.clear();
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        // No external resources to release
        Ok(())
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        let storage = self.storage.read().unwrap();
        if let Some((_, expires_at)) = storage.get(key) {
            if let Some(exp) = expires_at {
                if *exp > Instant::now() {
                    return Ok(Some(exp.duration_since(Instant::now())));
                }
            }
        }
        Ok(None)
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let mut storage = self.storage.write().unwrap();
        if let Some((value, _)) = storage.get(key) {
            let value = value.clone();
            let expires_at = Instant::now() + ttl;
            storage.insert(key.to_string(), (value, Some(expires_at)));
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn health_check(&self) -> Result<bool> {
        // Memory backend is always healthy
        Ok(true)
    }

    async fn stats(&self) -> Result<std::collections::HashMap<String, String>> {
        let storage = self.storage.read().unwrap();
        let mut stats = std::collections::HashMap::new();
        stats.insert("type".to_string(), "in-memory".to_string());
        stats.insert("entries".to_string(), storage.len().to_string());
        stats.insert("capacity".to_string(), self.capacity.to_string());
        Ok(stats)
    }

    async fn len(&self) -> Result<u64> {
        let storage = self.storage.read().unwrap();
        Ok(storage.len() as u64)
    }

    async fn capacity(&self) -> Result<u64> {
        Ok(self.capacity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_operations() {
        let backend = InMemoryBackend::new();

        // Set and get
        backend.set("key", b"value".to_vec(), None).await.unwrap();
        let value = backend.get("key").await.unwrap();
        assert_eq!(value, Some(b"value".to_vec()));

        // Exists
        assert!(backend.exists("key").await.unwrap());

        // Delete
        backend.delete("key").await.unwrap();
        assert!(!backend.exists("key").await.unwrap());
    }

    #[tokio::test]
    async fn test_ttl() {
        let backend = InMemoryBackend::new();

        backend.set("key", b"value".to_vec(), Some(Duration::from_millis(100))).await.unwrap();
        assert!(backend.get("key").await.unwrap().is_some());

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(backend.get("key").await.unwrap().is_none());
    }
}
```

- [ ] **Step 3: 更新 src/backend/client/mod.rs**

```rust
// src/backend/client/mod.rs — 添加 memory 模块
pub mod memory;

// 现有模块...
#[cfg(feature = "moka")]
pub mod moka;

#[cfg(feature = "dashmap")]
pub mod dashmap;

#[cfg(feature = "redis")]
pub mod redis;

// 重新导出
pub use memory::InMemoryBackend;
```

- [ ] **Step 4: 在 lib.rs 添加工厂函数**

````rust
// src/lib.rs — 在文件末尾添加

// ============================================================================
// Factory Functions (积木架构标准)
// ============================================================================

/// Create an in-memory cache with zero configuration
///
/// This is the standard factory function for:
/// - Unit tests (no external dependencies)
/// - Development environments
/// - Feature module `new()` patterns
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::new_in_memory;
/// use oxcache::backend::CacheBackend;
///
/// #[tokio::main]
/// async fn main() {
///     let cache = new_in_memory();
///     cache.set("key", b"value".to_vec(), None).await.unwrap();
/// }
/// ```
pub fn new_in_memory() -> backend::client::InMemoryBackend {
    backend::client::InMemoryBackend::new()
}

/// Create an in-memory cache with specific capacity
pub fn with_capacity(capacity: u64) -> backend::client::InMemoryBackend {
    backend::client::InMemoryBackend::with_capacity(capacity)
}
````

- [ ] **Step 5: 运行测试**

Run: `cargo test --lib memory`
Expected: 测试通过

- [ ] **Step 6: 提交**

```bash
git add src/backend/client/memory/ src/backend/client/mod.rs src/lib.rs
git commit -m "feat: add InMemoryBackend and new_in_memory() factory function

- Add simple RwLock<HashMap> based in-memory backend
- Provide zero-dependency cache for tests and development
- Follow BrickArchitecture foundation module pattern"
```

---

### Task 4: 编写 new_in_memory 集成测试

**Files:**

- Create: `tests/new_in_memory_test.rs`

- [ ] **Step 1: 创建测试文件**

```rust
// tests/new_in_memory_test.rs
//! Integration tests for new_in_memory() factory function

use oxcache::backend::CacheBackend;
use std::time::Duration;

#[tokio::test]
async fn test_new_in_memory_basic() {
    let cache = oxcache::new_in_memory();

    // Basic operations
    cache.set("key", b"value".to_vec(), None).await.unwrap();
    let value = cache.get("key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));
}

#[tokio::test]
async fn test_with_capacity() {
    let cache = oxcache::with_capacity(100);

    // Fill beyond capacity
    for i in 0..150 {
        cache.set(&format!("key{}", i), b"value".to_vec(), None).await.unwrap();
    }

    // Should have evicted some entries
    let len = cache.len().await.unwrap();
    assert!(len <= 100);
}

#[tokio::test]
async fn test_ttl_expiration() {
    let cache = oxcache::new_in_memory();

    cache.set("key", b"value".to_vec(), Some(Duration::from_millis(50))).await.unwrap();
    assert!(cache.get("key").await.unwrap().is_some());

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(cache.get("key").await.unwrap().is_none());
}

#[tokio::test]
async fn test_health_check() {
    let cache = oxcache::new_in_memory();
    assert!(cache.health_check().await.unwrap());
}

#[tokio::test]
async fn test_clear() {
    let cache = oxcache::new_in_memory();

    cache.set("key1", b"v1".to_vec(), None).await.unwrap();
    cache.set("key2", b"v2".to_vec(), None).await.unwrap();

    cache.clear().await.unwrap();

    assert!(!cache.exists("key1").await.unwrap());
    assert!(!cache.exists("key2").await.unwrap());
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --test new_in_memory_test`
Expected: 所有测试通过

- [ ] **Step 3: 提交**

```bash
git add tests/new_in_memory_test.rs
git commit -m "test: add integration tests for new_in_memory() factory"
```

---

### Task 5: 创建 CacheConfigError 错误类型

**Files:**

- Create: `src/config_error.rs`
- Modify: `src/lib.rs`

**目标:** 将配置阶段错误与运行时错误分离。

- [ ] **Step 1: 创建配置错误类型**

```rust
// src/config_error.rs
//! Configuration error types
//!
//! These errors occur during cache initialization, not during runtime operations.
//! For runtime errors, see [`CacheError`].

use thiserror::Error;

/// Configuration error: occurs during module initialization
///
/// This error type is used by factory functions (`new()`, `new_redis()`, etc.)
/// when the provided configuration is invalid.
#[derive(Debug, Error)]
pub enum CacheConfigError {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid value for field '{field}': {reason}")]
    InvalidValue {
        field: String,
        reason: String,
    },

    #[error("unsupported backend type: '{0}'")]
    UnsupportedBackend(String),

    #[error("capacity must be greater than 0")]
    ZeroCapacity,

    #[error("capacity {0} exceeds maximum {1}")]
    CapacityExceeded(u64, u64),

    #[error("TTL must be greater than 0")]
    ZeroTtl,

    #[error("TTL {0} seconds exceeds maximum {1} seconds")]
    TtlExceeded(u64, u64),

    #[error("connection string is empty")]
    EmptyConnectionString,

    #[error("invalid connection string: {0}")]
    InvalidConnectionString(String),

    #[error("connection failed during initialization: {0}")]
    ConnectionFailed(String),

    #[error("pool size must be greater than 0")]
    ZeroPoolSize,
}

/// Result type for configuration operations
pub type ConfigResult<T> = std::result::Result<T, CacheConfigError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = CacheConfigError::MissingField("url".to_string());
        assert_eq!(err.to_string(), "missing required field: url");

        let err = CacheConfigError::InvalidValue {
            field: "capacity".to_string(),
            reason: "must be positive".to_string(),
        };
        assert!(err.to_string().contains("capacity"));
    }
}
```

- [ ] **Step 2: 在 lib.rs 导出**

```rust
// src/lib.rs — 添加到公开 API 部分

// Configuration error (separate from runtime errors)
pub mod config_error;
pub use config_error::{CacheConfigError, ConfigResult};
```

- [ ] **Step 3: 运行测试**

Run: `cargo test --lib config_error`
Expected: 测试通过

- [ ] **Step 4: 提交**

```bash
git add src/config_error.rs src/lib.rs
git commit -m "feat(error): add CacheConfigError for initialization-phase errors

Separate configuration errors from runtime errors per BrickArchitecture spec.
Configuration errors occur during factory function initialization.
Runtime errors occur during cache operations."
```

---

### Task 6: 从 CacheError 移除配置相关变体

**Files:**

- Modify: `src/error.rs`
- Update all affected files

- [ ] **Step 1: 检查 ConfigError 使用情况**

Run: `grep -rn "ConfigError" src/ --include="*.rs"`

- [ ] **Step 2: 修改 error.rs，移除 ConfigError 变体**

```rust
// src/error.rs — 移除 ConfigError 变体
// 删除或注释掉：
// #[error("Configuration error: {0}")]
// ConfigError(String),

// 确保 CacheError 只包含运行时错误：
// - Serialization
// - Operation
// - Connection
// - NotFound
// - Degraded
// - L1Error
// - L2Error
// - NotSupported
// - WalError
// - DatabaseError
// - RedisError
// - IoError
// - BackendError
// - Timeout
// - ShutdownError
// - KeyTooLong
// - ValueTooLarge
// - BufferFull
// - InvalidInput
// - InvalidKey
// - LockError
// - ServiceNotFound
```

- [ ] **Step 3: 更新所有使用 CacheError::ConfigError 的地方**

Run: `cargo check 2>&1 | grep "ConfigError"`
Expected: 找到所有需要更新的位置

将 `CacheError::ConfigError(msg)` 替换为 `CacheConfigError::xxx`

- [ ] **Step 4: 运行测试**

Run: `cargo test --all-features`
Expected: 所有测试通过

- [ ] **Step 5: 提交**

```bash
git add src/error.rs
git commit -m "refactor(error): remove ConfigError from CacheError enum

BREAKING CHANGE: CacheError::ConfigError removed.
Use CacheConfigError for initialization-phase errors."
```

---

## Phase 2: 接口层重构（Breaking Changes）

### Task 7: 修改 health_check 和 close 签名

**Files:**

- Modify: `src/backend/interface.rs`
- Update all implementations

**目标:** 对齐积木架构规范：

- `health_check(&self) -> Result<()>` (不是 `Result<bool>`)
- `shutdown(&self)` (无返回值，替代 `close`)

- [ ] **Step 1: 修改 CacheBackend trait 签名**

```rust
// src/backend/interface.rs — 修改方法签名

// 将:
// async fn health_check(&self) -> Result<bool>;
// async fn close(&self) -> Result<()>;

// 改为:
/// Check if the backend is healthy
///
/// Returns Ok(()) if healthy, Err with diagnostic info if not.
async fn health_check(&self) -> anyhow::Result<()>;

/// Gracefully shutdown the backend
///
/// Waits for in-flight operations to complete, releases resources.
/// Internal timeouts only log warnings, never panic.
async fn shutdown(&self);
```

- [ ] **Step 2: 更新所有实现（MokaMemoryBackend）**

```rust
// src/backend/client/moka/backend.rs

#[async_trait]
impl CacheBackend for MokaMemoryBackend {
    // ... 其他方法 ...

    async fn health_check(&self) -> anyhow::Result<()> {
        // Moka is always healthy if it's accessible
        Ok(())
    }

    async fn shutdown(&self) {
        // Moka handles cleanup automatically
        // No explicit shutdown needed for memory cache
    }

    // 移除 close() 实现
}
```

- [ ] **Step 3: 更新 RedisBackend 实现**

```rust
// src/backend/client/redis/client.rs

#[async_trait]
impl CacheBackend for RedisBackend {
    // ... 其他方法 ...

    async fn health_check(&self) -> anyhow::Result<()> {
        self.client
            .ping()
            .await
            .map_err(|e| anyhow::anyhow!("redis health check failed: {e}"))
    }

    async fn shutdown(&self) {
        if let Err(e) = self.close_connection().await {
            tracing::warn!("redis shutdown error: {e}");
        }
    }
}
```

- [ ] **Step 4: 更新 DashMapMemoryBackend 实现**

```rust
// src/backend/client/dashmap/backend.rs

#[async_trait]
impl CacheBackend for DashMapMemoryBackend {
    // ... 其他方法 ...

    async fn health_check(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn shutdown(&self) {
        // DashMap handles cleanup automatically
    }
}
```

- [ ] **Step 5: 更新 InMemoryBackend 实现**

```rust
// src/backend/client/memory/backend.rs — 更新之前创建的文件

#[async_trait]
impl CacheBackend for InMemoryBackend {
    // ... 其他方法 ...

    async fn health_check(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn shutdown(&self) {
        // No external resources to release
    }
}
```

- [ ] **Step 6: 更新 MockBackend**

```rust
// src/testing/mock.rs

#[async_trait]
impl CacheBackend for MockBackend {
    // ... 其他方法 ...

    async fn health_check(&self) -> anyhow::Result<()> {
        if self.healthy {
            Ok(())
        } else {
            Err(anyhow::anyhow!("mock backend is unhealthy"))
        }
    }

    async fn shutdown(&self) {
        // Mock has no resources
    }
}
```

- [ ] **Step 7: 搜索并更新所有调用点**

Run: `grep -rn "\.health_check()\|\.close()" src/ --include="*.rs"`

将 `match result { Ok(true) => ... }` 改为 `result.is_ok()`
将 `.close()` 改为 `.shutdown()`

- [ ] **Step 8: 运行测试**

Run: `cargo test --all-features`
Expected: 所有测试通过

- [ ] **Step 9: 提交**

```bash
git add -A
git commit -m "refactor(backend)!: change health_check and close signatures

BREAKING CHANGES:
- health_check() now returns Result<()> instead of Result<bool>
- close() renamed to shutdown() with no return value
- All implementations updated accordingly

This aligns with BrickArchitecture foundation module spec."
```

---

### Task 8: 添加 anyhow 依赖（如果需要）

**Files:**

- Modify: `Cargo.toml`

- [ ] **Step 1: 检查 anyhow 是否已作为依赖**

Run: `grep "anyhow" Cargo.toml`

- [ ] **Step 2: 如果需要，将 anyhow 改为必需依赖**

```toml
# Cargo.toml
# 如果 anyhow 是 optional，改为：
anyhow = "~1.0"  # 移除 optional = true

# 或者保持 optional 但在需要的 feature 中启用
```

- [ ] **Step 3: 提交**

```bash
git add Cargo.toml
git commit -m "chore: make anyhow a required dependency for health_check errors"
```

---

### Task 9: 拆分 CacheBackend trait（ISP）

**Files:**

- Modify: `src/backend/interface.rs`
- Create: `src/backend/traits/mod.rs`
- Create: `src/backend/traits/reader.rs`
- Create: `src/backend/traits/writer.rs`
- Create: `src/backend/traits/connector.rs`

**目标:** 将 19 方法的胖接口拆分为小接口。

- [ ] **Step 1: 创建 traits 目录结构**

```bash
mkdir -p src/backend/traits
```

- [ ] **Step 2: 创建 CacheReader trait**

```rust
// src/backend/traits/reader.rs
//! Read operations for cache backends

use async_trait::async_trait;
use std::time::Duration;

use crate::error::Result;

/// Single-layer cache read operations
#[async_trait]
pub trait CacheReader: Send + Sync {
    /// Get a value from the cache
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Check if a key exists
    async fn exists(&self, key: &str) -> Result<bool>;

    /// Get the TTL for a key
    async fn ttl(&self, key: &str) -> Result<Option<Duration>>;

    /// Get the number of entries
    async fn len(&self) -> Result<u64>;

    /// Check if the cache is empty
    async fn is_empty(&self) -> Result<bool> {
        Ok(self.len().await? == 0)
    }

    /// Get the capacity
    async fn capacity(&self) -> Result<u64>;
}

/// Batch read operations
#[async_trait]
pub trait BatchReader: CacheReader {
    /// Get multiple values in a single operation
    async fn get_many(&self, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(self.get(key).await?);
        }
        Ok(results)
    }
}
```

- [ ] **Step 3: 创建 CacheWriter trait**

```rust
// src/backend/traits/writer.rs
//! Write operations for cache backends

use async_trait::async_trait;
use std::time::Duration;

use crate::error::Result;

/// Single-layer cache write operations
#[async_trait]
pub trait CacheWriter: Send + Sync {
    /// Set a value in the cache
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()>;

    /// Delete a key from the cache
    async fn delete(&self, key: &str) -> Result<()>;

    /// Set the TTL for an existing key
    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool>;

    /// Clear all entries
    async fn clear(&self) -> Result<()>;
}

/// Batch write operations
#[async_trait]
pub trait BatchWriter: CacheWriter {
    /// Set multiple key-value pairs
    async fn set_many(&self, items: &[(String, Vec<u8>, Option<Duration>)]) -> Result<()> {
        for (key, value, ttl) in items {
            self.set(key, value.clone(), *ttl).await?;
        }
        Ok(())
    }

    /// Delete multiple keys
    async fn delete_many(&self, keys: &[String]) -> Result<()> {
        for key in keys {
            self.delete(key).await?;
        }
        Ok(())
    }
}
```

- [ ] **Step 4: 创建 CacheConnector trait**

```rust
// src/backend/traits/connector.rs
//! Lifecycle operations for cache backends

use async_trait::async_trait;

/// Cache connector with lifecycle methods
///
/// Combines Reader + Writer with health check and shutdown.
#[async_trait]
pub trait CacheConnector: super::CacheReader + super::CacheWriter + Send + Sync {
    /// Health check with diagnostic info
    async fn health_check(&self) -> anyhow::Result<()>;

    /// Graceful shutdown
    async fn shutdown(&self);

    /// Get backend statistics
    async fn stats(&self) -> std::collections::HashMap<String, String>;
}
```

- [ ] **Step 5: 创建 traits/mod.rs**

```rust
// src/backend/traits/mod.rs
//! Trait definitions following Interface Segregation Principle

mod reader;
mod writer;
mod connector;

pub use reader::{CacheReader, BatchReader};
pub use writer::{CacheWriter, BatchWriter};
pub use connector::CacheConnector;
```

- [ ] **Step 6: 更新 src/backend/interface.rs**

```rust
// src/backend/interface.rs — 重写为组合 trait

mod traits;
pub use traits::*;

// 保留 CacheBackend 作为类型别名，便于迁移
#[async_trait]
pub trait CacheBackend: CacheConnector + BatchReader + BatchWriter {}

// Blanket implementation
impl<T: CacheConnector + BatchReader + BatchWriter> CacheBackend for T {}
```

- [ ] **Step 7: 更新 lib.rs 导出**

```rust
// src/lib.rs
pub use backend::{CacheReader, CacheWriter, CacheConnector, BatchReader, BatchWriter};
```

- [ ] **Step 8: 运行测试**

Run: `cargo test --all-features`
Expected: 可能需要更新一些实现

- [ ] **Step 9: 提交**

```bash
git add src/backend/traits/ src/backend/interface.rs src/lib.rs
git commit -m "refactor(backend): split CacheBackend into ISP-compliant traits

- CacheReader: read operations
- CacheWriter: write operations
- CacheConnector: lifecycle + Reader + Writer
- BatchReader/BatchWriter: batch operations
- CacheBackend: alias combining all traits

This follows Interface Segregation Principle from BrickArchitecture spec."
```

---

### Task 10: 添加 CacheStats 强类型结构

**Files:**

- Modify: `src/backend/traits/connector.rs` 或创建 `src/backend/stats.rs`

- [ ] **Step 1: 创建 CacheStats 结构**

```rust
// src/backend/stats.rs
//! Strongly-typed cache statistics

use serde::{Deserialize, Serialize};

/// Cache statistics snapshot
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total number of cache hits
    pub hits: u64,
    /// Total number of cache misses
    pub misses: u64,
    /// Current number of entries
    pub size: u64,
    /// Maximum capacity (None if unlimited)
    pub capacity: Option<u64>,
    /// Number of evictions
    pub evictions: u64,
    /// Number of errors
    pub errors: u64,
}

impl CacheStats {
    /// Calculate hit rate (0.0 to 1.0)
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Create empty stats
    pub fn new() -> Self {
        Self::default()
    }
}
```

- [ ] **Step 2: 更新 CacheConnector 使用 CacheStats**

```rust
// src/backend/traits/connector.rs
use crate::backend::stats::CacheStats;

#[async_trait]
pub trait CacheConnector: super::CacheReader + super::CacheWriter + Send + Sync {
    // ... 其他方法 ...

    /// Get backend statistics as strongly-typed struct
    async fn stats(&self) -> CacheStats;
}
```

- [ ] **Step 3: 更新所有实现的 stats() 方法**

Run: `grep -rn "async fn stats" src/ --include="*.rs"`
更新返回类型从 `HashMap<String, String>` 到 `CacheStats`

- [ ] **Step 4: 提交**

```bash
git add src/backend/stats.rs src/backend/traits/
git commit -m "feat(stats): add strongly-typed CacheStats struct

Replace HashMap<String, String> with CacheStats struct for
compile-time safety and better API ergonomics."
```

---

## Phase 3: 行为层修复

### Task 11: 重构全局注册表（方案 B：显式初始化）

**Files:**

- Modify: `src/internal.rs`
- Create: `src/registry.rs`

**目标:** 禁止懒加载，要求应用层显式初始化。

- [ ] **Step 1: 创建 registry.rs**

```rust
// src/registry.rs
//! Global cache registry for #[cached] macro support
//!
//! IMPORTANT: This registry must be explicitly initialized at application startup.
//! Do NOT use lazy initialization (OnceLock::get_or_init).

use std::sync::Arc;
use once_cell::sync::OnceCell;

use crate::backend::CacheBackend;

/// Global cache registry (singleton)
static CACHE_REGISTRY: OnceCell<Registry> = OnceCell::new();

/// Registry holding cache instances
struct Registry {
    caches: dashmap::DashMap<String, Arc<dyn CacheBackend>>,
}

impl Registry {
    fn new() -> Self {
        Self {
            caches: dashmap::DashMap::new(),
        }
    }
}

/// Initialize the global registry with a default cache
///
/// # Panics
///
/// Panics if called more than once.
pub fn init(default_cache: Arc<dyn CacheBackend>) {
    let registry = Registry::new();
    registry.caches.insert("default".to_string(), default_cache);

    CACHE_REGISTRY
        .set(registry)
        .expect("oxcache registry already initialized - call init() only once");
}

/// Initialize the global registry without a default cache
pub fn init_empty() {
    let registry = Registry::new();

    CACHE_REGISTRY
        .set(registry)
        .expect("oxcache registry already initialized - call init() only once");
}

/// Check if the registry is initialized
pub fn is_initialized() -> bool {
    CACHE_REGISTRY.get().is_some()
}

/// Register a cache instance
///
/// # Panics
///
/// Panics if the registry is not initialized.
pub fn register(name: &str, cache: Arc<dyn CacheBackend>) {
    let registry = CACHE_REGISTRY
        .get()
        .expect("oxcache registry not initialized - call init() first");
    registry.caches.insert(name.to_string(), cache);
}

/// Get a cache instance by name
///
/// Returns None if the registry is not initialized or the cache doesn't exist.
pub fn get(name: &str) -> Option<Arc<dyn CacheBackend>> {
    CACHE_REGISTRY.get()?.caches.get(name).map(|r| r.clone())
}

/// Remove a cache instance
pub fn remove(name: &str) -> Option<Arc<dyn CacheBackend>> {
    CACHE_REGISTRY.get()?.caches.remove(name).map(|(_, v)| v)
}

/// Clear all caches from the registry
pub fn clear() {
    if let Some(registry) = CACHE_REGISTRY.get() {
        registry.caches.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::client::InMemoryBackend;

    #[test]
    #[should_panic(expected = "only once")]
    fn test_double_init_panics() {
        let cache = Arc::new(InMemoryBackend::new());
        init(cache.clone());
        init(cache); // Should panic
    }
}
```

- [ ] **Step 2: 更新 lib.rs 导出**

```rust
// src/lib.rs
pub mod registry;

// 重新导出常用函数
pub use registry::{init, register, get, is_initialized};
```

- [ ] **Step 3: 更新 internal.rs**

```rust
// src/internal.rs — 简化为调用 registry

use crate::backend::CacheBackend;
use std::sync::Arc;

/// Internal cache registration function (called by #[cached] macro)
pub async fn __internal_register_cache(name: &str, cache: Arc<dyn CacheBackend>) {
    crate::registry::register(name, cache);
}

/// Internal cache retrieval function (called by #[cached] macro)
pub fn __internal_get_cache(name: &str) -> Option<Arc<dyn CacheBackend>> {
    crate::registry::get(name)
}

// ... 保留其他功能函数 ...
```

- [ ] **Step 4: 运行测试**

Run: `cargo test --all-features`
Expected: 可能需要修复一些测试

- [ ] **Step 5: 提交**

```bash
git add src/registry.rs src/internal.rs src/lib.rs
git commit -m "refactor: require explicit registry initialization

Replace lazy initialization with explicit init() call.
Application layer must now call oxcache::init() at startup.

This follows BrickArchitecture AppContainer pattern where
all singletons are explicitly held by the application."
```

---

### Task 12: 更新文档和示例

**Files:**

- Modify: `src/lib.rs` 文档
- Create: `examples/explicit_init.rs`

- [ ] **Step 1: 更新 lib.rs 文档**

````rust
// src/lib.rs — 更新模块文档

//! # 初始化（必须）
//!
//! 使用缓存前，必须先初始化全局注册表：
//!
//! ```rust,ignore
//! #[tokio::main]
//! async fn main() {
//!     // 初始化缓存
//!     let cache = Arc::new(oxcache::new_in_memory());
//!     oxcache::init(cache.clone());
//!
//!     // 现在可以使用 #[cached] 宏
//!     run_app().await;
//! }
//! ```
````

- [ ] **Step 2: 创建示例**

```rust
// examples/explicit_init.rs
//! Example: Explicit cache initialization

use std::sync::Arc;
use oxcache::{new_in_memory, init, register, get};

#[tokio::main]
async fn main() {
    // Step 1: Create cache instances
    let default_cache = Arc::new(new_in_memory());
    let user_cache = Arc::new(new_in_memory());

    // Step 2: Initialize registry (MUST be called before any cache operations)
    init(default_cache.clone());

    // Step 3: Register additional caches
    register("users", user_cache.clone());

    // Step 4: Use caches
    let cache = get("default").expect("default cache");
    // ... use cache ...

    println!("Cache initialized successfully!");
}
```

- [ ] **Step 3: 提交**

```bash
git add src/lib.rs examples/explicit_init.rs
git commit -m "docs: update documentation for explicit initialization pattern"
```

---

### Task 13: 创建 CacheConfig 结构体

**Files:**

- Create: `src/cache_config.rs`
- Modify: `src/lib.rs`

**目标:** 将配置值聚合在可序列化的结构体中。

- [ ] **Step 1: 创建 CacheConfig 结构体**

```rust
// src/cache_config.rs
//! Cache configuration structures
//!
//! These structures aggregate configuration values for serialization
//! and validation, separate from the Builder pattern.

use serde::Deserialize;
use std::time::Duration;

use crate::config_error::{CacheConfigError, ConfigResult};

/// Main cache configuration
#[derive(Debug, Clone, Deserialize)]
pub struct CacheConfig {
    /// L1 (memory) configuration - required
    pub l1: L1Config,

    /// L2 (Redis) configuration - optional
    #[serde(default)]
    pub l2: Option<L2Config>,

    /// Default TTL for cache entries
    #[serde(default = "default_ttl")]
    pub default_ttl_secs: u64,

    /// Serialization format
    #[serde(default)]
    pub serialization: SerializationType,
}

fn default_ttl() -> u64 {
    3600
}

/// L1 (memory) layer configuration
#[derive(Debug, Clone, Deserialize)]
pub struct L1Config {
    /// Maximum number of entries
    #[serde(default = "default_capacity")]
    pub capacity: u64,

    /// TTL for L1 entries (seconds)
    pub ttl_secs: Option<u64>,

    /// Backend type for L1
    #[serde(default)]
    pub backend: L1Backend,
}

fn default_capacity() -> u64 {
    10000
}

/// L1 backend type
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L1Backend {
    #[default]
    Moka,
    DashMap,
    InMemory,
}

/// L2 (Redis) layer configuration
#[derive(Debug, Clone, Deserialize)]
pub struct L2Config {
    /// Redis connection URL
    pub url: String,

    /// Connection pool size
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,

    /// TTL for L2 entries (seconds)
    pub ttl_secs: Option<u64>,

    /// Redis mode
    #[serde(default)]
    pub mode: RedisMode,
}

fn default_pool_size() -> u32 {
    10
}

/// Redis deployment mode
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedisMode {
    #[default]
    Standalone,
    Cluster,
    Sentinel,
}

/// Serialization format
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SerializationType {
    #[default]
    Json,
    Bincode,
    MessagePack,
    Cbor,
}

// Validation implementations
impl CacheConfig {
    /// Validate the configuration
    pub fn validate(&self) -> ConfigResult<()> {
        self.l1.validate()?;
        if let Some(ref l2) = self.l2 {
            l2.validate()?;
        }
        Ok(())
    }

    /// Load from TOML file
    #[cfg(feature = "toml")]
    pub fn from_toml(path: &str) -> ConfigResult<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| CacheConfigError::InvalidConnectionString(
                format!("failed to read config file: {e}")
            ))?;
        toml::from_str(&content)
            .map_err(|e| CacheConfigError::InvalidValue {
                field: "config".to_string(),
                reason: e.to_string(),
            })
    }
}

impl L1Config {
    const MAX_CAPACITY: u64 = 1_000_000_000;

    fn validate(&self) -> ConfigResult<()> {
        if self.capacity == 0 {
            return Err(CacheConfigError::ZeroCapacity);
        }
        if self.capacity > Self::MAX_CAPACITY {
            return Err(CacheConfigError::CapacityExceeded(
                self.capacity,
                Self::MAX_CAPACITY,
            ));
        }
        if let Some(ttl) = self.ttl_secs {
            if ttl == 0 {
                return Err(CacheConfigError::ZeroTtl);
            }
        }
        Ok(())
    }

    /// Get TTL as Duration
    pub fn ttl(&self) -> Option<Duration> {
        self.ttl_secs.map(Duration::from_secs)
    }
}

impl L2Config {
    fn validate(&self) -> ConfigResult<()> {
        if self.url.is_empty() {
            return Err(CacheConfigError::EmptyConnectionString);
        }
        if self.pool_size == 0 {
            return Err(CacheConfigError::ZeroPoolSize);
        }
        Ok(())
    }

    /// Get TTL as Duration
    pub fn ttl(&self) -> Option<Duration> {
        self.ttl_secs.map(Duration::from_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_config() {
        let config = CacheConfig {
            l1: L1Config {
                capacity: 1000,
                ttl_secs: Some(3600),
                backend: L1Backend::default(),
            },
            l2: None,
            default_ttl_secs: 3600,
            serialization: SerializationType::default(),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_zero_capacity_error() {
        let config = L1Config {
            capacity: 0,
            ttl_secs: None,
            backend: L1Backend::default(),
        };
        assert!(matches!(config.validate(), Err(CacheConfigError::ZeroCapacity)));
    }
}
```

- [ ] **Step 2: 在 lib.rs 导出**

```rust
// src/lib.rs
pub mod cache_config;
pub use cache_config::{CacheConfig, L1Config, L2Config, SerializationType};
```

- [ ] **Step 3: 运行测试**

Run: `cargo test --lib cache_config`
Expected: 测试通过

- [ ] **Step 4: 提交**

```bash
git add src/cache_config.rs src/lib.rs
git commit -m "feat(config): add CacheConfig struct for serialization

Separate configuration values from Builder pattern.
Config can be deserialized from TOML/JSON for application config."
```

---

### Task 14: 创建基于 Config 的工厂函数

**Files:**

- Modify: `src/lib.rs`

- [ ] **Step 1: 添加 new() 工厂函数**

````rust
// src/lib.rs — 添加基于配置的工厂函数

/// Create a cache from configuration
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::{CacheConfig, L1Config, new};
///
/// let config = CacheConfig {
///     l1: L1Config {
///         capacity: 10000,
///         ttl_secs: Some(3600),
///         backend: L1Backend::InMemory,
///     },
///     l2: None,
///     default_ttl_secs: 3600,
///     serialization: SerializationType::Json,
/// };
///
/// let cache = new(config).await?;
/// ```
#[cfg(feature = "moka")]
pub async fn new(config: CacheConfig) -> ConfigResult<std::sync::Arc<dyn backend::CacheBackend>> {
    config.validate()?;

    match config.l1.backend {
        L1Backend::InMemory => {
            Ok(std::sync::Arc::new(backend::client::InMemoryBackend::with_capacity(config.l1.capacity)))
        }
        L1Backend::Moka => {
            let backend = backend::MokaMemoryBackend::builder()
                .capacity(config.l1.capacity)
                .build();
            Ok(std::sync::Arc::new(backend))
        }
        L1Backend::DashMap => {
            let backend = backend::DashMapMemoryBackend::builder()
                .capacity(config.l1.capacity)
                .build();
            Ok(std::sync::Arc::new(backend))
        }
    }
}

/// Create a Redis cache
#[cfg(feature = "redis")]
pub async fn new_redis(url: &str) -> ConfigResult<std::sync::Arc<dyn backend::CacheBackend>> {
    if url.is_empty() {
        return Err(CacheConfigError::EmptyConnectionString);
    }

    backend::client::redis::RedisBackend::new(url)
        .await
        .map(|b| std::sync::Arc::new(b) as std::sync::Arc<dyn backend::CacheBackend>)
        .map_err(|e| CacheConfigError::ConnectionFailed(e.to_string()))
}

/// Create a tiered cache (L1 + L2)
#[cfg(all(feature = "moka", feature = "redis"))]
pub async fn new_tiered(
    l1_capacity: u64,
    redis_url: &str,
) -> ConfigResult<std::sync::Arc<dyn backend::CacheBackend>> {
    if l1_capacity == 0 {
        return Err(CacheConfigError::ZeroCapacity);
    }
    if redis_url.is_empty() {
        return Err(CacheConfigError::EmptyConnectionString);
    }

    // 创建 L1
    let l1 = backend::MokaMemoryBackend::builder()
        .capacity(l1_capacity)
        .build();

    // 创建 L2
    let l2 = backend::client::redis::RedisBackend::new(redis_url)
        .await
        .map_err(|e| CacheConfigError::ConnectionFailed(e.to_string()))?;

    // 创建分层缓存
    let tiered = backend::custom_tiered::TieredBackend::new(l1, l2);

    Ok(std::sync::Arc::new(tiered) as std::sync::Arc<dyn backend::CacheBackend>)
}
````

- [ ] **Step 2: 运行测试**

Run: `cargo test --all-features`
Expected: 测试通过

- [ ] **Step 3: 提交**

```bash
git add src/lib.rs
git commit -m "feat: add new(), new_redis(), new_tiered() factory functions

Factory functions accept configuration and return initialized cache.
Follows BrickArchitecture foundation module pattern."
```

---

## Phase 4: 最终验证

### Task 15: 运行完整测试套件

- [ ] **Step 1: 运行所有测试**

```bash
cargo test --all-features
```

- [ ] **Step 2: 运行 clippy**

```bash
cargo clippy --all-features -- -D warnings
```

- [ ] **Step 3: 运行格式检查**

```bash
cargo fmt -- --check
```

- [ ] **Step 4: 生成文档**

```bash
cargo doc --no-deps
```

- [ ] **Step 5: 最终提交**

```bash
git add -A
git commit -m "chore: final cleanup and verification"
```

---

### Task 16: 验证报告要求

**检查清单:**

- [ ] FIX-01: `health_check()` 返回 `Result<()>`，`shutdown()` 无返回值
- [ ] FIX-02: `as_any()` 和 `is()` 已移除
- [ ] FIX-03: `CacheConfigError` 与 `CacheError` 分离
- [ ] FIX-04: `new_in_memory()` 工厂函数存在
- [ ] FIX-05: CacheBackend 拆分为 CacheReader/CacheWriter/CacheConnector
- [ ] FIX-06: 注册表需显式初始化
- [ ] FIX-07: `CacheConfig` 结构体存在且可序列化
- [ ] 所有测试通过
- [ ] 零警告
- [ ] 文档完整

---

## 回滚计划

如果出现问题：

1. 每个 Phase 都有独立提交，可以使用 `git revert` 回滚特定提交
2. 保留旧文件直到验证完成
3. 每个 Task 后运行测试，尽早发现问题
