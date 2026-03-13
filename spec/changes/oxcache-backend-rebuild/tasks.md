# Tasks: OxCache 后端重构

## 实施状态更新 (2026-03-13)

### 已完成的工作

#### 阶段 1: 基础设施 ✅

**Task 1.1: 创建分数管理系统** ✅
- [x] 创建 `src/backend/score.rs`
  - [x] 定义 `BackendScore` trait
  - [x] 定义 `Scores` 常量结构体 (MOKA=100, DASHMAP=90, LMDB=85, SQLITE=70, REDIS=50, MEMCACHED=40)
- [x] 实现测试 (5 个测试通过)
- [x] 更新 `src/backend/mod.rs` 导出

**Task 1.2: 创建链式缓存核心** ✅
- [x] 创建 `src/chain.rs`
  - [x] 定义 `ChainLink` 结构体
  - [x] 实现 `ChainCache` 结构体
  - [x] 实现链式 get/set/delete 方法
  - [x] 实现回填功能 (backfill)
- [x] 实现测试 (6 个测试通过)

**Task 1.3: 创建 BackendConfig** ✅
- [x] 创建 `src/builder/config.rs`
  - [x] 定义 `BackendConfig<B>` 结构体
  - [x] 实现 `persistent()` 方法

**Task 1.4: 创建 BackendSorter** ✅
- [x] 创建 `src/builder/sorter.rs`
  - [x] 实现 `sort_links()` 方法
  - [x] 实现 `validate()` 方法

#### 阶段 2: 现有后端改造 ✅

**Task 2.1: 改造 MokaBackend** ✅
- [x] 实现 `BackendScore` trait (分数: 100)

**Task 2.2: 改造 DashMapBackend** ✅
- [x] 实现 `BackendScore` trait (分数: 90)

**Task 2.3: 改造 RedisBackend** ✅
- [x] 实现 `BackendScore` trait (分数: 50)

#### 阶段 3: 创建 OxCacheBuilder ✅

**Task 3.1: 创建 OxCacheBuilder** ✅
- [x] 创建 `src/builder/oxcache_builder.rs`
  - [x] 实现 `backend()` 方法
  - [x] 实现 `default_ttl()` 方法
  - [x] 实现 `enable_backfill()` 方法
  - [x] 实现 `build()` 方法
- [x] 实现测试 (5 个测试通过)

**Task 3.2: 更新 builder 模块** ✅
- [x] 导出 `OxCacheBuilder`
- [ ] 标记旧接口 deprecated (待完成)

#### 阶段 4: 新增后端实现 ⏳

**Task 4.1: 实现 SQLiteBackend** ⏳
- [ ] 创建独立的 `SQLiteBackend` (当前 database 模块有 SQLite 支持)
- [ ] 实现 `CacheBackend` trait
- [ ] 实现 `BackendScore` trait

**Task 4.2: 实现 LMDBBackend** ⏳
- [ ] 创建 `src/backend/lmdb.rs`
- [ ] 实现 `CacheBackend` trait
- [ ] 实现 `BackendScore` trait

#### 阶段 5: 集成测试 ✅

**Task 5.1: 端到端测试** ✅
- [x] 创建 `tests/integration/chain_cache_integration_test.rs`
- [x] 11 个集成测试全部通过

**Task 5.2: 性能测试** ⏳
- [ ] 基准测试 (链式缓存 vs 旧 TieredBackend)
- [ ] 性能对比报告

#### 阶段 6: 文档和清理 ⏳

**Task 6.1: 更新文档** ⏳
- [ ] 更新 README.md 示例
- [ ] 更新 API 文档注释

**Task 6.2: 清理旧代码** ⏳
- [ ] 标记旧接口 deprecated

**Task 6.3: 最终测试** ✅
- [x] 所有测试通过 (283 个单元测试 + 11 个集成测试)

---

## 新增文件清单

| 文件 | 状态 | 描述 |
|------|------|------|
| `src/backend/score.rs` | ✅ 已创建 | 分数管理系统 |
| `src/chain.rs` | ✅ 已创建 | 链式缓存核心 |
| `src/builder/config.rs` | ✅ 已创建 | 后端配置 |
| `src/builder/sorter.rs` | ✅ 已创建 | 后端排序器 |
| `src/builder/oxcache_builder.rs` | ✅ 已创建 | 新构建器 |
| `tests/integration/chain_cache_integration_test.rs` | ✅ 已创建 | 集成测试 |
| `src/backend/sqlite.rs` | ⏳ 待创建 | SQLite 后端 |
| `src/backend/lmdb.rs` | ⏳ 待创建 | LMDB 后端 |

---

## 修改文件清单

| 文件 | 修改内容 |
|------|----------|
| `src/backend/mod.rs` | 添加 score 模块导出 |
| `src/backend/client/moka/backend.rs` | 实现 BackendScore trait |
| `src/backend/client/dashmap/backend.rs` | 实现 BackendScore trait |
| `src/backend/client/redis/client.rs` | 实现 BackendScore trait |
| `src/builder/mod.rs` | 添加 config/sorter/oxcache_builder 模块 |
| `src/lib.rs` | 导出 ChainCache, OxCacheBuilder, BackendScore, Scores |
| `tests/integration/batch_write_test.rs` | 修复失败测试 |

---

## 测试状态

### 单元测试
- **通过**: 283 个
- **失败**: 0 个
- **忽略**: 2 个

### 集成测试 (chain_cache_integration)
- **通过**: 11 个
- **失败**: 0 个

---

## 待完成工作

### 高优先级
1. 标记旧接口 deprecated

### 中优先级
1. 实现独立的 SQLiteBackend
2. 实现 LMDBBackend

### 低优先级
1. 性能基准测试
2. 更新文档
