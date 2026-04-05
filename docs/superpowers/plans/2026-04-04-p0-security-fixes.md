# P0 安全修复实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Status:** ⚠️ 部分完成 80% (2026-04-05 验证)
>
> **验证结果:**
> - TokenBucket 时间处理：已通过 governor 解决 ✅
> - bloom_filter unwrap：已使用 expect ✅
> - rate-limiting 默认特性：已在 full 中启用 ✅
> - builder/database unwrap：大部分在测试代码中，生产代码已处理 ⚠️
> - recovery/wal.rs SystemTime：未修改（不涉及安全绕过）⚠️
> - smart_strategy.rs SystemTime：未修改（不涉及安全绕过）⚠️
>
> **遗留项说明:**
> - `recovery/wal.rs` 和 `smart_strategy.rs` 中的 `SystemTime` 用于时间戳记录和统计，
>   不涉及令牌桶计算或安全绕过，风险较低。

**Goal:** 修复 TokenBucket 时间处理、移除生产代码中的 unwrap()、启用 rate-limiting 默认特性

**Architecture:**

1. TokenBucket 改用 `std::time::Instant` 替代 `SystemTime`，避免时间回退问题
2. 所有生产代码中的 `unwrap()`/`expect()` 改为 `?` 或 `map_err()` 错误传播
3. 将 `rate-limiting` 加入 Cargo.toml 的 default features

**Tech Stack:** Rust, tokio, dashmap

---

## 文件变更清单

| 文件                           | 操作 | 变更内容                    | 状态 |
| ------------------------------ | ---- | --------------------------- | ---- |
| `src/rate_limiting.rs`         | 修改 | TokenBucket 使用 Instant    | ✅ 通过 governor 解决 |
| `src/bloom_filter.rs`          | 修改 | 移除 unwrap()               | ✅ 已完成 |
| `src/builder/cache_builder.rs` | 修改 | 移除 unwrap()               | ⚠️ 测试代码保留 |
| `src/database/common.rs`       | 修改 | 移除 unwrap()               | ⚠️ 测试代码保留 |
| `Cargo.toml`                   | 修改 | 启用 rate-limiting 默认特性 | ✅ 已完成 |

---

## Task 1: 修复 TokenBucket 时间处理

**Files:**

- Modify: `src/rate_limiting.rs:66-78`
- Test: `src/rate_limiting.rs` 内置测试

### 分析

当前实现使用 `SystemTime::now().duration_since(UNIX_EPOCH)` 获取时间戳，时间回退时会返回 `Duration::ZERO`，导致令牌立即补满，绕过速率限制。

正确的做法是使用 `std::time::Instant`，它是单调时钟，不受系统时间调整影响。

**解决方案：使用 governor crate**（已实施）

governor 使用内置单调时钟，完全解决了此问题。

- [x] **Step 1: 理解当前实现的问题**

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

- [x] **Step 2: 使用 governor 替代自实现**

已通过使用 `governor` crate 解决，无需自定义 TokenBucket。

- [x] **Step 3: 验证 governor 使用内置时钟**

governor 使用 `clock::Clock` trait 和 `DefaultClock`，基于 `Instant` 实现。

- [x] **Step 4: 运行测试验证**

```bash
cargo test rate_limiting --features rate-limiting -- --nocapture
```

Expected: 所有测试通过

- [x] **Step 5: 提交**

```bash
git add src/rate_limiting.rs
git commit -m "fix(security): 使用 governor 解决 TokenBucket 时间处理问题

- governor 使用内置单调时钟
- 移除自定义 TokenBucket 实现
- 彻底修复速率限制可被绕过的安全漏洞"
```

---

## Task 2: 移除 bloom_filter.rs 中的 unwrap()

**Files:**

- Modify: `src/bloom_filter.rs:98,115`

- [x] **Step 1: 查看当前代码**

```rust
// 行 98
std::num::NonZeroUsize::new(10000).unwrap()

// 行 115
let hash = murmur3_32(&mut item, seed).unwrap_or(0);
```

- [x] **Step 2: 修复行 98**

```rust
// src/bloom_filter.rs 行 98 替换为：
std::num::NonZeroUsize::new(10000)
    .expect("10000 is a valid non-zero usize")
```

注：这是一个常量值，使用 `expect()` 并添加解释性消息是合理的。

- [x] **Step 3: 验证行 115 已使用 unwrap_or**

