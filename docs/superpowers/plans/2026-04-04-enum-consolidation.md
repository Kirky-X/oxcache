# 枚举统一重构计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 合并 RedisMode/RedisModeType、BackendType、CacheLayer/Layer 等重复枚举定义，消除代码重复

**Architecture:**

1. 保留 `core/types.rs` 中的枚举定义作为唯一真实来源 (Single Source of Truth)
2. 删除 `backend/client/redis/client.rs` 中的 `RedisMode` 枚举，改用 `core::types::RedisModeType`
3. 删除 `backend/custom_tiered.rs` 中的 `BackendType` 和 `Layer` 枚举，改用 `core::types` 定义
4. 更新所有引用点使用统一枚举

**Tech Stack:** Rust, serde

---

## 问题分析

### 重复枚举清单

| 枚举名                        | 位置 1                | 位置 2                                 | 位置 3                             |
| ----------------------------- | --------------------- | -------------------------------------- | ---------------------------------- |
| `RedisMode` / `RedisModeType` | `core/types.rs:10-20` | `backend/client/redis/client.rs:18-27` | -                                  |
| `BackendType`                 | `core/types.rs:33-44` | `backend/custom_tiered.rs:365-391`     | -                                  |
| `CacheLayer` / `Layer`        | `core/types.rs:58-64` | `metrics/unified.rs:462-463`           | `backend/custom_tiered.rs:314-315` |

### 统一策略

1. **RedisModeType** (保留): `core/types.rs` 版本最完整，有 Serialize/Deserialize
2. **BackendType** (保留): `core/types.rs` 版本最完整
3. **CacheLayer** (保留): `core/types.rs` 版本最完整

---

## 文件变更清单

| 文件                                 | 操作 | 变更内容                                    |
| ------------------------------------ | ---- | ------------------------------------------- |
| `src/core/types.rs`                  | 保留 | 唯一真实来源                                |
| `src/backend/client/redis/client.rs` | 修改 | 删除 RedisMode，导入 RedisModeType          |
| `src/backend/custom_tiered.rs`       | 修改 | 删除 BackendType 和 Layer，导入 core::types |
| `src/metrics/unified.rs`             | 修改 | 删除 CacheLayer，导入 core::types           |
| `src/builder/cache_builder.rs`       | 修改 | 更新 RedisMode 引用为 RedisModeType         |

---

## Task 1: 删除 redis/client.rs 中的重复 RedisMode

**Files:**

- Modify: `src/backend/client/redis/client.rs:18-27`
- Test: `cargo test --features redis`

- [ ] **Step 1: 查看当前重复定义**

```rust
// src/backend/client/redis/client.rs 行 18-27
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedisMode {
    #[default]
    Standalone,
    Sentinel,
    Cluster,
}
```

- [ ] **Step 2: 添加导入语句**

在文件顶部添加：

```rust
// src/backend/client/redis/client.rs 行 6 附近
use crate::core::types::RedisModeType;

// 创建类型别名以保持兼容性 (可选，后续可移除)
pub type RedisMode = RedisModeType;
```

- [ ] **Step 3: 删除 RedisMode 定义**

删除行 18-27 的 RedisMode 定义。

- [ ] **Step 4: 更新所有 RedisMode 引用**

```bash
# 搜索文件内所有 RedisMode 引用
grep -n "RedisMode::" src/backend/client/redis/client.rs
```

将 `RedisMode::Standalone` 改为 `RedisModeType::Standalone`（或通过类型别名保持兼容）。

- [ ] **Step 5: 验证编译**

```bash
cargo build --features redis
```

- [ ] **Step 6: 提交**

```bash
git add src/backend/client/redis/client.rs
git commit -m "refactor: 删除重复的 RedisMode 枚举，统一使用 core::types::RedisModeType"
```

---

## Task 2: 删除 custom_tiered.rs 中的重复 BackendType

**Files:**

- Modify: `src/backend/custom_tiered.rs:365-401`
- Test: `cargo test --features full`

- [ ] **Step 1: 查看当前重复定义**

```rust
// src/backend/custom_tiered.rs 行 365-401
#[cfg(feature = "moka")]
BackendType::Moka => write!(f, "moka"),
// ... 等等
```

- [ ] **Step 2: 添加导入并创建类型别名**

```rust
// src/backend/custom_tiered.rs 文件顶部
use crate::core::types::{BackendType as CoreBackendType, CacheLayer};
```

- [ ] **Step 3: 更新 Display 实现**

检查 `custom_tiered.rs` 中是否有自定义的 `Display` 实现需要保留。如果有特定格式化需求，可以为 `CoreBackendType` 添加扩展方法。

