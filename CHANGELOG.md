# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- 新增 CacheKey trait 示例 (`example_cache_key.rs`)
- 新增数据压缩功能示例 (`example_compression.rs`)

### Changed
- 更新文档以反映当前代码实现
- 修正特性分层表，与 Cargo.toml 实际定义对齐
- 移除不存在的特性引用（bloom-filter、rate-limiting、wal-recovery 等）

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
