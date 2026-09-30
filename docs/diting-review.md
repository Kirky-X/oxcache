# 🔍 Diting Focused Review — oxcache（重建）

**Scope**: 近期核心模块——`tests/common/test_containers.rs` 容器可用性门控设施 + `tests/integration/backend/{valkey,dragonfly}_test.rs` 接线 + 配套 workflow/文档（R3 集成测试环境门控交付面）
**Language**: Rust
**Date**: 2026-09-27
**Review**: Focused Review（Engine A 维度 + Engine C 简化视角，聚焦复查）

> **本文档为重建**：路线图（README.md「质量审查留档项跟进」行）引用的原始
> `reviews/diting-review.md` 已不在仓内不可考——`reviews/` 目录被
> `.gitignore` 排除（`.gitignore:109`），引用必丢文件的同型失误曾发生两次
> 后，本重建版改存放于被跟踪的 `docs/`。本文按 diting 方法论对上述近期核心
> 模块重新复查生成，发现均基于本仓当前代码与本环境实测证据，非原始审查的
> 复刻。规模：3 Medium + 2 Low，与路线图记载的条目数量一致。

---

## Summary

| Dimension | Issues | Highest Severity |
|---|---|---|
| 🧹 Quality/Correctness | 2 | 🟡 Medium |
| 📚 Documentation | 1 | 🟡 Medium |
| ✨ Simplification | 2 | 🔵 Low |
| **Total** | **5** | |

**Verdict**: ✅ Approved（无 Critical/High；2 Medium + 1 Low 当轮修复，1 Medium + 1 Low 登记待办；**2026-09-30 待办修复轮已将 MED-001 与 LOW-002 收口，见文末「待办修复轮」**）

---

### Issues

#### 🟡 Medium（3）

---

**[MED-001]** `tests/**`（20+ 处，存量） — 测试线程内 `unsafe std::env::set_var` 与 libtest 默认并发构成形式上的数据竞争
**Confidence**: 90 | **Dimension**: Correctness（存量模式，非近期 diff 引入）

**Problem**: `set_allow_insecure()`（如 `valkey_test.rs:31-34`、`dragonfly_test.rs:20-24`）及 degradation_tests/redis_cluster_test/redis_sentinel_test/redis_client_comprehensive_test 等存量文件在测试体内直接 `unsafe env::set_var`。libtest 默认多线程并发运行测试，POSIX `environ` 非线程安全，标准库将 `set_var` 标记为 `unsafe` 即因此；同进程并发写环境变量属于形式 UB（实际危害受限于写入恒为同值，但读侧如 `redis_test_utils.rs` 的 env 探测与写侧并发即构成 race）。

**Remediation（✅ 已修复，2026-09-30 待办修复轮）**: 统一改为进程启动时一次性初始化——dev-deps 已有 `ctor`，在 `tests/common/mod.rs` 加 `#[ctor]` 于 main 前单线程设置 `OXCACHE_ALLOW_INSECURE_REDIS`，删除全部测试内 `set_var` 点（20+ 处、跨 10+ 文件）。

---

**[MED-002]** `tests/common/test_containers.rs:37-46,66` — strict 模式下失败短路消息遮蔽首个真实根因，CI 排障需重跑
**Confidence**: 95 | **Dimension**: Quality（可观测性）

**Problem**: `gate_skip` 无条件置 `CONTAINER_GATE_DEAD` 后，短路分支消息只有 "earlier startup failure short-circuited this probe"，不含首次失败的真实原因。实测（`OXCACHE_TEST_STRICT=1` + 死 `DOCKER_HOST`）：8 个 panic 中 7 条为无根因的短路消息，含 "Socket not found" 真因的仅并行竞态窗口内抢到真实探测的少数几条；串行（`--test-threads=1`）下必然只剩 1 条真因。CI 红灯时失败原因不可从输出直接读出。

**修复（✅ 已应用，本轮）**: `gate_skip` 增设 `CONTAINER_GATE_FIRST_REASON: OnceLock<String>` 记录首次失败原因，短路消息追加 `(first failure: …)` 后缀。

---

**[MED-003]** `README.md:558,561` — 路线图两行与现实脱节：已交付项仍标「📋 待办」
**Confidence**: 95 | **Dimension**: Documentation

