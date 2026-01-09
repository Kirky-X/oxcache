## 1. 缓存键生成工具类
- [ ] 1.1 创建 `src/utils/key_generator.rs` 模块
- [ ] 1.2 实现 `KeyGenerator::new()` 基础构造函数
- [ ] 1.3 实现 `generate(key_template, args)` 键生成方法
- [ ] 1.4 实现 `validate(key)` 键验证方法
- [ ] 1.5 实现 `normalize(key)` 键清理/规范化方法
- [ ] 1.6 实现命名空间/前缀管理 (`with_namespace`, `with_prefix`)
- [ ] 1.7 集成 murmur3_32 哈希算法用于键指纹
- [ ] 1.8 添加单元测试

## 2. 配置模块增强
- [ ] 2.1 在 `src/config.rs` 中新增 `DynamicConfig` 结构
- [ ] 2.2 新增 `EvictionPolicy` 枚举（LRU、LFU、TinyLFU、Random）
- [ ] 2.3 新增 `CacheStrategy` 运行时配置结构
- [ ] 2.4 添加配置验证和热重载机制

## 3. 运行时策略管理 API
- [ ] 3.1 在 `src/manager.rs` 中新增 `update_strategy()` 方法
- [ ] 3.2 实现 TTL 动态调整逻辑
- [ ] 3.3 实现容量动态调整逻辑
- [ ] 3.4 实现淘汰策略动态切换逻辑
- [ ] 3.5 添加策略变更事件通知机制
- [ ] 3.6 添加 CLI 命令支持

## 4. L1 缓存策略配置增强
- [ ] 4.1 在 `src/backend/l1.rs` 中新增 `EvictionPolicy` 配置支持
- [ ] 4.2 修改 `L1Backend::new()` 支持策略参数
- [ ] 4.3 实现策略切换时的缓存重建逻辑
- [ ] 4.4 集成 Moka 的多种淘汰策略

## 5. 宏集成改进
- [ ] 5.1 在 `macros/src/lib.rs` 中集成 `KeyGenerator`
- [ ] 5.2 支持新的键模板语法 `{namespace}:{fn_name}:{args}`
- [ ] 5.3 添加宏配置选项 `eviction_policy`

## 6. 文档与测试
- [ ] 6.1 更新 `docs/` 目录下的相关文档
- [ ] 6.2 在 `examples/` 中添加使用示例
- [ ] 6.3 添加集成测试验证完整功能
- [ ] 6.4 更新 `README.md` 新功能说明
