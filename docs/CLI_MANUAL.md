# CLI 工具完整手册

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

命令:
  init        初始化缓存配置
  start       启动缓存服务
  stop        停止缓存服务
  status      查看缓存状态
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
  help        显示帮助信息
```

## 命令详解

### init - 初始化配置

```bash
# 创建默认配置文件
oxcache init

# 指定配置文件路径
oxcache init --config /path/to/config.toml

# 使用交互式配置
oxcache init --interactive
```

**生成的配置文件示例**：

```toml
[global]
log_level = "info"
metrics_enabled = true

[services.default]
type = "two-level"
ttl = 3600

[services.default.l1]
backend = "moka"
capacity = 10000

[services.default.l2]
backend = "redis"
connection_string = "redis://localhost:6379"
mode = "standalone"
```

### start - 启动服务

```bash
# 使用默认配置启动
oxcache start

# 指定配置文件启动
oxcache start --config /path/to/config.toml

# 指定端口启动
oxcache start --port 8080

# 后台运行
oxcache start --daemon

# 指定日志级别
oxcache start --log-level debug
```

### stop - 停止服务

```bash
# 停止服务
oxcache stop

# 强制停止
oxcache stop --force

# 指定服务名称停止
oxcache stop --service my_cache
```

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

### get - 获取缓存值

```bash
# 获取缓存值
oxcache get user:123

# 指定服务
oxcache get user:123 --service default

# JSON 格式输出
oxcache get user:123 --json

# 显示详细信息
oxcache get user:123 --verbose

# 检查键是否存在
oxcache get user:123 --exists
```

**输出示例**：

```
键: user:123
值: {"id":123,"name":"张三","email":"zhangsan@example.com"}
TTL: 2847 秒
命中: L1
创建时间: 2026-01-22 10:35:00
```

### set - 设置缓存值

```bash
# 设置缓存值
oxcache set user:123 '{"id":123,"name":"张三"}'

# 指定 TTL
oxcache set user:123 '{"id":123,"name":"张三"}' --ttl 3600

# 指定服务
oxcache set user:123 '{"id":123,"name":"张三"}' --service default

# 从文件读取
oxcache set user:123 --file /path/to/data.json

# 设置多个值
oxcache set user:123 '{"id":123}' user:456 '{"id":456}'
```

### delete - 删除缓存键

```bash
# 删除单个键
oxcache delete user:123

# 删除多个键
oxcache delete user:123 user:456 user:789

# 按模式删除
oxcache delete --pattern "user:*"

# 指定服务
oxcache delete user:123 --service default

# 批量删除
oxcache delete --batch /path/to/keys.txt
```

### clear - 清空缓存

```bash
# 清空所有缓存
oxcache clear

# 清空指定服务
oxcache clear --service default

# 清空 L1 缓存
oxcache clear --l1

# 清空 L2 缓存
oxcache clear --l2

# 确认后清空
oxcache clear --confirm
```

### stats - 查看统计

```bash
# 查看所有统计
oxcache stats

# 查看指定服务
oxcache stats --service default

# 实时监控
oxcache stats --watch

# 指定刷新间隔
oxcache stats --watch --interval 5

# 只显示命中率
oxcache stats --hit-rate

# 只显示内存使用
oxcache stats --memory
```

**输出示例**：

```
缓存统计
========
服务名称: default

L1 统计
--------
总请求数: 100000
命中数: 85300
未命中数: 14700
命中率: 85.3%
平均响应时间: 0.1 ms

L2 统计
--------
总请求数: 14700
命中数: 13500
未命中数: 1200
命中率: 92.1%
平均响应时间: 2.5 ms

内存使用
--------
L1 内存: 45.2 MB
L2 内存: 128.5 MB
总内存: 173.7 MB
```

### health - 健康检查

```bash
# 执行健康检查
oxcache health

# 详细输出
oxcache health --verbose

# JSON 格式
oxcache health --json

# 持续检查
oxcache health --watch