**Problem**: 「Valkey 集成测试环境门控」行在该能力（container_or_skip 门控 + OXCACHE_TEST_STRICT fail-closed）交付后仍标 📋 待办，说明栏还停留在「无 Docker 环境无法运行」的旧约束；「质量审查留档项跟进」行引用的留档文件当时不存在。协作者据此会重复评估已完成工作或寻找缺失文件。

**修复（✅ 已应用，本轮）**: 两行更新为 ✅ 并指向交付物（门控语义见 tests/README.md「容器可用性门控」，留档即本文档）。

---

#### 🔵 Low（2）

---

**[LOW-001]** `tests/common/test_containers.rs:68,73` — 90 秒门控预算魔数双处硬编码，时长调整时消息文本易失真
**Confidence**: 100 | **Dimension**: Simplification（一致性）

**Problem**: `tokio::time::timeout(Duration::from_secs(90), …)` 与超时消息字面量 `"timed out after 90s …"` 各写一份 90，改预算漏改消息会让观测输出说谎。

**修复（✅ 已应用，本轮）**: 提取 `CONTAINER_GATE_TIMEOUT_SECS` 常量，消息内插同一常量。

---

**[LOW-002]** `tests/common/test_containers.rs`（start_redis_container / start_valkey_container / start_dragonfly_container）+ `valkey_test.rs`/`dragonfly_test.rs` 骨架 — 三段容器便捷函数与两份测试 setup 骨架近似克隆
**Confidence**: 90 | **Dimension**: Simplification

**Problem**: 三个 `start_*_container` 便捷函数均为「start → wait_ready → 组 URL」的同构 7 行（仅镜像/类型不同）；两个 backend 测试文件的 `set_allow_insecure` + `make_*_backend` + setup 骨架亦高度相似。语义收敛已由 `container_or_skip` 完成，剩属结构重复。

**Remediation（✅ 部分修复，2026-09-30 待办修复轮）**: 容器三胞胎已用声明式宏收敛；测试骨架残余（构造包装层）经实证评估不收敛（抽象成本高于残余重复，理由见文末「待办修复轮」）。

---

## 修复状态汇总（2026-09-27 当轮）

| ID | 处置 |
|---|---|
| MED-001 | 📋 登记待办（统一 ctor 初始化，跨 10+ 文件） |
| MED-002 | ✅ 本轮修复 |
| MED-003 | ✅ 本轮修复 |
| LOW-001 | ✅ 本轮修复 |
| LOW-002 | 📋 登记待办（结构收敛，收益/风险比待评估） |

## 待办修复轮（2026-09-30）

| ID | 处置 | 证据 |
|---|---|---|
| MED-001 | ✅ 已修复：`tests/common/mod.rs` 增 `#[ctor::ctor(unsafe)]` 进程加载期一次性写入 `OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS`（单线程期写环境变量，消除并发写 `environ` 竞争）；删除 `tests/` 目录下 11 个测试文件共 31 处测试体内 `set_var` 位点与 2 个 `set_allow_insecure` 空壳 helper，`tests/` 目录内测试体内不再存在任何环境变量写点（本声明仅限定 `tests/` 目录） | `integration`/`chaos`/`e2e` 三二进制零警告编译；`--features full` 下 degradation_tests 14/14、valkey/dragonfly/redis_cluster/redis_sentinel 模块全绿（容器不可达路径走既有门控跳过） |
| LOW-002 | ✅ 部分修复：`start_redis/valkey/dragonfly_container` 三胞胎以 `container_start_fn!` 声明式宏收敛（`tests/common/test_containers.rs`，`start → wait_ready → 组 URL` 单点维护）。**残余不修理由**：两份 backend 测试骨架在 MED-001 修复后仅剩 `make_*_backend` 构造包装层——valkey（2 函数，`RedisMode::ValkeyStandalone` + 透明模式双语义）与 dragonfly（1 函数，`OxCacheResult` 错误路由给门控）签名、模式与错误语义各异，收敛需为 3 个单行构造引入跨文件泛型抽象，抽象成本高于残余重复，维持原 LOW-002 裁决的收益/风险比结论 | 宏收敛后 `--features full` integration 二进制零警告编译，valkey 8/8、dragonfly 6/6 通过 |
