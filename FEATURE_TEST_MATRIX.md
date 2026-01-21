# Oxcache 特性组合测试矩阵

## 测试策略

### 三层测试策略

#### Tier 1: 预定义特性集测试

测试三个预定义的特性集，这些是最常用的配置：

1. **Minimal**: `--features minimal`
   - 仅 L1 内存缓存
   - 适合单机应用
   - 测试目标：确保基础功能正常

2. **Core**: `--features core`
   - L1 + L2 分层缓存
   - 适合需要分布式缓存的应用
   - 测试目标：确保分层缓存功能正常

3. **Full**: `--features full`
   - 所有特性
   - 适合生产环境
   - 测试目标：确保所有功能正常

#### Tier 2: 关键特性组合测试

测试关键但非预定义的特性组合：

1. **L1 Only + Macros**: `--features "l1-moka,macros"`
   - 测试 L1 缓存和宏功能
   - 适合快速开发

2. **L2 Only + Macros**: `--features "l2-redis,macros"`
   - 测试 L2 缓存和宏功能
   - 适合需要分布式缓存但不需 L1 的场景

3. **Tiered + Batch Write**: `--features "l1-moka,l2-redis,batch-write"`
   - 测试分层缓存和批量写入
   - 适合高吞吐量场景

4. **Tiered + Metrics**: `--features "l1-moka,l2-redis,metrics"`
   - 测试分层缓存和指标收集
   - 适合需要监控的场景

5. **Tiered + Bloom Filter**: `--features "l1-moka,l2-redis,bloom-filter"`
   - 测试分层缓存和布隆过滤器
   - 适合需要防穿透的场景

6. **Tiered + Database**: `--features "l1-moka,l2-redis,database"`
   - 测试分层缓存和数据库集成
   - 适合需要数据库加载的场景

7. **Core + Full Metrics**: `--features "core,full-metrics"`
   - 测试核心功能和完整指标
   - 适合生产监控

8. **Core + Confers**: `--features "core,confers"`
   - 测试核心功能和配置管理
   - 适合需要动态配置的场景

9. **Core + Smart Strategy**: `--features "core,smart-strategy"`
   - 测试核心功能和智能策略
   - 适合需要自适应策略的场景

10. **Core + HTTP Cache**: `--features "core,http-cache"`
    - 测试核心功能和 HTTP 缓存
    - 适合 Web 应用

#### Tier 3: 全面特性组合测试（可选）

测试所有可能的特性组合，确保没有遗漏。这个层级可以定期运行（例如每周或每月），而不是每次提交都运行。

## 测试矩阵

| ID | 特性组合 | 用例 | 测试频率 | 优先级 |
|----|---------|------|---------|--------|
| T1-1 | minimal | 单机应用 | 每次 | 高 |
| T1-2 | core | 分布式缓存 | 每次 | 高 |
| T1-3 | full | 生产环境 | 每次 | 高 |
| T2-1 | l1-moka,macros | 快速开发 | 每次 | 中 |
| T2-2 | l2-redis,macros | 分布式缓存（无 L1） | 每次 | 中 |
| T2-3 | l1-moka,l2-redis,batch-write | 高吞吐量 | 每次 | 中 |
| T2-4 | l1-moka,l2-redis,metrics | 监控 | 每次 | 中 |
| T2-5 | l1-moka,l2-redis,bloom-filter | 防穿透 | 每次 | 中 |
| T2-6 | l1-moka,l2-redis,database | 数据库加载 | 每次 | 中 |
| T2-7 | core,full-metrics | 生产监控 | 每次 | 中 |
| T2-8 | core,confers | 动态配置 | 每次 | 中 |
| T2-9 | core,smart-strategy | 自适应策略 | 每次 | 中 |
| T2-10 | core,http-cache | Web 应用 | 每次 | 中 |
| T3-* | 所有组合 | 全面测试 | 每周 | 低 |

## 测试覆盖

### 编译测试

确保每个特性组合都能成功编译：

```bash
# Tier 1
cargo check --features minimal
cargo check --features core
cargo check --features full

# Tier 2
cargo check --features "l1-moka,macros"
cargo check --features "l2-redis,macros"
cargo check --features "l1-moka,l2-redis,batch-write"
# ... 其他组合
```

### 单元测试

运行每个特性组合的单元测试：

