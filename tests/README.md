# Oxcache Test Suite Structure

This document describes the organization of the test suite for the Oxcache project.

## Directory Structure

```
tests/
├── common/                              # 共享测试工具
│   ├── mod.rs                           # 模块导出
│   ├── docker_test_utils.rs             # Docker 测试工具
│   ├── mock_backend.rs                  # Mock 后端（unit 专用测试替身，e2e/集成/chaos 禁用）
│   ├── redis_test_utils.rs              # Redis 测试工具
│   └── test_containers.rs               # Testcontainers 封装
│
├── e2e.rs                               # E2E 测试入口
├── e2e/
│   ├── advanced_scenarios_test.rs       # 高级场景（降级、并发、TTL 覆盖）
│   ├── cache_e2e_test.rs                # 基础 Cache 操作 E2E
│   ├── macro_test.rs                    # #[cached] 宏 E2E
│   └── real_world_scenario_test.rs      # 真实业务场景 E2E
│
├── integration.rs                       # 集成测试入口
├── integration/
│   ├── batch_write_test.rs              # 批量写入
│   ├── chain_cache_integration_test.rs  # 链式缓存
│   ├── comprehensive_test.rs            # 综合集成测试
│   ├── degradation_tests.rs             # 降级策略与健康检查
│   ├── invalidation_test.rs             # 缓存失效
│   ├── recovery_test.rs                 # 故障恢复
│   ├── sync_api_test.rs                 # Sync API (Moka/DashMap/Redis)
│   ├── two_level_test.rs                # 双层缓存
│   ├── version_test.rs                  # 版本管理
│   ├── redis/                           # Redis & 锁测试
│   │   ├── redis_client_comprehensive_test.rs  # Redis 客户端综合
│   │   ├── redis_cluster_test.rs        # Redis Cluster
│   │   ├── redis_sentinel_test.rs       # Redis Sentinel
│   │   ├── redis_version_compatibility_test.rs # Redis 版本兼容
│   │   ├── dist_lock_test.rs            # 分布式锁
│   │   └── lock_warmup_test.rs          # 锁与预热
│   ├── ttl/                             # TTL 测试
│   │   ├── ttl_expire_test.rs           # TTL 过期
│   │   └── ttl_consistency_test.rs      # TTL 一致性回归
│   └── backend/                         # 替代后端测试
│       ├── aerospike_test.rs            # Aerospike
│       ├── dragonfly_test.rs            # Dragonfly
│       ├── l2_backend_test.rs           # L2 后端 (Redis)
│       └── valkey_test.rs               # Valkey
│
├── unit.rs                              # 单元测试入口
├── unit/
│   ├── backend_interface_test.rs        # 后端接口
│   ├── cache_builder_test.rs            # CacheBuilder
│   ├── cache_test.rs                    # Cache 核心
│   ├── dashmap_backend_test.rs          # DashMap 后端
│   ├── depth_limited_test.rs            # 深度限制
│   ├── error_test.rs                    # 错误类型
│   ├── layer_test.rs                    # 缓存层
│   ├── metrics_test.rs                  # 指标收集
│   ├── mock_backend_test.rs             # Mock 后端
│   ├── moka_backend_test.rs             # Moka 后端
│   ├── penetration_guard_test.rs        # 穿透防护
│   ├── redis_client_test.rs             # Redis 客户端
│   ├── serialization_test.rs            # 序列化
│   ├── traits_test.rs                   # Trait 实现
│   ├── utils_redaction_test.rs          # 日志脱敏
│   └── utils_security_log_test.rs       # 安全日志
│
├── macros.rs                            # 宏测试入口
├── macros/
│   ├── advanced_params_test.rs          # 宏高级参数（single_flight / strict / condition / skip）
│   ├── skip_cache_none_test.rs          # cache_none 模式宏测试
│   ├── skip_cache_write_test.rs         # skip_cache_write 宏测试
│   ├── sync_test.rs                     # sync 模式宏测试
│   └── compile_fail/                    # trybuild 编译失败测试
│       ├── invalid_arg.rs / .stderr
│       └── sync_with_async_fn.rs / .stderr
│
├── feature_test.rs                      # Feature 门控测试
├── bloom_filter_integration.rs          # Bloom filter 集成测试 (feature = "bloom")
│
├── chaos.rs                             # 混沌测试入口
├── chaos/
│   ├── chaos_test.rs                    # 混沌工程测试
│   ├── backend_failure_test.rs          # 后端故障注入（FailingBackend，自 e2e 下沉）
│   ├── network_failure_test.rs          # 网络故障模拟
│   └── random_failure_test.rs           # 随机故障模拟
│
├── security.rs                          # 安全测试入口
├── security/
│   ├── security_coverage_test.rs        # 安全覆盖测试
│   └── security_tests.rs               # 安全验证测试
│
├── performance.rs                       # 性能测试入口
├── performance/
│   ├── memory_leak_test.rs              # 内存泄漏检测
│   ├── memory_tests.rs                  # 内存使用测试
│   ├── miri_memory_test.rs              # Miri 内存安全
│   ├── performance_test.rs              # 性能基准
│   └── pipeline_performance_test.rs     # Pipeline 性能
│
└── real_env/                            # 真实环境配置
    ├── docker-compose.yml               # Redis 主从
    ├── docker-compose.cluster.yml       # Redis Cluster
    ├── docker-compose.sentinel.yml      # Redis Sentinel
    └── configs/                         # Redis 配置文件
```