# 指定检查间隔
oxcache health --watch --interval 10
```

**输出示例**：

```
健康检查
========
状态: 健康 ✅

L1 缓存: 正常 ✅
  - 容量: 10000
  - 当前大小: 5234
  - 命中率: 85.3%

L2 缓存: 正常 ✅
  - 连接: redis://localhost:6379
  - 状态: 连接正常
  - 命中率: 92.1%

WAL 恢复: 正常 ✅
  - 文件: /path/to/wal.db
  - 大小: 1.2 MB

指标收集: 正常 ✅
  - 端点: http://localhost:9464
```

### backup - 备份数据

```bash
# 备份所有数据
oxcache backup --output /path/to/backup.json

# 备份指定服务
oxcache backup --service default --output /path/to/backup.json

# 备份到压缩文件
oxcache backup --output /path/to/backup.json.gz

# 只备份 L1 数据
oxcache backup --l1 --output /path/to/backup.json

# 只备份 L2 数据
oxcache backup --l2 --output /path/to/backup.json

# 包含元数据
oxcache backup --include-metadata --output /path/to/backup.json
```

### restore - 恢复数据

```bash
# 从备份恢复
oxcache restore --input /path/to/backup.json

# 恢复到指定服务
oxcache restore --input /path/to/backup.json --service default

# 覆盖已存在的键
oxcache restore --input /path/to/backup.json --overwrite

# 只恢复 L1 数据
oxcache restore --input /path/to/backup.json --l1

# 只恢复 L2 数据
oxcache restore --input /path/to/backup.json --l2

# 从压缩文件恢复
oxcache restore --input /path/to/backup.json.gz
```

### export - 导出数据

```bash
# 导出所有数据
oxcache export --output /path/to/export.json

# 导出指定键
oxcache export user:123 user:456 --output /path/to/export.json

# 按模式导出
oxcache export --pattern "user:*" --output /path/to/export.json

# 指定格式
oxcache export --format json --output /path/to/export.json
oxcache export --format csv --output /path/to/export.csv

# 包含 TTL
oxcache export --include-ttl --output /path/to/export.json
```

### import - 导入数据

```bash
# 导入数据
oxcache import --input /path/to/import.json

# 指定服务
oxcache import --input /path/to/import.json --service default

# 跳过错误
oxcache import --input /path/to/import.json --skip-errors

# 批量大小
oxcache import --input /path/to/import.json --batch-size 100

# 显示进度
oxcache import --input /path/to/import.json --progress
```

### config - 配置管理

```bash
# 查看当前配置
oxcache config show

# 查看指定配置项
oxcache config get services.default.ttl

# 设置配置项
oxcache config set services.default.ttl 7200

# 重置配置
oxcache config reset

# 验证配置
oxcache config validate

# 重新加载配置
oxcache config reload
```

## 高级用法

### 批量操作

```bash
# 批量设置
cat keys.txt | while read key value; do
    oxcache set "$key" "$value"
done

# 批量删除
cat keys.txt | xargs oxcache delete

# 批量导出
for service in service1 service2 service3; do
    oxcache export --service "$service" --output "$service.json"
done
```

### 脚本集成

```bash
#!/bin/bash
# cache_backup.sh - 备份脚本

BACKUP_DIR="/backups/oxcache"
DATE=$(date +%Y%m%d_%H%M%S)

# 创建备份目录
mkdir -p "$BACKUP_DIR"

# 备份所有服务
for service in $(oxcache status --json | jq -r '.services[].name'); do
    oxcache backup --service "$service" --output "$BACKUP_DIR/${service}_${DATE}.json"
done

# 清理旧备份（保留最近 7 天）
find "$BACKUP_DIR" -name "*.json" -mtime +7 -delete

echo "备份完成: $DATE"
```

### 监控告警

```bash
#!/bin/bash
# cache_monitor.sh - 监控脚本

# 检查健康状态
if ! oxcache health --json | jq -e '.status == "healthy"' > /dev/null; then
    echo "警告: 缓存服务不健康"
    # 发送告警
    curl -X POST "https://api.example.com/alert" \
        -d "message=缓存服务不健康"
