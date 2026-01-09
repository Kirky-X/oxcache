# Change: 增强缓存键生成与策略管理

## Why

当前 Oxcache 缓存系统存在以下问题：

1. **缓存键生成不完善**：仅有 `#[cached]` 宏生成的键，没有独立的键生成工具类，缺乏标准化键前缀/命名空间管理和键验证/清理工具
2. **动态策略切换缺失**：配置在启动时固定，无法在运行时动态调整 TTL、容量或淘汰策略
3. **LRU 淘汰机制不透明**：L1 缓存依赖 Moka 内置策略，未暴露可配置的淘汰策略选项

这些问题限制了缓存系统的灵活性和运维能力。

## What Changes

- **新增 `KeyGenerator` 工具类**：提供标准化的缓存键生成、验证和清理功能
- **新增键前缀/命名空间管理**：支持服务级、应用级键前缀，避免键冲突
- **新增运行时策略管理 API**：支持动态调整 TTL、容量、淘汰策略等配置
- **新增 L1 淘汰策略配置**：暴露 Moka 的淘汰策略选项（LRU、LFU、TinyLFU 等）

## Impact

- Affected specs: `cache-core`
- Affected code:
  - `src/utils/` (新增 key_generator.rs)
  - `src/manager.rs` (新增运行时配置 API)
  - `src/config.rs` (新增动态配置结构)
  - `src/backend/l1.rs` (新增策略配置)
  - `macros/src/lib.rs` (集成新的键生成工具)

## Compatibility

- 此变更向后兼容，不影响现有 API
- 新增功能为可选使用，不强制修改现有代码
