# 第三方库替换计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 使用成熟的第三方 crate 替换自实现的速率限制和熔断器

**Architecture:**

1. 使用 `governor` crate 替换 `rate_limiting.rs` 中的 TokenBucket 实现
2. 使用 `circuit-breaker` 或自定义精简实现替换 `circuit_breaker/mod.rs`

**Tech Stack:** Rust, governor, async-trait

---

## 背景

| 模块     | 当前行数 | 推荐 Crate             | 成熟度     |
| -------- | -------- | ---------------------- | ---------- |
| 速率限制 | 419 行   | `governor`             | ⭐⭐⭐⭐⭐ |
| 熔断器   | 301 行   | 简化自实现或 `failing` | ⭐⭐⭐     |

---

## Task 1: 添加 governor 依赖

**Files:**

- Modify: `Cargo.toml`

- [ ] **Step 1: 添加 governor 到依赖**

```toml
# Cargo.toml 在 [dependencies] 部分
governor = { version = "0.6", optional = true }
```

- [ ] **Step 2: 添加到 features**

```toml
# 在 [features] 部分
rate-limiting = ["dep:governor", "dep:dashmap"]
```

- [ ] **Step 3: 验证依赖下载**

```bash
cargo fetch
```

- [ ] **Step 4: 提交**

```bash
git add Cargo.toml
git commit -m "chore: 添加 governor crate 作为 rate-limiting 依赖"
```

---

## Task 2: 使用 governor 重写速率限制模块

**Files:**

- Rewrite: `src/rate_limiting.rs`
- Test: `src/rate_limiting.rs` 内置测试

- [ ] **Step 1: 理解 governor API**

`governor` 提供两种主要类型：

- `RateLimiter`: 基于令牌桶的速率限制器
- `Jitter`: 添加随机抖动防止惊群效应

示例：

```rust
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

let quota = Quota::per_second(NonZeroU32::new(10).unwrap());
let limiter = RateLimiter::direct(quota);

if limiter.check().is_ok() {
    // 允许请求
} else {
    // 拒绝请求
}
```

- [ ] **Step 2: 重写 RateLimitConfig**

```rust
// src/rate_limiting.rs 完全重写

#[cfg(feature = "rate-limiting")]
use governor::{
    clock::DefaultClock,
    middleware::NoOpMiddleware,
    Quota, RateLimiter,
};
#[cfg(feature = "rate-limiting")]
use std::num::NonZeroU32;

/// 速率限制配置
#[cfg(feature = "rate-limiting")]
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// 每秒允许的最大请求数
    pub max_requests_per_second: u32,
    /// 令牌桶容量（突发流量处理能力）
    pub burst_capacity: u32,
}

#[cfg(feature = "rate-limiting")]
impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests_per_second: 1000,
            burst_capacity: 2000,
        }
    }
}
```

- [ ] **Step 3: 重写 ClientRateLimiter**

```rust
// src/rate_limiting.rs 继续

/// 客户端级别的速率限制器
#[cfg(feature = "rate-limiting")]
pub struct ClientRateLimiter {
    per_client: dashmap::DashMap<String, Arc<RateLimiter<governor::state::direct::NotKeyed, governor::state::InMemoryState, DefaultClock, NoOpMiddleware>>>,
    global_limit: Arc<RateLimiter<governor::state::direct::NotKeyed, governor::state::InMemoryState, DefaultClock, NoOpMiddleware>>,
    config: RateLimitConfig,
}

#[cfg(feature = "rate-limiting")]
impl ClientRateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        let quota = Quota::per_second(NonZeroU32::new(config.max_requests_per_second).unwrap())
            .allow_burst(NonZeroU32::new(config.burst_capacity).unwrap());

        Self {
            per_client: dashmap::DashMap::new(),
            global_limit: Arc::new(RateLimiter::direct(quota)),
            config,
        }
    }

    /// 检查是否允许请求
    pub fn check(&self, client_id: &str, cost: u32) -> bool {
        // 先检查全局限制
        if self.global_limit.check().is_err() {
            return false;
        }

        // 再检查客户端限制
        let limiter = self.per_client
            .entry(client_id.to_string())
            .or_insert_with(|| {
                let quota = Quota::per_second(NonZeroU32::new(self.config.max_requests_per_second).unwrap())
                    .allow_burst(NonZeroU32::new(self.config.burst_capacity).unwrap());
                Arc::new(RateLimiter::direct(quota))
            });

        limiter.check().is_ok()
    }

    /// 获取或创建客户端限流器
    pub fn get_client_limiter(&self, client_id: &str) -> Arc<RateLimiter<governor::state::direct::NotKeyed, governor::state::InMemoryState, DefaultClock, NoOpMiddleware>> {
        self.per_client
            .entry(client_id.to_string())
            .or_insert_with(|| {
                let quota = Quota::per_second(NonZeroU32::new(self.config.max_requests_per_second).unwrap())
                    .allow_burst(NonZeroU32::new(self.config.burst_capacity).unwrap());
                Arc::new(RateLimiter::direct(quota))
            })
            .clone()
    }
}
```

