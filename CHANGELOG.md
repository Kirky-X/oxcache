# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.2] - 2026-07-02

### Fixed
- `minimal` feature build failure: `security/mod.rs` unconditionally compiled `pub mod regex;` and `lazy_static!` blocks using `::regex::Regex`, but `regex` crate is only available with `redis` feature. All security submodules and functions are now gated behind `#[cfg(feature = "redis")]`.
- `ConfigResult` type alias in `error.rs` referenced `CacheConfigError` (which was already `#[cfg(feature = "redis")]` gated) without itself being gated. Now properly gated behind `#[cfg(feature = "redis")]`.
- `lib.rs` re-exported `CacheConfigError` and `ConfigResult` unconditionally. Now split: `pub use error::{CacheError, Result};` (always) + `pub use error::{CacheConfigError, ConfigResult};` (redis only).
- 69 Redis unit tests in `src/backend/memory/redis/client.rs` panicked without a live Redis server. All 69 tests now marked `#[ignore = "requires Redis server; run with: cargo test --features redis --lib -- --ignored"]` for CI isolation.
- README builder API examples referenced 6 non-existent methods (`.redis()`, `.redis_with_mode()`, `.tiered()`, `.with_backend()`, `.batch_writes()`, `.auto_promote()`). Replaced with real API: `.backend_arc()`, `.tti()`, `.sync_mode()` + notes on `RedisBackend::new()` and `ChainCache::builder()`.
- README inter-language links pointed to `../README.md` instead of `README.md` (both files in same directory).
- README security import path `use oxcache::security::{...}` → `use oxcache::{...}` (functions are re-exported at crate root).
- `lib.rs` Features list said `moka` instead of `memory`; `serialization` description listed "JSON/Bincode/MessagePack/CBOR" but only JSON is supported. Rewritten to match `Cargo.toml` exactly (tiered + core component features).
- `lib.rs` `compile_error!` message in `check_feature_dependence!` macro referenced `version = "0.1"` instead of `version = "0.3.2"`.
- `error.rs:63` doc comment typo: "络连接问题" → "网络连接问题" (missing "网" character).
- `html_root_url` updated from 0.3.1 → 0.3.2.

### Removed
- Phantom `init_config` macro documentation in `lib.rs` (lines 125-145): no implementation, no `pub use`, no `macro_export` — was misleading dead documentation.

### Added
- `docs/SECURITY.md`: comprehensive security documentation covering Redis TLS enforcement, key validation, Lua script sandbox, SCAN pattern restrictions, connection string redaction, logging security, threat model, and vulnerability reporting process.
- `.editorconfig`, `.github/CODEOWNERS`, `.github/ISSUE_TEMPLATE/` (bug/feature/question), `.github/PULL_REQUEST_TEMPLATE.md`, `.github/dependabot.yml`, `.github/workflows/codeql.yml`, `clippy.toml`, `lefthook.yml`: industrial-grade project harness from env-init.

### Changed
- Version bumped 0.3.1 → 0.3.2 in `Cargo.toml`, `macros/Cargo.toml`, and `oxcache_macros` dependency.
- README section headers: removed `(0.3.0)` version annotations from "Sync API", "Bloom Filter", and "TTL Behavior Reference" sections.

### Known Issues
- `docs/API_REFERENCE.md` is stale (documents 0.2.x API). Will be rewritten in 0.4.0.

## [0.3.1] - 2026-06-30

### Added
- `Cache<K,V>::ttl(&key) -> Result<Option<Duration>>` async method for querying per-entry remaining TTL
- `Cache<K,V>::expire(&key, ttl) -> Result<bool>` async method for updating per-entry TTL
- `Cache<K,V>::ttl_sync(&key) -> Result<Option<Duration>>` sync variant
- `Cache<K,V>::expire_sync(&key, ttl) -> Result<bool>` sync variant
- 新增回归测试 `tests/cache_ttl_expire_test.rs`（11 个测试覆盖 update-with-preserving-TTL 流程）

### Fixed
- `Cache<K,V>` 未暴露 `ttl()` / `expire()` 方法，导致下游 `set()` 更新值时丢失 per-entry TTL（`set(k, v, None)` 覆盖了原有 TTL）

## [0.3.0] - 2026-06-30

