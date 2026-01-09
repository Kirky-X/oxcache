## Context

Oxcache 是一个高性能双层缓存库，当前存在以下限制：
1. 缓存键生成仅通过宏实现，缺乏独立的工具类
2. 缓存策略在启动时固定，无法运行时调整
3. L1 缓存淘汰策略不可配置

本次增强旨在解决这些问题，同时保持向后兼容和高性能特性。

## Goals / Non-Goals

### Goals:
- 提供独立的 `KeyGenerator` 工具类
- 支持运行时动态调整缓存策略
- 暴露 L1 缓存的淘汰策略配置选项
- 保持零侵入式使用体验

### Non-Goals:
- 不修改现有的 `#[cached]` 宏的基本行为
- 不移除任何现有 API
- 不引入外部重型依赖（如重新实现哈希算法）

## Decisions

### Decision 1: KeyGenerator 设计
- **选择**: 创建独立的 `KeyGenerator` struct，提供静态方法和实例方法
- **理由**: 工具类模式在 Rust 中常见，便于使用且易于测试
- **替代方案考虑**: 
  - Trait-based 设计 → 过于复杂，对工具类不必要
  - 直接在宏中扩展 → 破坏单一职责

```rust
pub struct KeyGenerator {
    namespace: String,
    prefix: String,
    hash_algo: HashAlgo,
}

impl KeyGenerator {
    pub fn new() -> Self { ... }
    pub fn with_namespace(mut self, ns: String) -> Self { ... }
    pub fn generate(&self, template: &str, args: &[(&str, &str)]) -> String { ... }
}
```

### Decision 2: 动态配置存储
- **选择**: 使用 `DashMap` 存储运行时配置，支持并发读写
- **理由**: 与现有 `CacheManager` 架构一致，无需引入新同步机制
- **替代方案考虑**:
  - `RwLock` → 性能不如 `DashMap` 的细粒度锁
  - `Mutex` → 写锁竞争激烈

### Decision 3: 淘汰策略实现
- **选择**: 通过 Moka 的 `Builder` 模式配置策略
- **理由**: Moka 本身支持多种策略，直接映射即可
- **替代方案考虑**:
  - 自定义 LRU 实现 → 性能无法保证，且不必要

```rust
pub enum EvictionPolicy {
    Lru,
    Lfu,
    TinyLfu,
    Random,
}

impl L1Backend {
    pub fn with_policy(mut self, policy: EvictionPolicy) -> Self { ... }
}
```

### Decision 4: 策略变更事件
- **选择**: 使用 `tracing::event!` 记录策略变更
- **理由**: 符合现有日志记录实践，无需引入新事件系统
- **替代方案考虑**:
  - 自定义事件监听器 → 增加复杂度，当前场景不必要

## Risks / Trade-offs

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 动态策略切换导致性能抖动 | 中 | 策略变更应稀疏发生，且有缓存预热补偿 |
| KeyGenerator 误用导致键冲突 | 低 | 提供默认命名空间，文档明确说明 |
| Moka 版本升级改变 API | 低 | 封装 Moka 调用，隔离依赖 |

## Migration Plan

1. **向后兼容**: 所有新 API 添加为可选使用，现有代码无需修改
2. **渐进式采用**: 
   - 用户可逐步迁移到新的键生成工具
   - 动态策略默认为现有静态配置
3. **回滚方案**: 策略变更失败时回滚到原配置

## Open Questions

1. ~~是否需要支持远程配置源（如 etcd）动态推送配置？~~ → 暂不需要，当前仅支持 API 动态调整
2. ~~是否需要为键生成提供缓存友好的字符串 interning？~~ → 暂不需要，Moka 内部已处理