```bash
# Tier 1
cargo test --features minimal --lib
cargo test --features core --lib
cargo test --features full --lib

# Tier 2
cargo test --features "l1-moka,macros" --lib
# ... 其他组合
```

### 集成测试

运行每个特性组合的集成测试：

```bash
# Tier 1
cargo test --features minimal --test '*'
cargo test --features core --test '*'
cargo test --features full --test '*'

# Tier 2
cargo test --features "l1-moka,macros" --test '*'
# ... 其他组合
```

### 文档测试

确保文档示例在每个特性组合下都能编译：

```bash
cargo test --features minimal --doc
cargo test --features core --doc
cargo test --features full --doc
```

## 自动化验证

### CI 集成

所有 Tier 1 和 Tier 2 测试都在 CI 中自动运行：

- 每次提交运行 Tier 1 和 Tier 2 测试
- 每周运行 Tier 3 测试（可选）

### 本地验证

开发者可以使用以下命令在本地验证特性组合：

```bash
# 验证特定特性组合
./scripts/validate-feature-combinations.sh "l1-moka,macros"

# 验证所有 Tier 1 和 Tier 2 组合
./scripts/validate-feature-combinations.sh --all

# 生成测试报告
./scripts/validate-feature-combinations.sh --report
```

### 预提交钩子

可以使用 pre-commit 钩子自动验证特性组合：

```bash
# .git/hooks/pre-commit
#!/bin/bash
./scripts/validate-feature-combinations.sh --quick
```

## 测试失败处理

### 编译失败

如果某个特性组合编译失败：

1. 检查特性依赖关系
2. 查看编译错误信息
3. 修复代码或调整特性定义
4. 更新文档

### 测试失败

如果某个特性组合的测试失败：

1. 检查测试是否应该在该特性组合下运行
2. 查看测试失败原因
3. 修复测试或代码
4. 添加特性门控（如果需要）

### 文档测试失败

如果文档测试失败：

1. 检查文档示例是否正确
2. 检查特性依赖是否正确
3. 修复文档或代码
4. 确保示例在正确的特性组合下运行

## 测试报告

### 报告格式

测试报告应包含以下信息：

- 测试日期和时间
- 测试的 Rust 版本
- 每个特性组合的测试结果
- 编译时间
- 测试运行时间
- 失败的测试（如果有）
- 警告信息

### 报告示例

```markdown
# Feature Combination Test Report

**Date**: 2026-01-21 10:00:00 UTC
**Rust Version**: 1.75.0
**Total Combinations**: 13

## Results

| ID | Features | Compile | Unit Tests | Integration Tests | Status |
|----|----------|---------|------------|-------------------|--------|
| T1-1 | minimal | ✅ (10s) | ✅ (5s) | ✅ (15s) | Pass |
| T1-2 | core | ✅ (12s) | ✅ (7s) | ✅ (20s) | Pass |
| T1-3 | full | ✅ (15s) | ✅ (10s) | ✅ (30s) | Pass |
| T2-1 | l1-moka,macros | ✅ (8s) | ✅ (4s) | ✅ (12s) | Pass |
| ... | ... | ... | ... | ... | ... |

## Summary

- **Total**: 13 combinations
- **Passed**: 13
- **Failed**: 0
- **Total Time**: 5m 30s

## Issues

No issues found.
```

## 维护指南

### 添加新特性

当添加新特性时：

1. 更新 `FEATURE_ANALYSIS.md`
2. 确定新特性的依赖关系
3. 更新测试矩阵，添加相关组合
4. 更新 CI 配置
5. 运行所有测试

### 移除特性

当移除特性时：

1. 从 `FEATURE_ANALYSIS.md` 中移除
2. 从测试矩阵中移除相关组合
3. 更新 CI 配置
4. 运行所有测试

### 修改特性依赖

当修改特性依赖时：

1. 更新 `FEATURE_ANALYSIS.md`
2. 更新受影响的特性组合
3. 更新测试矩阵
4. 运行所有测试

## 最佳实践

1. **保持测试矩阵简洁**: 只测试关键组合，避免测试过多组合
2. **定期审查**: 定期审查测试矩阵，移除不再需要的组合
3. **文档化**: 为每个特性组合添加用例说明
4. **自动化**: 尽可能自动化测试流程
5. **快速反馈**: 确保测试快速运行，提供快速反馈