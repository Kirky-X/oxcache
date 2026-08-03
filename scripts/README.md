# Oxcache 脚本目录

## 目录结构

```text
scripts/
├── hooks/                     # Git Hook 系统
│   ├── pre-commit.sh          # 7 阶段检查主脚本
│   └── setup-hooks.sh         # 安装/卸载脚本
├── lib/
│   └── common.sh              # 公共函数库
├── tests/
│   ├── run_all_tests.sh       # 综合测试运行器
│   └── memory_test.sh         # 内存泄漏测试 (Miri/Valgrind)
├── redis.sh                   # Redis 服务管理 + 性能测试
├── validate.sh                # 验证脚本 (特性/文档/安全)
├── run-all.sh                 # 统一入口
└── README.md                  # 本文档
```

## Hook 系统

```bash
bash scripts/hooks/setup-hooks.sh          # 安装
bash scripts/hooks/pre-commit.sh           # 手动运行 7 阶段检查
bash scripts/hooks/setup-hooks.sh --uninstall  # 卸载
```

## Redis 服务管理

```bash
./scripts/redis.sh start              # 启动单机
./scripts/redis.sh stop               # 停止单机
./scripts/redis.sh start-cluster      # 启动 Cluster
./scripts/redis.sh stop-cluster       # 停止 Cluster
./scripts/redis.sh start-sentinel     # 启动 Sentinel
./scripts/redis.sh stop-sentinel      # 停止 Sentinel
./scripts/redis.sh restart            # 重启
./scripts/redis.sh perf               # 性能测试
```

## 验证

```bash
./scripts/validate.sh features --all   # 特性组合
./scripts/validate.sh features --quick # 快速验证
./scripts/validate.sh docs             # 文档
./scripts/validate.sh security         # 安全审计
./scripts/validate.sh all              # 全部
```

## 测试

```bash
./scripts/tests/run_all_tests.sh              # 全部
./scripts/tests/run_all_tests.sh unit         # 单元
./scripts/tests/run_all_tests.sh integration  # 集成
./scripts/tests/run_all_tests.sh coverage     # 覆盖率
./scripts/tests/run_all_tests.sh clean        # 清理
```

## 统一入口

```bash
./scripts/run-all.sh --all             # 全部
./scripts/run-all.sh --pre-commit      # Hook 检查
./scripts/run-all.sh --tests           # 测试
./scripts/run-all.sh --validation      # 验证
./scripts/run-all.sh --performance     # 性能
```
