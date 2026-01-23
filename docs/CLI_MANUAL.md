# CLI 工具完整手册

> **⚠️ 实验性功能警告**
> 
> CLI 工具当前处于**实验性阶段**，仅实现了部分核心功能。本文档描述的功能可能尚未完全实现。如果您需要某个功能，请通过 [GitHub Issues](https://github.com/Kirky-X/oxcache/issues) 提供反馈，我们将根据用户需求优先实现。
>
> **当前已实现的命令**：`status`、`admin clean`、`admin warmup`、`metrics`

## 概述

Oxcache 提供了功能强大的命令行工具（CLI），用于缓存管理、监控、调试和维护。通过 CLI 工具，可以方便地执行各种缓存操作，无需编写代码。

### 核心特性

- ✅ **缓存管理**：创建、删除、清空缓存
- ✅ **数据操作**：查询、设置、删除缓存数据
- ✅ **监控统计**：查看缓存命中率、内存使用等指标
- ✅ **健康检查**：检查缓存服务状态
- ✅ **批量操作**：批量导入导出数据
- ✅ **配置管理**：查看和修改缓存配置

## 安装

### 从源码安装

```bash
# 克隆仓库
git clone https://github.com/Kirky-X/oxcache.git
cd oxcache

# 构建 CLI 工具
cargo build --release --features cli

# 安装到系统
cargo install --path . --features cli
```

### 使用 Cargo 安装

```bash
cargo install oxcache --features cli
```

## 命令概览

```
oxcache <command> [options]

已实现的命令:
  status      查看缓存状态
  admin       管理员操作
    clean     清理缓存
    warmup    缓存预热
  metrics     查看缓存指标

计划中的命令（尚未实现）:
  init        初始化缓存配置
  start       启动缓存服务
  stop        停止缓存服务
  get         获取缓存值
  set         设置缓存值
  delete      删除缓存键
  clear       清空缓存
  stats       查看缓存统计
  health      健康检查
  backup      备份缓存数据
  restore     恢复缓存数据
  export      导出缓存数据
  import      导入缓存数据
  config      配置管理
```

## 命令详解

### status - 查看状态

```bash
# 查看所有服务状态
oxcache status

# 查看指定服务状态
oxcache status --service default

# 详细输出
oxcache status --verbose

# JSON 格式输出
oxcache status --json
```

**输出示例**：

```
服务状态
========
服务名称: default
状态: 运行中
类型: two-level
启动时间: 2026-01-22 10:30:00

L1 缓存
--------
后端: moka
容量: 10000
当前大小: 5234
命中率: 85.3%
内存使用: 45.2 MB

L2 缓存
--------
后端: redis
连接: redis://localhost:6379
状态: 连接正常
命中率: 92.1%
```

### admin - 管理员操作

```bash
# 清理缓存
oxcache admin clean

# 清理指定服务
oxcache admin clean --service default

# 清理 L1 缓存
oxcache admin clean --l1

# 清理 L2 缓存
oxcache admin clean --l2

# 缓存预热
oxcache admin warmup

# 指定预热配置
oxcache admin warmup --config /path/to/warmup.toml

# 查看预热状态
oxcache admin warmup --status
```

### metrics - 查看指标

```bash
# 查看所有指标
oxcache metrics

# 查看指定服务
oxcache metrics --service default

# JSON 格式输出
oxcache metrics --json

# Prometheus 格式
oxcache metrics --prometheus

# 实时监控
oxcache metrics --watch
```

## 功能请求

CLI 工具当前处于实验性阶段，以下功能尚未实现。如果您需要这些功能，请通过 [GitHub Issues](https://github.com/Kirky-X/oxcache/issues) 提供反馈：

### 计划中的功能

- `init` - 初始化缓存配置
- `start` - 启动缓存服务
- `stop` - 停止缓存服务
- `get` - 获取缓存值
- `set` - 设置缓存值
- `delete` - 删除缓存键
- `clear` - 清空缓存
- `stats` - 查看缓存统计
- `health` - 健康检查
- `backup` - 备份缓存数据
- `restore` - 恢复缓存数据
- `export` - 导出缓存数据
- `import` - 导入缓存数据
- `config` - 配置管理

### 如何贡献

如果您愿意帮助实现这些功能，请：

1. 在 [GitHub Issues](https://github.com/Kirky-X/oxcache/issues) 上创建 issue 说明您想要实现的功能
2. Fork 仓库并创建您的功能分支
3. 提交 Pull Request
4. 我们会尽快审查和合并您的贡献

## 已知限制

1. **功能不完整** - CLI 工具只实现了部分核心功能
2. **无服务管理** - 无法启动/停止缓存服务
3. **无数据操作** - 无法直接操作缓存数据
4. **无备份恢复** - 无法备份和恢复缓存数据

## 故障排除

### 问题：命令不存在

**原因**：该命令尚未实现

**解决方案**：
1. 查看本文档的"功能请求"部分
2. 使用已实现的命令（`status`、`admin`、`metrics`）
3. 通过 [GitHub Issues](https://github.com/Kirky-X/oxcache/issues) 提供功能请求

### 问题：CLI 工具无法运行

**原因**：
- 未启用 `cli` 特性
- 未正确安装

**解决方案**：
```bash
# 使用 CLI 特性安装
cargo install oxcache --features cli

# 或从源码构建
cargo build --release --features cli
```

## 相关文档

- [用户指南](USER_GUIDE.md)
- [架构文档](ARCHITECTURE.md)
- [API 参考](API_REFERENCE.md)
- [配置验证指南](CONFIG_VALIDATION.md)

## 示例代码

- `examples/src/09_cli/` - CLI 工具示例
- `src/cli/` - CLI 工具实现