# 🔎 ws-R14 治理复核 — oxcache（FEATURE_AUDIT_REPORT.md §9.2 第 2 项）

> 复核日期：2026-09-30 ｜ 版本基线：0.5.0-rc.7 ｜ 复核人：实现工程师（复核中无预置结论，逐项实证后落判）
>
> 上游条目：`base/FEATURE_AUDIT_REPORT.md` §9.2 遗留跟进清单第 2 项：
> 「oxcache：redis-only / bare-kit 预存编译失败（backend 模块无条件导入
> moka/dashmap/serde）；`core`/`full` 语义重整；basic_ops 序列化双分支
> Noop 化。」本记录逐分句给出结论（修复 / 过时留证），并附 cargo-hack
> 主要 feature 组合抽查结果。

## 结论总表

| 分句 | 结论 | 处置 |
|---|---|---|
| redis-only 预存编译失败 | **过时**（已由后续 feature 口径整理修复） | 留证，无需改动 |
| bare-kit 预存编译失败 | **成立**（本轮实证复现） | ✅ 已修：`trait-kit` 隐含 `memory` 基线 |
| `core`/`full` 语义重整 | **不成立**（现状口径自洽且有文档） | 留证，维持现状 |
| basic_ops 序列化双分支 Noop 化 | **成立**（负分支不可达死码） | ✅ 已修：5 处负分支删除、正分支去恒真 cfg |

## 分句 1：redis-only 编译失败 — 过时

**实证**：`cargo check --no-default-features --features redis`（0.5.0-rc.7，
rustc 1.97.1）**零 error 通过**。

**归因**：审计所指「backend 模块无条件导入 moka/dashmap/serde」的形态
已被后续提交消解——`redis` 特性现隐含 `serialization`
（`redis = ["dep:redis", "dep:regex", "serialization"]`），内存后端符号按
依赖开启特性门控整合（`MemoryBackendType` 重导出门控
`any(memory, all(redis, serialization))`，`DashMapMemoryBackend`/`MokaMemoryBackend`
各自按 `offload`/`byte-weight` 进一步门控，见 `src/lib.rs` 重导出区），
backend 模块内部的 moka/dashmap 使用随 optional 依赖节点存在与否编译，
redis-only 组合不再触达缺失符号。

## 分句 2：bare-kit 编译失败 — 成立，已修

**实证（修复前）**：`cargo check --no-default-features --features trait-kit`
报 3 处 `E0432: unresolved import crate::backend`，位点：

- `src/integrations/kit/decorator.rs:14`
- `src/integrations/kit/module.rs:162`
- `src/integrations/kit/shutdown.rs:15`

**根因**：`integrations` 模块（`trait-kit` 特性门控）装饰/包装
`CacheBackend`，硬引用 `crate::backend`；而 backend 模块门控
`any(memory, redis, disk)`，`trait-kit = ["dep:trait-kit", "dep:futures"]`
不含任何 backend 基线 → 单开必炸。

**修复**：`trait-kit = ["dep:trait-kit", "dep:futures", "memory"]`——kit
集成的用途即包装一个可用的 Cache，无 backend 基线时集成本身无意义；
隐含 `memory` 与既有同型特性口径一致（`degradation`/`adaptive-ttl`/
`config-confers` 均隐含 memory，Cargo.toml 注释已留痕）。`kit` 别名继承。

**实证（修复后）**：`--features trait-kit` 与 `--features kit` 两组合
check 均零 error。

## 分句 3：`core`/`full` 语义重整 — 不成立，维持现状

**论据**：

1. `full` 的成员口径三处文档一致且明确声明（README「特性标志」、
   `docs/ARCHITECTURE.md`「特性标志」节、`docs/API_REFERENCE.md`
   「分层特性集」），并显式注明「`bloom`、`kit` 等选择加入特性不包含在
   `full` 中，精确成员见 Cargo.toml」——不存在文档与定义漂移；
2. 「重整」隐含的期待（full 收纳更多特性）与既定设计方向相反：
   full 定位是「常用后端与缓存能力的开箱组合」，`bloom`/`kit` 系审计
   既定的显式选择加入项；rc.7 新增的 `warmup`/`inklog`/`adaptive-ttl`
   等延续同一口径（默认关、按需显式启用），纳入 full 反而违背
   「默认 feature 集行为零变化」约束（spec Constraints）；