fi

# 检查命中率
HIT_RATE=$(oxcache stats --json | jq '.hit_rate')
if (( $(echo "$HIT_RATE < 0.7" | bc -l) )); then
    echo "警告: 缓存命中率为 $HIT_RATE，低于 70%"
    # 发送告警
    curl -X POST "https://api.example.com/alert" \
        -d "message=缓存命中率为 $HIT_RATE"
fi
```

### 定时任务

```bash
# 添加到 crontab
# 每小时备份
0 * * * * /path/to/cache_backup.sh

# 每 10 分钟检查健康状态
*/10 * * * * /path/to/cache_monitor.sh

# 每天凌晨 3 点清理过期数据
0 3 * * * oxcache clear --confirm
```

## 配置文件

### 完整配置示例

```toml
[global]
log_level = "info"
metrics_enabled = true
metrics_port = 9464
health_check_interval = 30

[services.default]
type = "two-level"
ttl = 3600

[services.default.l1]
backend = "moka"
capacity = 10000
ttl = 600

[services.default.l2]
backend = "redis"
connection_string = "redis://localhost:6379"
mode = "standalone"
ttl = 3600

[services.user_cache]
type = "l2-only"
ttl = 7200

[services.user_cache.l2]
backend = "redis"
connection_string = "redis://localhost:6379"
mode = "sentinel"
sentinel_nodes = ["localhost:26379", "localhost:26380", "localhost:26381"]
master_name = "mymaster"

[services.product_cache]
type = "two-level"
ttl = 1800

[services.product_cache.l1]
backend = "moka"
capacity = 50000
ttl = 300

[services.product_cache.l2]
backend = "redis"
connection_string = "redis://localhost:6379"
mode = "cluster"
cluster_nodes = [
    "localhost:7000",
    "localhost:7001",
    "localhost:7002",
]

[features]
bloom_filter_enabled = true
rate_limiting_enabled = true
smart_strategy_enabled = true

[features.bloom_filter]
expected_items = 1000000
false_positive_rate = 0.01

[features.rate_limiting]
max_requests_per_second = 1000
burst_capacity = 2000
block_duration_secs = 10

[features.smart_strategy]
prefetch_enabled = true
compression_enabled = true
```

## 最佳实践

### ✅ 推荐做法

1. **定期备份**：设置定时任务定期备份缓存数据
2. **监控告警**：监控缓存健康状态和命中率
3. **配置管理**：使用配置文件管理缓存设置
4. **日志记录**：启用日志记录，便于问题排查
5. **安全防护**：限制 CLI 工具访问权限

### ❌ 避免做法

1. **过度清空**：不要频繁清空缓存，影响性能
2. **忽略错误**：不要忽略 CLI 命令的错误输出
3. **硬编码配置**：不要在脚本中硬编码配置
4. **无备份操作**：不要在没有备份的情况下执行危险操作
5. **权限过宽**：不要给 CLI 工具过高的权限

## 故障排除

### 问题：服务无法启动

**原因**：
- 配置文件错误
- 端口被占用
- Redis 连接失败

**解决方案**：
1. 检查配置文件语法
2. 检查端口占用情况
3. 验证 Redis 连接

### 问题：命令执行失败

**原因**：
- 服务未启动
- 权限不足
- 参数错误

**解决方案**：
1. 确认服务已启动
2. 检查文件权限
3. 验证命令参数

### 问题：数据丢失

**原因**：
- 未启用 WAL
- Redis 故障
- 误操作

**解决方案**：
1. 启用 WAL 功能
2. 配置 Redis 持久化
3. 定期备份数据

## 相关文档

- [用户指南](USER_GUIDE.md)
- [架构文档](ARCHITECTURE.md)
- [API 参考](API_REFERENCE.md)
- [配置验证指南](CONFIG_VALIDATION.md)

## 示例代码

- `examples/src/09_cli/` - CLI 工具示例
- `src/cli/` - CLI 工具实现