- [ ] **Step 4: 删除重复的 BackendType 定义**

删除 `custom_tiered.rs` 中的 BackendType 枚举定义（约行 365-401）。

- [ ] **Step 5: 验证编译**

```bash
cargo build --features full
```

- [ ] **Step 6: 提交**

```bash
git add src/backend/custom_tiered.rs
git commit -m "refactor: 删除重复的 BackendType 枚举，统一使用 core::types"
```

---

## Task 3: 统一 CacheLayer/Layer 枚举

**Files:**

- Modify: `src/backend/custom_tiered.rs:314-315`
- Modify: `src/metrics/unified.rs:462-463`
- Test: `cargo test --all-features`

- [ ] **Step 1: 查看三个定义**

```rust
// core/types.rs 行 58-64
pub enum CacheLayer {
    L1,
    L2,
}

// custom_tiered.rs 行 314-315
pub enum Layer {
    L1,
    L2,
}

// metrics/unified.rs 行 462-463 (可能是 Display 实现)
CacheLayer::L1 => write!(f, "L1"),
CacheLayer::L2 => write!(f, "L2"),
```

- [ ] **Step 2: 更新 custom_tiered.rs 使用 CacheLayer**

```rust
// 删除 Layer 枚举定义
// 在所有使用 Layer 的地方改用 crate::core::types::CacheLayer
```

- [ ] **Step 3: 更新 metrics/unified.rs**

确保使用 `core::types::CacheLayer`。

- [ ] **Step 4: 验证编译**

```bash
cargo build --all-features
```

- [ ] **Step 5: 提交**

```bash
git add src/backend/custom_tiered.rs src/metrics/unified.rs
git commit -m "refactor: 统一 CacheLayer/Layer 枚举定义"
```

---

## Task 4: 更新 builder/cache_builder.rs 中的字符串匹配为枚举

**Files:**

- Modify: `src/builder/cache_builder.rs:623-627,665-669,729-733,759-763`
- Test: `cargo test builder --features full`

- [ ] **Step 1: 查看当前字符串匹配**

```rust
// 行 623-627
let mode_str = l2_opts.get("mode").and_then(|v| v.as_str()).unwrap_or("standalone");
match mode_str {
    "cluster" => crate::backend::client::RedisMode::Cluster,
    "sentinel" => crate::backend::client::RedisMode::Sentinel,
    _ => crate::backend::client::RedisMode::Standalone,
}
```

- [ ] **Step 2: 使用枚举的 FromStr 实现**

为 `RedisModeType` 添加 `FromStr` trait 实现（如果不存在）：

```rust
// src/core/types.rs 添加
impl std::str::FromStr for RedisModeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standalone" => Ok(Self::Standalone),
            "sentinel" => Ok(Self::Sentinel),
            "cluster" => Ok(Self::Cluster),
            _ => Err(format!("Unknown Redis mode: {}", s)),
        }
    }
}
```

- [ ] **Step 3: 重构匹配逻辑**

```rust
// 替换行 623-627 为：
let mode = l2_opts
    .get("mode")
    .and_then(|v| v.as_str())
    .map(|s| s.parse::<RedisModeType>().unwrap_or_default())
    .unwrap_or_default();
```

- [ ] **Step 4: 对所有 4 处重复模式应用相同修改**

位置：行 623-627, 665-669, 729-733, 759-763

- [ ] **Step 5: 验证测试**

```bash
cargo test builder --features full -- --nocapture
```

- [ ] **Step 6: 提交**

```bash
git add src/core/types.rs src/builder/cache_builder.rs
git commit -m "refactor: 使用 RedisModeType 枚举替代字符串匹配"
```

---

## Task 5: 最终验证

- [ ] **Step 1: 运行完整测试**

```bash
cargo test --all-features
```

- [ ] **Step 2: 运行 clippy**

```bash
cargo clippy --all-features -- -D warnings
```

- [ ] **Step 3: 检查是否有遗漏的重复定义**

```bash
grep -rn "enum RedisMode" src/
grep -rn "enum BackendType" src/
grep -rn "enum CacheLayer" src/
grep -rn "enum Layer" src/
```

Expected: 每个枚举只应有一个定义

---

## 完成标准

- [ ] `RedisModeType` 只在 `core/types.rs` 定义
- [ ] `BackendType` 只在 `core/types.rs` 定义
- [ ] `CacheLayer` 只在 `core/types.rs` 定义
- [ ] 所有引用点已更新使用 `core::types` 导出
- [ ] 所有测试通过
- [ ] clippy 无警告
