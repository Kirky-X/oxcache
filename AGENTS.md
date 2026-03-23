# PROJECT KNOWLEDGE BASE

**Generated:** 2026-03-06
**Commit:** 13ccb93
**Branch:** main

## Language Rules

- Always respond in Chinese (中文)
- 代码注释使用中文
- 文档支持中英双语

---

## PROJECT OVERVIEW

oxcache 是一个 Rust 实现的高性能多层缓存库 (v0.2.0)，提供 L1 内存缓存 (Moka/DashMap) 和 L2 分布式缓存 (Redis) 的两级缓存解决方案。

---

## STRUCTURE

```
oxcache/
├── src/
│   ├── lib.rs            # 入口点，导出所有模块和宏
│   ├── core/             # 核心基础模块 (constants, types, features, events)
│   ├── cache/            # 核心缓存模块 (Cache, UnifiedCache, ChainCache)
│   ├── builder/          # CacheBuilder, BackendBuilder
│   ├── backend/          # L1/L2 后端实现
│   ├── traits/           # CacheKey, Cacheable traits
│   ├── config/           # Confers 配置
│   ├── serialization/    # 序列化/压缩
│   ├── database/         # SQLite/MySQL/PostgreSQL 集成
│   ├── security/         # 安全验证模块
│   ├── testing/          # 测试辅助模块 (MockBackend)
│   ├── metrics/          # 指标收集
│   ├── recovery/         # WAL 恢复
│   └── smart_strategy/   # 智能策略
├── tests/                # 测试套件 (unit/integration/e2e/chaos)
├── examples/             # 示例代码
├── macros/               # 内部宏 crate
└── Cargo.toml           # 特性配置
```

---

## WHERE TO LOOK

| 任务 | 位置 |
|------|------|
| 核心缓存 API | `src/cache/` |
| 缓存构建器 | `src/builder/` |
| 基础类型/常量 | `src/core/` |
| 安全验证 | `src/security/` |
| 配置模块 (confers) | `src/config/confers_config.rs` |
| L1 后端 (Moka/DashMap) | `src/backend/client/` |
| L2 后端 (Redis) | `src/backend/client/redis/` |
| 特性配置 | `Cargo.toml` |
| 测试 | `tests/unit/`, `tests/integration/` |

---

## CONVENTIONS

### 特性系统

- **minimal**: 仅 L1 内存缓存，无 Redis
- **core**: L1 + L2 基础
- **full**: 全部功能 (默认)
- **confers**: 使用 confers 库进行配置管理

特性依赖通过 `check_feature_dependence!` 宏校验。

### 配置系统

使用 confers 库进行配置管理，支持以下特性：
- **文件加载**: TOML/JSON 配置文件
- **环境变量**: 支持环境变量覆盖
- **验证**: 使用 garde 进行字段验证
- **默认值**: 通过 `#[config(default = ...)]` 属性设置

### 异步 API

```rust
let cache: Cache<String, User> = Cache::builder().build().await?;
cache.set(&key, &value).await?;
```

### 空实现模式

禁用的特性使用宏生成空实现：`empty_struct!`, `empty_async_methods!`

---

## ANTI-PATTERNS

- **禁止** 跳过特性依赖检查
- **禁止** 非 async 上下文使用异步方法
- **禁止** 忽略序列化兼容性
- L2 依赖 L1，需同时启用 `moka`
- `wal-recovery` 需要 `database` + `redis`

---

## BUILD & TEST

```bash
# 构建
cargo build

# 测试
cargo test
```
