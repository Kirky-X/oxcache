# oxcache 测试场景矩阵（TEST_SCENARIOS）

> 阶段 2 验收产物：功能域 → 测试落点地图 + E2E 缺口补盲记录。
> 执行口径：一律使用项目自身测试框架 `cargo test`（禁止独立脚本替代）；
> 集成/E2E 依赖 Docker 真实容器（testcontainers 自起自清 / `tests/real_env/`
> compose 集群），禁用 test double（mockall 等），进程内真实实现与故障注入
> 替身（chaos 式 `FailingBackend`）为项目既有口径；断言按真实行为固化，
> 文档推测与实现不符时修正并注明依据。

## 1. 测试金字塔基线（all-features 实测）

| 层 | 入口 | 结果 |
|---|---|---|
| 单元 | `cargo test --lib` | 1132 passed / 0 failed / 120 ignored（ignored 补盲见 §4） |
| 集成 | `tests/unit.rs` | 327 passed |
| 集成 | `tests/integration.rs` | 137 passed + 4 --ignored 补盲全过（valkey 首轮 1 失败为镜像拉取瞬态，复验 8/8 过） |
| E2E | `tests/e2e.rs` | 75 passed（72 既有 + 本阶段新增 3，见 §3） |
| 安全 | `tests/security.rs` | 14 passed |
| 宏 | `tests/macros.rs`（trybuild） | 10 passed（51.4s 含编译失败快照校验） |
| 特性 | `tests/feature_test.rs` | 2 passed |
| 混沌 | `tests/chaos/backend_failure_test.rs` | 19 passed |
| 布隆 | `tests/bloom_filter_integration_test.rs` | 9 passed |
| 性能 | `tests/performance.rs` | 19 passed + 5 --ignored 补盲全过 |

examples：37/37 运行 rc=0（`error_handling`/`custom_backend`/`events` 三例输出为
设计性错误演示，符合附录 A 口径）。

## 2. 功能域 → 落点矩阵

| 域 | 场景要点 | 既有落点 | 缺口处置 |
|---|---|---|---|
| 内存后端 moka/dashmap | 读写/过期/TTL/TTI/容量 | lib 内联 + `tests/unit.rs` + e2e advanced/cache | 无缺口 |
| Redis 后端 | 连接/读写/管道/重连 | `tests/integration/redis/redis_client_comprehensive_test.rs` | 无缺口 |
| Redis 集群 | 6 节点建群/分片/failover | `tests/integration/redis/redis_cluster_test.rs` + real_env compose | 无缺口 |
| Redis 哨兵 | 主从切换/发现 | `tests/integration/redis/redis_sentinel_test.rs` + real_env compose | 无缺口 |
| Dragonfly | 兼容模式/操作面 | `tests/integration/backend/dragonfly_test.rs`（8 测试） | 无缺口 |
| Valkey | 显式模式/透明复用 | `tests/integration/backend/valkey_test.rs`（8 测试） | 无缺口 |
| Aerospike | 容器自管+access-address 注入+全操作面 | `tests/integration/backend/aerospike_test.rs`（7 测试） | lib 内联 11 个 --ignored 为同路径冗余且 CI 从不跑，记录为环境边界（§4.4） |
| 链式缓存 | 排序/回填/竞速读/部分失败 | `tests/integration/chain_cache_integration_test.rs` | 无缺口 |
| 分布式锁 | 重入/TTL/争用/续期/释放 | `tests/integration/redis/dist_lock_test.rs`（§4.1 注）+ **新增** `tests/e2e/dist_lock_watchdog_e2e.rs` | 看门狗组合语义缺口 → OXL-WD-01/02/03 |
| 事件系统 | 事件构建/发布订阅/错误事件 | `src/core/events.rs` 21 内联（组件级）+ **新增** `tests/e2e/events_chain_e2e.rs` | publisher→失败→订阅方集成链缺口 → OXE-01/02/03 |
| Lua 脚本 | 脚本加载/EVAL/原子性 | lib 内联（8 --ignored 补盲全过） | 无缺口 |
| 序列化 | JSON/bincode/typed API | lib 内联 + `tests/unit.rs` | 无缺口 |
| 批量操作 | mget/mset/pipeline | lib 内联 + e2e advanced | 无缺口 |
| 指标 | 统计/Prometheus/JSON 导出 | lib 内联 + `tests/unit.rs` | 无缺口 |
| 压缩 | zstd/flate2 透明压缩 | lib 内联 | 无缺口 |
| 布隆过滤器 | 误判率/扩容/集成 | `tests/bloom_filter_integration_test.rs` | 无缺口 |
| 过程宏 | derive/属性宏/编译失败快照 | `tests/macros.rs`（trybuild） | 无缺口 |
| 安全 | 键注入/脱敏/审计日志 | `tests/security.rs` | 无缺口 |
| 混沌 | 后端故障注入/降级 | `tests/chaos/backend_failure_test.rs`（`FailingBackend`） | 无缺口 |
| kit 集成 | AsyncKit 注册/config/build/健康/生命周期 | `src/integrations/kit/` 15 内联（module 7/observer 3/decorator 3/shutdown 2） | 覆盖厚，无需 e2e |
| i18n | 多语言错误消息 | `src/i18n/mod.rs` 36 内联 | 覆盖厚，无需 e2e |
| cli | FeatureSet 布尔开关（cli_available） | lib 内联 | 无独立 e2e 必要性 |
| promotion | ChainCache backfill 概念演示 | integration 已覆盖 backfill | 无缺口 |
| feature_test | feature 门控存在性 | `tests/feature_test.rs` | 无缺口 |

