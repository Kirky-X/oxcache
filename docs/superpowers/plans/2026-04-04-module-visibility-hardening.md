# 模块可见性加固计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将内部实现模块的可见性从 `pub` 改为 `pub(crate)`，遵循最小权限原则

**Architecture:**

1. 识别应改为 `pub(crate)` 的模块
2. 更新 `lib.rs` 中的模块声明
3. 添加必要的 re-exports 以保持公共 API 不变

**Tech Stack:** Rust

---

## 需要加固的模块

| 模块             | 当前可见性 | 建议可见性   | 理由               |
| ---------------- | ---------- | ------------ | ------------------ |
| `security`       | `pub`      | `pub(crate)` | 安全验证是内部 API |
| `bloom_filter`   | `pub`      | `pub(crate)` | 外部不应直接操作   |
| `rate_limiting`  | `pub`      | `pub(crate)` | 限流是内部保护机制 |
| `smart_strategy` | `pub`      | `pub(crate)` | 智能策略是内部优化 |

---

## Task 1: 将 security 模块改为 pub(crate)

**Files:**

- Modify: `src/lib.rs:509-510`

- [ ] **Step 1: 查看当前声明**

```rust
// src/lib.rs 行 509-510
#[cfg(any(feature = "redis", feature = "full"))]
pub mod security;
```

- [ ] **Step 2: 改为 pub(crate)**

```rust
// 修改为
#[cfg(any(feature = "redis", feature = "full"))]
pub(crate) mod security;
```

- [ ] **Step 3: 检查外部依赖**

搜索是否有外部代码使用 `oxcache::security`：

```bash
grep -rn "oxcache::security" src/
```

如果找到内部使用，确认这些使用仍在同一 crate 内。

- [ ] **Step 4: 添加 re-export（如果需要公共 API）**

如果某些 security 函数需要对外暴露，在 `lib.rs` 添加选择性 re-export：

```rust
// 仅导出必要的公共 API
#[cfg(any(feature = "redis", feature = "full"))]
pub use security::redaction::{redact_connection_string, redact_cache_key};
```

- [ ] **Step 5: 验证编译**

```bash
cargo build --features full
```

- [ ] **Step 6: 提交**

```bash
git add src/lib.rs
git commit -m "refactor(visibility): security 模块改为 pub(crate)"
```

---

## Task 2: 将 bloom_filter 模块改为 pub(crate)

**Files:**

- Modify: `src/lib.rs:450-451`

- [ ] **Step 1: 查看当前声明**

```rust
// src/lib.rs 行 450-451
#[cfg(any(feature = "bloom-filter", feature = "full"))]
pub mod bloom_filter;
```

- [ ] **Step 2: 改为 pub(crate)**

```rust
// 修改为
#[cfg(any(feature = "bloom-filter", feature = "full"))]
pub(crate) mod bloom_filter;
```

- [ ] **Step 3: 检查是否有公共 bloom_filter 使用**

```bash
grep -rn "use.*bloom_filter" src/
```

确认所有使用都在 crate 内部。

- [ ] **Step 4: 验证编译**

```bash
cargo build --features bloom-filter
```

- [ ] **Step 5: 提交**

```bash
git add src/lib.rs
git commit -m "refactor(visibility): bloom_filter 模块改为 pub(crate)"
```

---

## Task 3: 将 rate_limiting 模块改为 pub(crate)

**Files:**

- Modify: `src/lib.rs:467-468`

- [ ] **Step 1: 查看当前声明**

```rust
// src/lib.rs 行 467-468
#[cfg(any(feature = "rate-limiting", feature = "full"))]
pub mod rate_limiting;
```

- [ ] **Step 2: 改为 pub(crate)**

```rust
// 修改为
#[cfg(any(feature = "rate-limiting", feature = "full"))]
pub(crate) mod rate_limiting;
```

- [ ] **Step 3: 检查是否有导出的类型需要保留**

查看 `lib.rs` 中是否有 `pub use rate_limiting::*`：

```bash
grep -n "pub use.*rate_limiting" src/lib.rs
```

如果有，这些类型仍需要 re-export：

```rust
// 保持公共 API（如果有需要）
#[cfg(any(feature = "rate-limiting", feature = "full"))]
pub use rate_limiting::{RateLimitConfig, ClientRateLimiter};
```

