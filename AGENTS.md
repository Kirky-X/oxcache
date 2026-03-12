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
│   ├── cache.rs          # Cache<T> 主实现
│   ├── cache_interface.rs # 统一缓存接口 UnifiedCache
│   ├── builder/         # CacheBuilder, BackendBuilder
│   ├── backend/         # L1/L2 后端实现 (11 .rs)
│   ├── traits/          # CacheKey, Cacheable traits
│   ├── config/          # Confers 配置
│   ├── serialization/   # 序列化/压缩
│   ├── database/        # SQLite 集成
│   ├── recovery/        # WAL 恢复
│   ├── metrics/         # 指标收集
│   └── smart_strategy/  # 智能策略
├── tests/               # 测试套件 (unit/integration/e2e/chaos)
├── examples/            # 示例代码
├── macros/              # 内部宏 crate
└── Cargo.toml          # 特性配置
```

---

## WHERE TO LOOK

| 任务 | 位置 |
|------|------|
| 核心缓存 API | `src/cache.rs`, `src/cache_interface.rs` |
| 缓存构建器 | `src/builder/` |
| L1 后端 (Moka/DashMap) | `src/backend/client/` |
| L2 后端 (Redis) | `src/backend/redis/` |
| 特性配置 | `Cargo.toml` |
| 测试 | `tests/unit/`, `tests/integration/` |

---

## CONVENTIONS

### 特性系统

- **minimal**: 仅 L1 内存缓存，无 Redis
- **core**: L1 + L2 基础
- **full**: 全部功能 (默认)

特性依赖通过 `check_feature_dependence!` 宏校验。

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
