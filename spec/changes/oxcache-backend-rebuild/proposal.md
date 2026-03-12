# Proposal: OxCache 后端重构 - 分数排序系统

## Summary

重构 OxCache 后端架构，引入内置分数排序系统，消除 TieredBackend 概念，实现可扩展的多后端链式缓存。

## Motivation

当前 OxCache 存在以下问题：

1. **架构混乱**: TieredBackend 混杂了 L1/L2 概念，后端职责不清晰
2. **扩展困难**: 新增后端需要修改 TieredBackend 代码
3. **用户困惑**: 需要理解 L1/L2 概念才能正确配置
4. **维护复杂**: 多层级联逻辑与后端实现耦合

## Goals

- [ ] 实现内置分数系统，每个后端自带分数属性
- [ ] 创建 BackendSorter 内部模块，自动修正后端顺序
- [ ] 移除 TieredBackend，统一为 ChainCache 链式缓存
- [ ] 实现 OxCacheBuilder，支持任意数量后端配置
- [ ] 新增至少 2 个后端实现 (SQLite, LMDB)
- [ ] 保持向后兼容，旧 API 可用但标记为 deprecated

## Non-Goals

- [ ] 不实现磁盘持久化 WAL（未来特性）
- [ ] 不实现分布式一致性协议（使用 Redis 原生支持）
- [ ] 不支持后端动态热插拔（运行时变更）

## Scope

### 涉及模块

- `src/backend/` - 后端接口和实现
- `src/builder/` - Builder 模式实现
- `src/chain.rs` - 新增链式缓存核心
- `src/score.rs` - 新增分数管理系统

### 新增文件

```
src/
├── backend/
│   ├── score.rs       # 分数管理系统
│   ├── sqlite.rs      # SQLite 后端
│   └── lmdb.rs        # LMDB 后端
├── builder/
│   ├── mod.rs         # OxCacheBuilder
│   ├── config.rs      # BackendConfig
│   └── sorter.rs      # BackendSorter
└── chain.rs           # ChainCache
```

### 修改文件

```
src/
├── backend/
│   ├── mod.rs         # 导出更新
│   ├── moka.rs        # 实现 BackendScore
│   ├── dashmap.rs     # 实现 BackendScore
│   └── redis.rs       # 实现 BackendScore
├── builder/
│   ├── mod.rs         # 移除 TieredBackend
│   └── backend_builder.rs  # 重构为 OxCacheBuilder
└── lib.rs             # 导出更新
```

## Success Criteria

1. **功能测试**: 所有现有测试通过
2. **新后端**: SQLite、LMDB 后端可正常工作
3. **自动排序**: 乱序配置能自动修正为正确顺序
4. **性能**: 链式缓存性能不劣于原有 TieredBackend 20%
5. **文档**: 更新 API 文档和使用示例

## Risks

1. **重构风险**: 大面积修改可能引入回归 bug
2. **兼容性**: 移除旧接口可能导致用户代码 break
3. **性能**: 多后端链式调用可能增加延迟

## Timeline

预计开发周期：2-3 周
