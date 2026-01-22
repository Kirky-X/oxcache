# Oxcache 测试组织指南

## 概述

本文档描述了 Oxcache 项目的测试文件组织结构和分类标准。

## 测试目录结构

```
tests/
├── common/                   # 公共测试工具
│   ├── mod.rs                # 模块入口
│   ├── redis_test_utils.rs   # Redis 测试工具
│   └── database_test_utils.rs # 数据库测试工具
├── integration/              # 集成测试
│   ├── batch_write_test.rs           # 批量写入测试
│   ├── chaos_test.rs                 # 混沌测试框架
│   ├── comprehensive_test.rs         # 综合功能测试
│   ├── degradation_tests.rs          # 缓存降级测试
│   ├── invalidation_test.rs          # 缓存失效测试
│   ├── lifecycle_test.rs             # 生命周期测试
│   ├── lock_warmup_test.rs           # 分布式锁和预热测试
│   ├── metrics_test.rs               # 指标收集测试
│   ├── manual_control_test.rs        # 手动控制测试
│   ├── partitioning_tests.rs         # 数据库分区测试
│   ├── performance_test.rs           # 性能测试
│   ├── recovery_test.rs              # 故障恢复测试
│   ├── redis_native_test.rs          # Redis 原生操作测试
│   ├── redis_test.rs                 # Redis 集成测试
│   ├── sea_orm_sqlite_tests.rs       # SeaORM 测试
│   ├── security_test.rs              # 安全测试
│   ├── single_flight_test.rs         # 单次请求去重测试
│   ├── sqlite_partition_tests.rs     # SQLite 分区测试
│   ├── tiered_cache_test.rs          # 分层缓存测试
│   ├── ttl_control_test.rs           # TTL 控制测试
│   ├── two_level_test.rs             # 双层缓存测试
│   ├── version_test.rs               # 版本兼容性测试
│   └── wal_test.rs                   # WAL 测试
├── e2e/                      # 端到端测试
│   └── macro_test.rs         # 宏端到端测试
├── chaos/                    # 混沌测试
│   └── random_failure_test.rs # 随机故障测试
├── integration.rs            # 集成测试入口（模块声明）
├── config_test.rs            # 配置测试
├── cross_database_integration_tests.rs # 跨数据库集成测试
├── database_partitioning_tests.rs      # 数据库分区测试
├── debug_mysql_test.rs        # MySQL 调试测试
├── feature_combinations.rs    # 特性组合测试
├── feature_gated_init_test.rs # 特性门控初始化测试
├── l2_backend_test.rs         # L2 后端测试
├── layer_test.rs              # 分层测试
├── memory_leak_test.rs        # 内存泄漏测试
├── memory_tests.rs            # 内存测试
├── miri_memory_test.rs        # Miri 内存安全测试
├── mock_l2_backend_test.rs    # Mock L2 后端测试
├── redis_test_utils.rs        # Redis 测试工具
├── redis_version_compatibility_test.rs # Redis 版本兼容性测试
├── security_tests.rs          # 安全测试
├── sqlite_connection_test.rs  # SQLite 连接测试
└── test_utils.rs              # 测试工具
```

## 测试分类

### 1. 缓存核心功能测试 (cache, client, backend)

- `tests/integration/two_level_test.rs` - 双层缓存流程测试
- `tests/integration/tiered_cache_test.rs` - 分层缓存细粒度控制测试
- `tests/integration/redis_test.rs` - Redis 模式测试
- `tests/integration/manual_control_test.rs` - 手动 L1/L2 控制
- `tests/l2_backend_test.rs` - L2 后端通用测试
- `tests/mock_l2_backend_test.rs` - Mock L2 后端测试

### 2. 配置管理测试 (config)

- `tests/config_test.rs` - TOML 配置加载、手动配置创建、TTL 验证

### 3. 数据库集成测试 (database)

- `tests/integration/partitioning_tests.rs` - PostgreSQL/MySQL/SQLite 分区
- `tests/integration/sqlite_partition_tests.rs` - SQLite 分区详细测试
- `tests/integration/sea_orm_sqlite_tests.rs` - SeaORM 集成测试
- `tests/database_partitioning_tests.rs` - 数据库分区测试
- `tests/cross_database_integration_tests.rs` - 跨数据库集成测试

### 4. 序列化功能测试 (serialization)

- `tests/unit/serialization_test.rs` - JSON 序列化器基本功能

### 5. 同步机制测试 (sync)

- `tests/integration/invalidation_test.rs` - 缓存失效机制
- `tests/integration/batch_write_test.rs` - 批量写入同步
- `tests/integration/lock_warmup_test.rs` - 分布式锁和缓存预热

### 6. 安全性测试 (security)

- `tests/integration/security_test.rs` - Lua 脚本注入防护、SCAN ReDoS 防护
- `tests/security_tests.rs` - 安全测试套件

### 7. 故障恢复测试 (recovery)

- `tests/integration/recovery_test.rs` - 故障恢复测试
- `tests/integration/wal_test.rs` - WAL 测试

### 8. 性能相关测试 (benchmarks)

- `tests/integration/performance_test.rs` - 性能测试
- `tests/memory_tests.rs` - 内存测试
- `tests/memory_leak_test.rs` - 内存泄漏测试

## 运行测试

### 运行所有测试

```bash
cargo test
```

### 运行集成测试

```bash
cargo test --test integration
```

### 运行配置测试

```bash
cargo test --test config_test
```

### 运行特定测试

```bash
cargo test --test integration -- test_two_level_cache_flow
```

## 添加新测试

### 新增集成测试

1. 在 `tests/integration/` 目录创建新文件
2. 文件开头格式：

```rust
#![allow(deprecated)]
//! 测试模块描述
//!
//! 详细说明...

use crate::common::{is_redis_available, setup_logging};

#[tokio::test]
async fn test_new_feature() {
    // 测试代码
}
```

3. 在 `tests/integration.rs` 中添加模块声明：

```rust
#[path = "integration/your_new_test.rs"]
mod your_new_test;
```

### 新增单元测试

1. 在 `tests/unit/` 目录创建新文件（或在现有文件中添加）
2. 使用标准 Rust 测试格式

## 注意事项

- 测试文件使用 `#[path = "..."]` 属性通过 `tests/integration.rs` 组织
- 使用 `use crate::common::*` 导入公共测试工具
- 使用 `#[allow(deprecated)]` 允许使用已弃用的 API
- 记得在测试中使用 `is_redis_available()` 检查 Redis 是否可用