- [ ] **Step 4: 验证编译和测试**

```bash
cargo test --features rate-limiting
```

- [ ] **Step 5: 提交**

```bash
git add src/lib.rs
git commit -m "refactor(visibility): rate_limiting 模块改为 pub(crate)"
```

---

## Task 4: 将 smart_strategy 模块改为 pub(crate)

**Files:**

- Modify: `src/lib.rs:500-501,573-577`

- [ ] **Step 1: 查看当前声明**

```rust
// src/lib.rs 行 500-501
#[cfg(any(feature = "smart-strategy", feature = "full"))]
pub mod smart_strategy;

// 行 573-577 (re-export)
#[cfg(any(feature = "smart-strategy", feature = "full"))]
pub use smart_strategy::{
    CompressibilityChecker, CompressionDecider, HitRateCollector, HitRateStats, PrefetchDecider, SmartStrategyConfig,
    SmartStrategyManager,
};
```

- [ ] **Step 2: 改模块声明为 pub(crate)**

```rust
// 修改为
#[cfg(any(feature = "smart-strategy", feature = "full"))]
pub(crate) mod smart_strategy;
```

- [ ] **Step 3: 保持必要的 re-export**

如果 `SmartStrategyManager` 等类型需要对外暴露，保留 re-export：

```rust
// 保持必要的公共 API
#[cfg(any(feature = "smart-strategy", feature = "full"))]
pub use smart_strategy::{SmartStrategyManager, SmartStrategyConfig};
```

如果这些类型完全不需要对外暴露，删除 re-export。

- [ ] **Step 4: 验证编译**

```bash
cargo build --features smart-strategy
```

- [ ] **Step 5: 提交**

```bash
git add src/lib.rs
git commit -m "refactor(visibility): smart_strategy 模块改为 pub(crate)"
```

---

## Task 5: 添加 #![deny(unsafe_code)]

**Files:**

- Modify: `src/lib.rs`

- [ ] **Step 1: 在 lib.rs 顶部添加 lint**

```rust
// src/lib.rs 在现有 lints 附近添加
#![deny(unsafe_code)]
```

- [ ] **Step 2: 处理编译错误**

如果有 unsafe 代码，需要：

1. 添加 `#[allow(unsafe_code)]` 到特定模块/函数
2. 添加 `// SAFETY:` 注释解释安全保证

检查现有的 unsafe 使用：

```bash
grep -rn "unsafe" src/
```

- [ ] **Step 3: 为现有 unsafe 添加注释**

对于每个 `unsafe` 块，添加 `// SAFETY:` 注释：

```rust
// SAFETY: 这里解释为什么这段 unsafe 代码是安全的
unsafe {
    // ...
}
```

- [ ] **Step 4: 验证编译**

```bash
cargo build --all-features
```

- [ ] **Step 5: 提交**

```bash
git add src/lib.rs
git commit -m "security: 添加 #![deny(unsafe_code)] lint"
```

---

## Task 6: 最终验证

- [ ] **Step 1: 运行完整测试**

```bash
cargo test --all-features
```

- [ ] **Step 2: 检查 clippy**

```bash
cargo clippy --all-features -- -D warnings
```

- [ ] **Step 3: 检查公开 API 文档**

```bash
cargo doc --all-features --no-deps --open
```

确认只有预期的类型出现在公开文档中。

- [ ] **Step 4: 更新 AGENTS.md（可选）**

如果可见性规则需要记录，更新 `AGENTS.md`：

```markdown
## 可见性规则

- `pub`: 公共 API，外部可调用
- `pub(crate)`: 内部实现，仅 crate 内可用
- `pub(super)`: 父模块可用
- 私有（默认）: 仅当前模块可用

### 内部模块（pub(crate)）

- `security`: 安全验证
- `bloom_filter`: 布隆过滤器
- `rate_limiting`: 速率限制
- `smart_strategy`: 智能策略
```

---

## 完成标准

- [ ] 4 个模块已改为 `pub(crate)`
- [ ] 必要的公共 API 已 re-export
- [ ] `#![deny(unsafe_code)]` 已添加
- [ ] 所有测试通过
- [ ] 公开 API 文档正确
