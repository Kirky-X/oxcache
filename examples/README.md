# Oxcache Examples

[Oxcache](https://github.com/Kirky-X/oxcache) 示例集合，演示高性能两级缓存库的各种用法。

## Quick Start

```bash
# 运行单个示例
cd examples && cargo run --example example_basic_operations

# 列出所有可用示例
cargo run --example --list

# 运行所有示例测试
cargo test --examples
```

## 学习路径

按以下顺序逐步学习 oxcache：

### 入门（从这里开始）

| 示例 | 功能 | 关键 API |
|------|------|----------|
| `example_basic_operations` | 基本 CRUD 操作 | `get`/`set`/`delete`/`exists` |
| `example_new_api` | 现代 API 入门 | `Cache::builder()`/`Cache::memory()` |
| `example_cache_builder` | CacheBuilder 配置 | `capacity`/`ttl`/`tti`/`sync_mode` |
| `example_serialization` | JSON 序列化 | `serde::Serialize`/`Deserialize` |
| `example_cache_key` | 自定义缓存键 | `CacheKey` trait |

### 核心功能

| 示例 | 功能 | 关键 API |
|------|------|----------|
| `example_get_or` | 缓存未命中时计算 | `get_or` (single-flight) |
| `example_sync_api` | 同步 API | `get_sync`/`set_sync`/`clear_sync`/`len_sync` |
| `example_byte_ops` | 字节级操作 | `get_bytes`/`set_bytes`/`len`/`capacity`/`shutdown` |
| `example_cached_macro` | `#[cached]` 宏 | `#[cached(service/ttl/key_prefix)]` |
| `example_explicit_init` | 显式初始化 | `Cache::new()`/全局缓存 |

### 进阶

| 示例 | 功能 | 关键 API |
|------|------|----------|
| `example_batch_write` | 批量操作 | `set_many`/`get_many`/`delete_many` |
| `example_chain_cache` | 链式缓存 | `ChainCache`/`ChainLink` |
| `example_invalidation` | 缓存失效策略 | TTL/TTI/手动失效 |
| `example_warmup` | 缓存预热 | 批量预加载 |
| `example_smart_strategy` | 缓存策略模式 | Cache-Aside/Lazy Loading/TTL 分层 |
| `example_cache_promotion` | 缓存提升 | L2→L1 提升/热点分析 |
| `example_error_handling` | 错误处理 | `OxCacheError`/重试/可恢复性 |
| `example_custom_backend` | 自定义后端 | `CacheReader`/`CacheWriter`/`CacheConnector` |

### Redis 相关（需要 Redis 服务）

| 示例 | 功能 | 关键 API |
|------|------|----------|
| `example_redis_native` | Redis 原生操作 | `RedisBackend` |
| `example_redis_modes` | Redis 部署模式 | Standalone/Cluster/Sentinel |
| `example_redis_pipeline` | Pipeline 批量 | `set_many_pipeline`/`get_many_pipeline` |
| `example_lua_script` | Lua 脚本执行 | `eval_lua`/`script_load`/`eval_sha` |
| `example_moka_ttl` | Moka per-entry TTL | `Expiry` trait |
| `example_dashmap_backend` | DashMap 后端 | `DashMapMemoryBackend` |

### 配置与特性

| 示例 | 功能 | 关键 API |
|------|------|----------|
| `example_dynamic_config` | 动态配置 | 运行时配置变更 |
| `example_key_generator` | Key 生成器 | `KeyGenerator` |
| `example_events` | 事件系统 | `CacheEvent`/`CacheEventType` |
| `example_metrics` | 指标导出 | `export_json_format`/`export_prometheus_format` |
| `example_compression` | 数据压缩 | `JsonSerializer::with_compression()` |
| `example_security` | 安全脱敏 | `redact_value`/`redact_connection_string` |
| `example_security_validation` | 安全验证 | `validate_redis_key`/`validate_lua_script` |
| `example_bloom_filter` | 布隆过滤器 | `BloomFilter`/`BloomFilterBackend` |
| `example_i18n` | 国际化 | `CacheI18nFormatter` |
| `example_cli_usage` | CLI 使用 | 命令行工具 |
| `example_database_integration` | 数据库集成 | Cache-Aside 模式 |
| `example_comprehensive_usage` | 综合使用 | 全部功能概览 |

## 特性依赖

示例 crate 启用了以下 oxcache 特性：

```toml
oxcache = { path = "..", features = ["full", "bloom-filter", "i18n"] }
```

- `full` — 包含所有核心功能（Redis、序列化、压缩、指标、Lua 脚本等）
- `bloom-filter` — 布隆过滤器支持
- `i18n` — ICU4X 国际化格式化

## 先决条件

- Rust 1.85+
- Redis 6.0+（Redis 相关示例需要运行中的 Redis 服务）
- 无 Redis 的示例可独立运行（内存后端）

## 目录结构

```
examples/src/
├── 01_basics/       # 入门示例（11 个）
├── 02_advanced/     # 进阶示例（14 个）
├── 03_config/       # 配置示例（2 个）
├── 05_database/     # 数据库集成（1 个）
└── 06_features/     # 特性展示（8 个）
```

## 贡献

1. Fork 仓库
2. 创建特性分支
3. 运行 `cargo fmt` 和 `cargo clippy`
4. 提交 Pull Request

## 许可证

MIT License - 详见 [LICENSE](../LICENSE)。
