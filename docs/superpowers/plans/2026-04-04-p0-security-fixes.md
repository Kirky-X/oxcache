# P0 安全修复实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 TokenBucket 时间处理、移除生产代码中的 unwrap()、启用 rate-limiting 默认特性

**Architecture:**

1. TokenBucket 改用 `std::time::Instant` 替代 `SystemTime`，避免时间回退问题
2. 所有生产代码中的 `unwrap()`/`expect()` 改为 `?` 或 `map_err()` 错误传播
3. 将 `rate-limiting` 加入 Cargo.toml 的 default features

**Tech Stack:** Rust, tokio, dashmap

---

## 文件变更清单

| 文件                           | 操作 | 变更内容                    |
| ------------------------------ | ---- | --------------------------- |
| `src/rate_limiting.rs`         | 修改 | TokenBucket 使用 Instant    |
| `src/bloom_filter.rs`          | 修改 | 移除 unwrap()               |
| `src/builder/cache_builder.rs` | 修改 | 移除 unwrap()               |
| `src/database/common.rs`       | 修改 | 移除 unwrap()               |
| `Cargo.toml`                   | 修改 | 启用 rate-limiting 默认特性 |

---

## Task 1: 修复 TokenBucket 时间处理

**Files:**

- Modify: `src/rate_limiting.rs:66-78`
- Test: `src/rate_limiting.rs` 内置测试

### 分析

当前实现使用 `SystemTime::now().duration_since(UNIX_EPOCH)` 获取时间戳，时间回退时会返回 `Duration::ZERO`，导致令牌立即补满，绕过速率限制。

正确的做法是使用 `std::time::Instant`，它是单调时钟，不受系统时间调整影响。

- [ ] **Step 1: 理解当前实现的问题**

阅读 `src/rate_limiting.rs` 行 65-78：

```rust
#[inline]
fn now_millis() -> u64 {
    let since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or({
            std::time::Duration::ZERO
        });
    since_epoch.as_millis() as u64
}
```

问题：`SystemTime` 会受 NTP 调整影响，时间回退时返回 `Duration::ZERO`。

- [ ] **Step 2: 修改 TokenBucket 使用 Instant**

需要重构整个 TokenBucket 以使用 `Instant` 而非毫秒时间戳。

```rust
// src/rate_limiting.rs 行 44-62 替换为：

/// 令牌桶速率限制器
///
/// 使用令牌桶算法实现精确的速率限制，支持突发流量
#[cfg(feature = "rate-limiting")]
#[derive(Debug)]
pub struct TokenBucket {
    tokens: AtomicU64,
    last_update: std::sync::Mutex<std::time::Instant>, // 改用 Mutex<Instant>
    capacity: u64,
    refill_rate: u64, // 每秒补充的令牌数
}

#[cfg(feature = "rate-limiting")]
impl TokenBucket {
    /// 创建新的令牌桶
    pub fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            tokens: AtomicU64::new(capacity),
            last_update: std::sync::Mutex::new(std::time::Instant::now()),
            capacity,
            refill_rate,
        }
    }
```

- [ ] **Step 3: 重写 try_acquire_n 方法**

```rust
// src/rate_limiting.rs 行 93-128 替换为：

/// 尝试获取多个令牌
///
/// 使用 Mutex 保护 last_update，确保时间计算正确
pub fn try_acquire_n(&self, n: u64) -> bool {
    let mut last_update = self.last_update.lock().unwrap();
    let now = std::time::Instant::now();
    let elapsed = now.duration_since(*last_update);
    let refill = (elapsed.as_millis() as u64 * self.refill_rate) / 1000;

    let current_tokens = self.tokens.load(Ordering::SeqCst);
    let new_tokens = (current_tokens + refill).min(self.capacity);

    if new_tokens < n {
        return false;
    }

    // 更新 tokens 和 last_update
    self.tokens.store(new_tokens - n, Ordering::SeqCst);
    *last_update = now;
    true
}
```

- [ ] **Step 4: 更新 available_tokens 方法**

```rust
// src/rate_limiting.rs 行 130-139 替换为：

/// 获取当前可用令牌数
pub fn available_tokens(&self) -> u64 {
    let last_update = self.last_update.lock().unwrap();
    let now = std::time::Instant::now();
    let current_tokens = self.tokens.load(Ordering::Relaxed);
    let elapsed = now.duration_since(*last_update);
    let refill = (elapsed.as_millis() as u64 * self.refill_rate) / 1000;

    (current_tokens + refill).min(self.capacity)
}
```

- [ ] **Step 5: 运行测试验证**

```bash
cargo test rate_limiting --features rate-limiting -- --nocapture
```

Expected: 所有测试通过

- [ ] **Step 6: 提交**

```bash
git add src/rate_limiting.rs
git commit -m "fix(security): TokenBucket 使用 Instant 替代 SystemTime

- 使用 std::time::Instant 单调时钟避免时间回退问题
- 改用 Mutex<Instant> 保护 last_update
- 修复速率限制可被绕过的安全漏洞"
```

---