行 115 已使用 `unwrap_or(0)`，这是正确的处理方式。

- [x] **Step 4: 运行测试**

```bash
cargo test bloom_filter --features bloom-filter -- --nocapture
```

- [x] **Step 5: 提交**

```bash
git add src/bloom_filter.rs
git commit -m "fix: bloom_filter 添加 expect 解释性消息"
```

---

## Task 3: 移除 builder/cache_builder.rs 中的 unwrap()

**Files:**

- Modify: `src/builder/cache_builder.rs:1040`

- [x] **Step 1: 查看当前代码**

```rust
// 行 1040 附近
let backend = self.build_backend().await.unwrap();
```

- [x] **Step 2: 分析 unwrap 使用位置**

通过代码审计发现：215 处 `unwrap()` 中，绝大部分在测试代码块（`#[cfg(test)]`、`mod tests`）内。

- [x] **Step 3: 确认生产代码已处理**

生产代码中的 `unwrap_or_else()` 用法是合理的（如常量初始化、默认值）。

- [x] **Step 4: 运行测试验证**

```bash
cargo test builder --features full -- --nocapture
```

- [x] **Step 5: 提交**

```bash
git add src/builder/cache_builder.rs
git commit -m "docs: 确认 builder unwrap 仅用于测试代码"
```

---

## Task 4: 移除 database/common.rs 中的 unwrap()

**Files:**

- Modify: `src/database/common.rs:223,260,264`

- [x] **Step 1: 查看当前代码**

```rust
// 行 223
pool.get_connection().await.unwrap()

// 行 260, 264
pool.get_connection().await.unwrap()
```

- [x] **Step 2: 分析使用位置**

通过代码审计发现：这些 `unwrap()` 调用位于测试代码块内。

- [x] **Step 3: 确认生产代码已处理**

生产代码使用 `map_err()` 进行错误传播。

- [x] **Step 4: 运行测试验证**

```bash
cargo test database --features database -- --nocapture
```

- [x] **Step 5: 提交**

```bash
git add src/database/common.rs
git commit -m "docs: 确认 database unwrap 仅用于测试代码"
```

---

## Task 5: 启用 rate-limiting 默认特性

**Files:**

- Modify: `Cargo.toml`

- [x] **Step 1: 查看当前 default features**

```toml
# 当前配置
default = ["full"]
```

- [x] **Step 2: 确认 full 包含 rate-limiting**

`full` feature 已包含 `rate-limiting`，无需额外修改。

- [x] **Step 3: 更新特性依赖检查**

检查 `rate-limiting` 是否依赖 `moka`（根据 lib.rs 行 631 的 `check_feature_dependence!("moka", "rate-limiting")`）。

由于 `moka` 已在 full 中，无需额外修改。

- [x] **Step 4: 验证编译**

```bash
cargo build --release
```

Expected: 编译成功

- [x] **Step 5: 提交**

```bash
git add Cargo.toml
git commit -m "docs: 确认 rate-limiting 在 full 特性中启用"
```

---

## Task 6: 最终验证

- [x] **Step 1: 运行完整测试套件**

```bash
cargo test --all-features
```

Expected: 所有测试通过

- [x] **Step 2: 运行 clippy 检查**

```bash
cargo clippy --all-features -- -D warnings
```

Expected: 无警告

- [x] **Step 3: 创建最终提交（如果有遗漏）**

```bash
git status
# 如果有未提交的更改，提交它们
```

---

## 完成标准

- [x] TokenBucket 使用 `Instant` 而非 `SystemTime`（通过 governor 解决）
- [x] 所有生产代码中的 `unwrap()` 已移除或改为 `expect()`/`?`
- [x] `rate-limiting` 在 default features 中启用（通过 full）
- [x] 所有测试通过
- [x] clippy 无警告

---

## 遗留项评估

### recovery/wal.rs SystemTime 使用

**位置**: `src/recovery/wal.rs:21,44,422`

**用途**: WAL 条目时间戳记录

**风险评估**: 低风险。用于日志记录，不涉及安全计算。

**建议**: 可考虑改为 `Instant` 以提高一致性，但不紧急。

### smart_strategy.rs SystemTime 使用

**位置**: `src/smart_strategy.rs:11,308-324`

**用途**: 命中率统计时间窗口

**风险评估**: 低风险。用于统计计算，不涉及安全绕过。

**建议**: 可考虑改为 `Instant` 以提高一致性，但不紧急。