- [ ] **Step 4: 保持 API 兼容性**

导出相同的公共 API：

```rust
// 保持原有导出
pub use self::RateLimitConfig;
pub use self::ClientRateLimiter;
```

- [ ] **Step 5: 更新测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_allows_within_limit() {
        let config = RateLimitConfig {
            max_requests_per_second: 10,
            burst_capacity: 20,
        };
        let limiter = ClientRateLimiter::new(config);

        for _ in 0..20 {
            assert!(limiter.check("test-client", 1));
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let config = RateLimitConfig {
            max_requests_per_second: 5,
            burst_capacity: 5,
        };
        let limiter = ClientRateLimiter::new(config);

        for _ in 0..5 {
            assert!(limiter.check("test-client", 1));
        }

        // 应该被限制
        assert!(!limiter.check("test-client", 1));
    }
}
```

- [ ] **Step 6: 验证测试**

```bash
cargo test rate_limiting --features rate-limiting -- --nocapture
```

- [ ] **Step 7: 提交**

```bash
git add src/rate_limiting.rs
git commit -m "refactor: 使用 governor crate 替换自实现的 TokenBucket

- 完全重写速率限制模块使用 governor
- 保持原有 API 兼容性
- 移除自定义的 TokenBucket 实现
- 减少 ~250 行自实现代码"
```

---

## Task 3: 评估熔断器替换方案

**Files:**

- Analyze: `src/circuit_breaker/mod.rs`

- [ ] **Step 1: 分析当前实现**

读取 `src/circuit_breaker/mod.rs`，理解其特性：

- 三态状态机 (Closed/Open/HalfOpen)
- 失败阈值
- 恢复超时
- 手动/自动恢复

- [ ] **Step 2: 评估第三方选项**

| Crate             | 优点     | 缺点         |
| ----------------- | -------- | ------------ |
| `failing`         | 简单轻量 | 功能较少     |
| `circuit-breaker` | 功能完整 | 维护状态不明 |
| 保持自实现        | 完全控制 | 维护负担     |

**建议**: 当前实现已经足够精简，建议保留自实现但添加 `#[deny(unsafe_code)]` 和更多测试。

- [ ] **Step 3: 添加改进（可选）**

如果决定保留自实现，添加以下改进：

```rust
// 添加指标追踪
pub struct CircuitBreakerMetrics {
    pub total_requests: AtomicU64,
    pub successful_requests: AtomicU64,
    pub failed_requests: AtomicU64,
    pub rejected_requests: AtomicU64,
}
```

- [ ] **Step 4: 提交分析结果**

```bash
git add docs/superpowers/plans/2026-04-04-third-party-replacement.md
git commit -m "docs: 熔断器保留自实现，已评估第三方选项"
```

---

## Task 4: 清理废弃的 TokenBucket 代码

**Files:**

- Modify: `src/rate_limiting.rs`

- [ ] **Step 1: 删除 TokenBucket 结构体定义**

删除 `TokenBucket` 结构体及其所有方法（约行 44-140）。

- [ ] **Step 2: 删除 now_millis 辅助函数**

不再需要，因为 governor 使用自己的时钟。

- [ ] **Step 3: 验证没有遗漏的引用**

```bash
grep -rn "TokenBucket" src/
```

Expected: 无结果

- [ ] **Step 4: 提交**

```bash
git add src/rate_limiting.rs
git commit -m "refactor: 删除废弃的 TokenBucket 实现"
```

---

## Task 5: 最终验证

- [ ] **Step 1: 运行完整测试**

```bash
cargo test --all-features
```

- [ ] **Step 2: 检查依赖树**

```bash
cargo tree -i governor
```

Expected: 显示 governor 被正确引入

- [ ] **Step 3: 性能基准测试（可选）**

比较新旧实现的性能差异。

---

## 完成标准

- [ ] `governor` 已添加到 Cargo.toml
- [ ] `rate_limiting.rs` 使用 governor 重写
- [ ] 所有测试通过
- [ ] API 保持向后兼容
- [ ] 熔断器已评估并记录决策