## Running Tests

### All Tests
```bash
cargo test --features full
```

### By Test Binary
```bash
cargo test --features full --lib                    # 库单元测试 (1537)
cargo test --features full --test unit              # 单元测试 (332)
cargo test --features full --test integration       # 集成测试 (139)
cargo test --features full --test e2e               # 端到端测试 (65)
cargo test --features full --test macros            # 宏测试 (21)
cargo test --features full --test feature_test      # Feature 门控测试 (2)
cargo test --features "full,bloom" --test bloom_filter_integration  # Bloom filter (7)
```

> 括号内为 `#[test]` / `#[tokio::test]` 函数的 grep 文本计数（与
> [README](../README.md#-测试) 主表同口径，截至 0.5.0-rc.7）；feature 门控与
> `--ignored` 用例不计入运行数，实际以 `-- --list` 为准。

### Minimal Feature
```bash
cargo test --features minimal
```

### Skip Network Tests
```bash
cargo test --features full -- --skip redis
```

## 容器可用性门控

valkey/dragonfly 集成测试经 `common/test_containers.rs` 的 `container_or_skip`
门控容器启动（90 秒预算覆盖镜像拉取与创建，失败后同模块副本内短路后续探测）：

- **本地默认**：Docker/镜像不可用时打印 `[TEST-SKIP] ... (gate skip #N)` 并跳过
  （结果计入 passed，需 `--nocapture` 查看跳过原因）；
- **CI（fail-closed）**：ci.yml 与 release.yml 的 test step 置位
  `OXCACHE_TEST_STRICT=1`，容器不可用时跳过转为 panic 失败，杜绝静默失去覆盖；
- **与 `OXCACHE_SKIP_REDIS_TESTS` 的分工**：后者显式跳过基于 Redis URL 探测的
  redis 系测试（跳过即预期行为）；`OXCACHE_TEST_STRICT` 是反向开关，要求容器
  必须可用，把「环境不可用」从静默跳过升级为失败；
- **两档跳过语义**：容器不可用走 `gate_skip`（置失败短路闩，`first failure`
  仅记录基础设施类原因）；容器就绪后的后端连接/健康检查瞬时失败走
  `backend_skip`（可见带计数，但不置闩、不占首因槽），避免一次握手抖动
  放大为整组覆盖丢失。`OXCACHE_TEST_STRICT` 置位时两档跳过均转 panic
  （含后端层瞬时失败，CI 抖动以重跑吸收）；
- 已知残留：90 秒超时取消启动流程的窄窗口可能在 daemon 侧留下无管理者容器
  （testcontainers `watchdog` feature 已兜底信号退出），长寿命开发机可偶发
  `docker system prune`。

## Feature Flags

| Feature | Description |
|---------|-------------|
| `minimal` | 仅 L1 内存缓存 (默认) |
| `full` | 全部功能 |
| `bloom` | Bloom filter 后端 |
| `memory` | 内存后端 (Moka/DashMap) |
| `redis` | Redis 后端 |
| `macros` | `#[cached]` 过程宏 |
| `serialization` | 序列化支持 |
| `compression` | 压缩支持 (flate2 + zstd) |
| `lua` | Lua 脚本支持 |
| `batch` | 批量写入 |
| `lock` | 分布式锁 |
| `dragonfly` | Dragonfly 后端 |
| `aerospike` | Aerospike 后端 |

## 目标级 feature 门控

集成测试目标按其内容依赖的特性做 crate 级门控（`#![cfg(...)]`），feature
不满足时整目标置空而非编译失败：

| 目标 | 门控 | 原因 |
|------|------|------|
| `bloom_filter_integration` | `all(bloom, memory)` | 被 Moka 后端接点依赖（装饰器包装 `MokaMemoryBackend`）；bloom 单开不带 backend |

> 偏差标注：蓝图 5.2 对 `bloom_filter_integration` 有「不改一行」的字面约束。
> 本门控为编译层置空（测试体与断言零改动，bloom+memory 下 7 passed），
> 系 2d2793c 修复 bloom 单开 test 目标编译失败的必要伴随，待蓝图 owner
> 追认「不改一行」的意图边界（禁断言弱化 vs 字面禁改）。