3. `core = ["minimal", "redis"]` 语义（L1+L2 最小组配）与名称相符。

结论：无裂缝，重整不必要；如未来调整 full 成员，属产品决策而非缺陷修复。

## 分句 4：basic_ops 序列化双分支 — 成立，已修

**论证（负分支不可达）**：双分支形态
`#[cfg(any(feature = "serialization", feature = "full"))] { 正路径 }`
`#[cfg(not(...))] { Err("Serialization feature is required...") }` 出现于
cache API 层；而 cache 模块门控为 `any(memory, redis, disk)`，三者均隐含
`serialization`（`memory`/`redis` 显式隐含，`disk = ["memory", ...]` 传递
隐含）⇒ **凡本文件参与编译的组合 serialization 必开** ⇒ 负分支为不可达
死码，正分支 cfg 恒真。

**修复**（5 处位点）：

- `src/cache/api/batch_ops.rs`：`set_many` / `set_many_with_ttl` / `get_many`
  三处负分支删除、正分支块展开、`Arc` 导入去 cfg（文件头留一行口径注释）；
- `src/cache/api/basic_ops.rs`：`set`（async）与 `set_sync` 两处同型处理。

**行为面核验**：负分支无任何测试引用（全仓 grep 证实）；删除后默认与
`--features full,serde-bincode,postcard` 全量 lib 测试绿（1242 通过），
无可达行为变化。

## cargo-hack 主要 feature 组合抽查（0.6.45）

### 单特性全矩阵（`cargo hack check --each-feature --no-dev-deps`）

首轮抽查即暴露 **4 个单开编译裂缝**（与 §9.2 审计所指同类——特性无
backend/infra 基线），逐个修复后全矩阵绿：

| 裂缝特性 | 症状 | 修复 |
|---|---|---|
| `batch` | `unresolved import crate::backend`（BatchWriter 包装 `Arc<dyn CacheBackend>`） | `batch = ["memory"]` |
| `compression` | `crate::backend` + `crate::infra` 缺失（装饰器 + 序列化面） | `compression = [..., "memory"]` |
| `encrypt` | `crate::backend` 缺失（加密装饰器） | `encrypt = [..., "memory"]` |
| `integrity` | `encryption` 模块不存在（integrity 模块寄生其中） | `integrity = ["encrypt", ...]` |

修复后 `--each-feature` 全矩阵（30+ 特性含别名）**零失败**。

### 组合抽查（14 组，全绿）

```text
OK full,adaptive-ttl,warmup,inklog,audit      OK redis,warmup
OK disk,adaptive-ttl                          OK telemetry,audit,inklog
OK serde-bincode,redis,compression            OK batch,offload,redis
OK stale,offload,telemetry                    OK encrypt,integrity,redis
OK invalidation,pubsub,redis                  OK full,bloom,kit,warmup,adaptive-ttl,inklog
OK macros,redis,metrics                       OK config-confers,degradation
OK byte-weight,offload                        OK hotkey,metrics,audit
```

组合设计覆盖：本轮新增特性（warmup/inklog）单开与入列 full、装饰器族
（compression/encrypt/integrity/degradation/adaptive-ttl）、存储后端三态
（memory/redis/disk）、序列化格式族（serde-bincode/postcard 经 full 组合）、
观测族（telemetry/metrics/hotkey/audit）、锁与失效族（lock/invalidation/pubsub）。

### 语义约束核验

- `--no-default-features` 单独：零 error（无任何缓存面，符合最小化承诺）；
- 默认（minimal）全量 lib 测试 925 通过——隐含基线调整（batch/compression/
  encrypt/integrity/trait-kit 均为默认关的选择加入特性）对默认行为零影响。

## 文档同步

- `docs/API_REFERENCE.md`「特性依赖」表补全本轮修复与既有隐含口径全量行；
- `Cargo.toml` 各修复特性处留口径注释（与 degradation 同口径的表述统一）。