测试环境协议（环境变量）：`REDIS_URL` 优先 / `OXCACHE_SKIP_REDIS_TESTS` 全跳过 /
`OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS`（非 TLS 门禁）/
`REDIS_CLUSTER_AVAILABLE` / `REDIS_SENTINEL_AVAILABLE` / `REDIS_SENTINEL_MASTER_URL`
（默认 `redis://127.0.0.1:16379`）/ `REDIS_VERSION_TEST_ENABLED`。

## 3. 阶段 2 E2E 缺口补盲（本阶段新增落地）

### 3.1 `tests/e2e/dist_lock_watchdog_e2e.rs`（场景 ID：OXL-WD-01/02/03）

既有覆盖引用：`dist_lock_test.rs` 的 `test_dist_lock_watchdog_renew` 已覆盖单持有方
续期（ttl=300ms，sleep 500ms 后 `is_held` 仍 true），其余 4 测试均
`watchdog_enabled(false)`。本文件补看门狗**组合语义**：

- **OXL-WD-01** 续期保持互斥：A（ttl=1s，watchdog 每 ttl/3≈333ms 续期）持锁
  跨 2.5s（>2×TTL）后 B 仍被拒，错误文本含 "already held"；
- **OXL-WD-02** release 停止续期并立即转手：A release 后 B acquire 立即成功；
- **OXL-WD-03** release 后看门狗确已停止：TTL 到期后 `is_held`=false 且
  `backend.exists`=false，键无"复活"续期。

### 3.2 `tests/e2e/events_chain_e2e.rs`（场景 ID：OXE-01/02/03）

既有覆盖引用：`src/core/events.rs` 21 内联为组件级语义；`emit_backend_error`
各调用点由单元覆盖。**tests 层此前零覆盖** publisher 配置 → 真实后端失败 →
`publish_error` 到达的完整链路。以停止真实 Redis 容器注入故障（真实失败路径，
非 mock）：

- **OXE-01** set：L2 失败 → 部分成功语义（`Ok(())`）+ 事件到达；
- **OXE-02** get：L1 命中不触发 L2（对照无新事件）；L1 miss + L2 失败 →
  降级为 `Ok(None)` + 事件到达；
- **OXE-03** delete：L2 失败 → 部分成功语义 + 事件到达。

事件契约断言：三条失败路径各产生恰好一条 `publish_error`，格式
`"backend {name}: {error}"`，key 为 `Some(原始键)`。

## 4. 真实行为核正与发现（阶段 2）

1. **容器生命周期发现（重要）**：testcontainers 0.28 的 `ContainerAsync` 在
   drop 时即删除容器。`dist_lock_test.rs` 既有模式把容器锁在 `setup()` 局部
   作用域内，返回后首个连接操作必报 `broken pipe`，被既有 `ok_or_skip!` 宏
   静默吞掉——该文件断言在此环境下实际未执行（表现为"绿"）。本阶段新增
   e2e 均返回容器句柄由测试体持有至结束，规避该问题（见
   `dist_lock_watchdog_e2e.rs::setup` 文档注释）。
2. **`read_from_chain` 降级语义核正**：L1 明确 miss + L2 失败时返回
   `Ok(None)`（部分后端明确 miss 即不整体失败），而非 `Err`；`Err` 仅在
   全部后端失败时传播。OXE-02 断言按此真实行为固化。
3. **valkey 瞬态失败**：`valkey_test.rs` 首轮 1 失败为 valkey 镜像首次拉取
   竞态（`expect` 连接失败），单独复验 8/8 过；记录为 flaky 观察项，非代码缺陷。
4. **aerospike lib --ignored 环境边界**：lib 内联 11 个 --ignored 测试与
   `aerospike_test.rs`（7 个非 ignored，真实容器 + 注入 + 全操作面）为同路径
   冗余覆盖；客户端 `wait_till_stabilized` 首轮 tend nodes=0 即 break 叠加
   `fail_if_not_connected=true` 的组合在 lib 进程内无法稳定复现容器就绪，
   且 CI 从不跑 `--ignored`（`--lib` 默认范围 1132/0 达标）。记录为已知
   环境边界，非代码缺陷。

## 5. 分层特性组合矩阵（CI 口径复刻）

组合集（`.github/workflows/ci.yml`）：`minimal` / `core` / `full` 三基础组 +
10 关键组合 `minimal,macros` / `minimal,bloom` / `minimal,compression` /
`core,macros` / `core,bloom` / `core,compression` / `core,batch` / `core,lua` /
`core,cli` / `core,bloom,macros`；口径 `cargo test --features "<组合>" --workspace`
（叠加默认 minimal，与 CI 一致）。执行结果见 `reviews/acceptance-report.md` 台账。

## 6. 静态门槛

| 门槛 | 命令 | 要求 |
|---|---|---|
| 格式 | `cargo fmt --all -- --check` | 零 diff |
| Lint | `cargo clippy --all-targets --all-features --workspace -- -D warnings` | 零告警 |
| 文档 | `cargo doc --no-deps --all-features`（`-D warnings`） | 零告警 |
| 供应链 | `cargo deny check` / `cargo audit` | 无高危 |
| MSRV | `rust-version = "1.97.1"`（workspace.package） | 工具链 1.97.1 编译通过 |
