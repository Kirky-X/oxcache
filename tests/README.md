# Oxcache Test Suite Structure

This document describes the organization of the test suite for the Oxcache project.

## Directory Structure

```
tests/
├── common/                              # 共享测试工具
│   ├── mod.rs                           # 模块导出
│   ├── docker_test_utils.rs             # Docker 测试工具
│   ├── mock_backend.rs                  # Mock 后端
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
cargo test --features full --lib                    # 库单元测试 (1000+)
cargo test --features full --test unit              # 单元测试 (325)
cargo test --features full --test integration       # 集成测试 (133)
cargo test --features full --test e2e               # 端到端测试 (74)
cargo test --features full --test macros            # 宏测试 (10)
cargo test --features full --test feature_test      # Feature 门控测试 (2)
cargo test --features "full,bloom" --test bloom_filter_integration  # Bloom filter (9)
```

### Minimal Feature
```bash
cargo test --features minimal
```

### Skip Network Tests
```bash
cargo test --features full -- --skip redis
```

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
| `compression` | 压缩支持 (flate2) |
| `lua` | Lua 脚本支持 |
| `batch` | 批量写入 |
| `lock` | 分布式锁 |
| `dragonfly` | Dragonfly 后端 |
| `aerospike` | Aerospike 后端 |