## Task 2: 移除 bloom_filter.rs 中的 unwrap()

**Files:**

- Modify: `src/bloom_filter.rs:98,115`

- [ ] **Step 1: 查看当前代码**

```rust
// 行 98
std::num::NonZeroUsize::new(10000).unwrap()

// 行 115
let hash = murmur3_32(&mut item, seed).unwrap_or(0);
```

- [ ] **Step 2: 修复行 98**

```rust
// src/bloom_filter.rs 行 98 替换为：
std::num::NonZeroUsize::new(10000)
    .expect("10000 is a valid non-zero usize")
```

注：这是一个常量值，使用 `expect()` 并添加解释性消息是合理的。

- [ ] **Step 3: 验证行 115 已使用 unwrap_or**

行 115 已使用 `unwrap_or(0)`，这是正确的处理方式。

- [ ] **Step 4: 运行测试**

```bash
cargo test bloom_filter --features bloom-filter -- --nocapture
```

- [ ] **Step 5: 提交**

```bash
git add src/bloom_filter.rs
git commit -m "fix: bloom_filter 添加 expect 解释性消息"
```

---

## Task 3: 移除 builder/cache_builder.rs 中的 unwrap()

**Files:**

- Modify: `src/builder/cache_builder.rs:1040`

- [ ] **Step 1: 查看当前代码**

```rust
// 行 1040 附近
let backend = self.build_backend().await.unwrap();
```

- [ ] **Step 2: 修改为错误传播**

找到具体的 `unwrap()` 调用位置并替换为 `?` 操作符。

需要先读取完整上下文：

```bash
grep -n "unwrap()" src/builder/cache_builder.rs | head -20
```

- [ ] **Step 3: 逐个替换 unwrap()**

对于构建器中的 `unwrap()`，应改为返回 `Result` 类型：

```rust
// 如果函数签名返回 Result，直接使用 ?
let backend = self.build_backend().await?;

// 如果是测试代码中的 expect，添加描述性消息
.expect("backend build should not fail in test")
```

- [ ] **Step 4: 运行测试验证**

```bash
cargo test builder --features full -- --nocapture
```

- [ ] **Step 5: 提交**

```bash
git add src/builder/cache_builder.rs
git commit -m "fix: cache_builder 移除 unwrap()，改用错误传播"
```

---

## Task 4: 移除 database/common.rs 中的 unwrap()

**Files:**

- Modify: `src/database/common.rs:223,260,264`

- [ ] **Step 1: 查看当前代码**

```rust
// 行 223
pool.get_connection().await.unwrap()

// 行 260, 264
pool.get_connection().await.unwrap()
```

- [ ] **Step 2: 替换为错误传播**

```rust
// 原代码
let conn = pool.get_connection().await.unwrap();

// 改为
let conn = pool.get_connection().await.map_err(|e| {
    CacheError::DatabaseError(format!("Failed to get connection: {}", e))
})?;
```

- [ ] **Step 3: 运行测试验证**

```bash
cargo test database --features database -- --nocapture
```

- [ ] **Step 4: 提交**

```bash
git add src/database/common.rs
git commit -m "fix: database/common 移除 unwrap()，返回明确错误"
```

---

## Task 5: 启用 rate-limiting 默认特性

**Files:**

- Modify: `Cargo.toml`

- [ ] **Step 1: 查看当前 default features**

```toml
# 当前配置 (约行 20-25)
default = ["moka", "serialization", "metrics", "redis", "compression", "full-metrics"]
```

- [ ] **Step 2: 添加 rate-limiting 到 default**

```toml
# 修改为
default = ["moka", "serialization", "metrics", "redis", "compression", "full-metrics", "rate-limiting"]
```

- [ ] **Step 3: 更新特性依赖检查**

检查 `rate-limiting` 是否依赖 `moka`（根据 lib.rs 行 631 的 `check_feature_dependence!("moka", "rate-limiting")`）。

由于 `moka` 已在 default 中，无需额外修改。

- [ ] **Step 4: 验证编译**

```bash
cargo build --release
```

Expected: 编译成功

- [ ] **Step 5: 提交**

```bash
git add Cargo.toml
git commit -m "feat: 启用 rate-limiting 为默认特性

- 添加 rate-limiting 到 default features
- 确保用户默认获得 DoS 保护"
```

---

## Task 6: 最终验证

- [ ] **Step 1: 运行完整测试套件**

```bash
cargo test --all-features
```

Expected: 所有测试通过

- [ ] **Step 2: 运行 clippy 检查**

```bash
cargo clippy --all-features -- -D warnings
```

Expected: 无警告

- [ ] **Step 3: 创建最终提交（如果有遗漏）**

```bash
git status
# 如果有未提交的更改，提交它们
```

---

## 完成标准

- [ ] TokenBucket 使用 `Instant` 而非 `SystemTime`
- [ ] 所有生产代码中的 `unwrap()` 已移除或改为 `expect()`/`?`
- [ ] `rate-limiting` 在 default features 中启用
- [ ] 所有测试通过
- [ ] clippy 无警告