### BREAKING
- `MokaMemoryBackend::set(ttl=Some(_))` 不再静默忽略 TTL，改为真实生效（基于 `moka::Expiry` trait）
- `MokaMemoryBackend::ttl(key)` 不再永远返回 `Ok(None)`，改为返回剩余 TTL
- `MokaMemoryBackend::expire(key, ttl)` 不再永远返回 `Ok(false)`，改为真实更新并返回 `Ok(true)`
- `MockBackend::set(ttl=Some(_))` 不再忽略 TTL，改为真实生效（用 `Instant` 跟踪 + lazy 过期清理）
- `MockBackend::ttl(key)` / `expire(key, ttl)` 行为对齐 DashMap/Redis

### Added
- 新增同步 API 路径：`SyncCacheBackend` trait 层级（`SyncCacheReader` + `SyncCacheWriter` + `SyncCacheConnector`）
- 新增 `Cache<K,V>` 同步方法：`get_sync` / `set_sync` / `set_with_ttl_sync` / `delete_sync` / `exists_sync` / `get_or_sync` / `clear_sync` / `get_bytes_sync` / `set_bytes_sync`
- 新增 `CacheBuilder::sync_mode(bool)` 配置，启用后 `Cache<K,V>` 持有 `Arc<dyn SyncCacheBackend>`
- 新增 `MokaMemoryBackend` / `DashMapMemoryBackend` / `BloomFilterBackend` 的 `SyncCacheBackend` 实现
- 新增 `ChainCache` 同步 API：`from_sync_backend` 构造、`get_sync` / `set_sync` / `delete_sync`（任一链接不支持 sync 时返回 `Err(NotSupported)`）
- 新增 `#[cached(service = "...", sync)]` 宏模式，生成同步函数
- 新增 `bloom-filter` feature：`BloomFilter` 类型 + `BloomFilterBackend` 装饰器，过滤负查询（async + sync 双 API）
- 新增 `CacheError::NotSupported(String)` 错误变体（错误码 `CACHE_009`）
- 新增跨后端 TTL 行为一致性回归测试套件 (`tests/ttl_consistency_regression.rs`)
- 新增 sync API 端到端集成测试 (`tests/sync_api_integration.rs`)
- 新增 BloomFilter 端到端集成测试 (`tests/bloom_filter_integration.rs`)
- 新增 `#[cached(sync)]` 宏集成测试 (`tests/macros_sync_test.rs`)
- 新增示例：`example_sync_api` / `example_bloom_filter` / `example_moka_ttl`

### Changed
- `MokaMemoryBackend` 内部 `cache` 字段类型改为 `moka::future::Cache<String, MokaEntry, MokaExpiry>`，使用 `Expiry` trait 支持 per-entry TTL
- `MockBackend` 内部数据结构扩展为 `HashMap<String, (Vec<u8>, Option<Instant>)>`，支持 TTL 跟踪
- `ChainCache` TTL 透传行为契约化（透传 + 返回最高分链接 TTL）
- `ChainLink` 新增 `backend_sync: Option<Arc<dyn SyncCacheBackend>>` 字段以支持 sync API（async-only 后端保持 `None`）
- 更新文档以反映当前代码实现
- 修正特性分层表，与 Cargo.toml 实际定义对齐
- 移除不存在的特性引用（rate-limiting、wal-recovery 等）

### Fixed
- 修复 `MokaMemoryBackend` per-entry TTL 静默忽略的问题（违反"失败必须显性化"原则）
- 修复 `MockBackend` TTL 静默忽略的问题

## [0.2.0] - 2026-03-14

### Added
- 新增 `Cache::new()` 方法，支持特性门控的后端初始化
- 新增后端评分系统 (`BackendScoreTrait`)，支持智能后端选择
- 新增链式缓存功能 (`ChainCache`)
- 新增 Lua 脚本执行支持 (`lua-script` feature)
- 新增批量写入功能 (`batch-write` feature)
- 新增 CLI 工具支持 (`cli` feature)
- 新增 OpenTelemetry 可观测性集成
- 新增单元测试和集成测试，提升测试覆盖率

### Changed
- 重构构建器模块，优化 API 设计
- 优化核心模块实现，提升性能
- 优化后端实现，改进错误处理
- 统一测试工具模块，消除代码重复

### Fixed
- 修复基准测试重复问题
- 修复 TTL 卡死问题
- 修复测试告警问题
- 修复模块重复加载问题
- 修复文档与代码实现不一致的问题
- 移除生产代码中的 `unwrap()` 使用，优化错误处理
- 使用 `matches!` 宏简化 match 表达式

### Security
- 更新安全审计忽略列表

### Styling
- 运行 `cargo fmt` 统一代码格式

## [0.1.0] - 2024-01-01

### Added
- Initial release
