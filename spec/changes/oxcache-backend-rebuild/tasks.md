# Tasks: OxCache 后端重构

## 阶段 1: 基础设施

### Task 1.1: 创建分数管理系统
- [x] **Step 1**: 创建 `src/backend/score.rs`
  - [x] 定义 `BackendScore` trait (BackendScoreTrait)
  - [x] 定义 `Scores` 常量结构体
- [x] **Step 2**: 实现测试
  - [x] 测试 BackendScore trait 实现
- [x] **Step 3**: 更新 `src/backend/mod.rs` 导出
- [x] **Step 4**: 运行测试确保通过

### Task 1.2: 创建链式缓存核心
- [x] **Step 1**: 创建 `src/backend/chain_cache.rs` 和 `src/chain.rs`
  - [x] 定义 `ChainLink` 结构体
  - [x] 实现 `ChainCache` 结构体
  - [x] 实现 `get` 方法 (从 L1 依次往下查找)
  - [x] 实现 `set` 方法 (写入所有后端)
  - [x] 实现 `delete` 方法
  - [x] 实现其他 CacheBackend 方法
- [x] **Step 2**: 实现测试
  - [x] 测试链式读写
  - [x] 测试多后端降级
- [x] **Step 3**: 运行测试

### Task 1.3: 创建 BackendConfig
- [x] **Step 1**: 创建 `src/builder/config.rs`
  - [x] 定义 `BackendConfig<B>` 结构体
  - [x] 实现 `new()` 构造函数
  - [x] 实现 `persistent()` 方法
- [x] **Step 2**: 实现测试

### Task 1.4: 创建 BackendSorter
- [x] **Step 1**: 创建 `src/builder/sorter.rs`
  - [x] 定义 `BackendSorter` 结构体
  - [x] 实现 `sort()` 方法
  - [x] 实现 `correct()` 修正逻辑

## 阶段 2: 现有后端改造

### Task 2.1: 改造 MokaBackend
- [ ] **Step 1**: 修改 `src/backend/moka.rs`
  - [ ] 添加 `use crate::backend::score::BackendScore`
  - [ ] 实现 `impl BackendScore for MokaBackend`
- [ ] **Step 2**: 运行测试

### Task 2.2: 改造 DashMapBackend
- [ ] **Step 1**: 修改 `src/backend/dashmap.rs`
  - [ ] 实现 `BackendScore` trait
- [ ] **Step 2**: 运行测试

### Task 2.3: 改造 RedisBackend
- [ ] **Step 1**: 修改 `src/backend/redis.rs`
  - [ ] 实现 `BackendScore` trait
- [ ] **Step 2**: 运行测试

## 阶段 3: 创建 OxCacheBuilder

### Task 3.1: 创建 OxCacheBuilder
- [ ] **Step 1**: 创建 `src/builder/oxcache_builder.rs`
  - [ ] 定义 `OxCacheBuilder` 结构体
  - [ ] 实现 `new()` 构造函数
  - [ ] 实现 `backend()` 方法
  - [ ] 实现 `ttl()` 方法
  - [ ] 实现 `max_capacity()` 方法
  - [ ] 实现 `build()` 方法 (调用 BackendSorter)
- [ ] **Step 2**: 实现测试

### Task 3.2: 更新 builder 模块
- [ ] **Step 1**: 修改 `src/builder/mod.rs`
  - [ ] 导出 `OxCacheBuilder`
  - [ ] 导出 `BackendConfig`
  - [ ] 导出 `BackendSorter`
- [ ] **Step 2**: 标记旧接口 deprecated

## 阶段 4: 新增后端实现

### Task 4.1: 实现 SQLiteBackend
- [ ] **Step 1**: 创建 `src/backend/sqlite.rs`
  - [ ] 实现 `SQLiteBackend` 结构体
  - [ ] 实现 `CacheBackend` trait
  - [ ] 实现 `BackendScore` trait (分数: 70)
- [ ] **Step 2**: 实现测试
  - [ ] 测试基本读写
  - [ ] 测试 TTL
- [ ] **Step 3**: 添加 feature flag 到 Cargo.toml
- [ ] **Step 4**: 更新 `src/backend/mod.rs` 导出

### Task 4.2: 实现 LMDBBackend
- [ ] **Step 1**: 创建 `src/backend/lmdb.rs`
  - [ ] 实现 `LMDBBackend` 结构体
  - [ ] 实现 `CacheBackend` trait
  - [ ] 实现 `BackendScore` trait (分数: 80)
- [ ] **Step 2**: 实现测试
- [ ] **Step 3**: 添加 feature flag 到 Cargo.toml
- [ ] **Step 4**: 更新导出

## 阶段 5: 集成测试

### Task 5.1: 端到端测试
- [ ] **Step 1**: 测试多后端链式
  - [ ] Moka + Redis
  - [ ] SQLite + Redis
  - [ ] Moka + SQLite + Redis
- [ ] **Step 2**: 测试自动排序
  - [ ] 乱序配置自动修正

### Task 5.2: 性能测试
- [ ] **Step 1**: 基准测试
  - [ ] 链式缓存 vs 旧 TieredBackend
- [ ] **Step 2**: 性能对比报告

## 阶段 6: 文档和清理

### Task 6.1: 更新文档
- [ ] **Step 1**: 更新 `README.md` 示例
- [ ] **Step 2**: 更新 API 文档注释

### Task 6.2: 清理旧代码
- [ ] **Step 1**: 移除 TieredBackend (deprecated 之后)
- [ ] **Step 2**: 清理废弃代码

### Task 6.3: 最终测试
- [ ] **Step 1**: 运行所有测试
- [ ] **Step 2**: 确保无回归
