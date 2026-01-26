# Oxcache 脚本目录结构

## 目录结构

```
scripts/
├── lib/
│   └── common.sh              # 统一的公共函数库
├── pre-commit/
│   ├── run-all.sh             # 统一运行所有预提交检查
│   ├── precommit_audit.sh     # 安全审计
│   ├── precommit_clippy.sh    # 代码质量检查
│   ├── precommit_deny.sh      # 依赖安全检查
│   ├── precommit_secrets.sh   # 密钥检测
│   ├── precommit_license.sh   # 许可证合规检查
│   ├── precommit_toml.sh      # TOML配置验证
│   └── precommit_tests.sh     # 测试检查
├── tests/
│   ├── run_all_tests.sh       # 运行所有测试
│   ├── memory_test.sh         # 内存泄漏测试
│   ├── real_redis_test.sh     # Redis真实环境测试
│   └── test_redis_failover.sh # Redis故障转移测试
├── validation/
│   ├── validate-feature-combinations.sh # 特性组合验证
│   ├── validate_docs.sh                 # 文档验证
│   └── security_audit.sh                # 安全审计
└── performance/
    └── redis_perf_test.sh     # Redis性能测试
```

## 使用说明

### 预提交检查

运行所有预提交检查：

```bash
./scripts/pre-commit/run-all.sh
```

或者单独运行某个检查：

```bash
./scripts/pre-commit/precommit_audit.sh
./scripts/pre-commit/precommit_clippy.sh
```

### 测试脚本

运行所有测试：

```bash
./scripts/tests/run_all_tests.sh
```

### 验证脚本

验证特性组合：

```bash
./scripts/validation/validate-feature-combinations.sh --all
```

验证文档：

```bash
./scripts/validation/validate_docs.sh
```

## 公共库

所有脚本都使用 `lib/common.sh` 中的公共函数，包括：

- `log_info()` - 信息日志
- `log_success()` - 成功日志
- `log_warning()` - 警告日志
- `log_error()` - 错误日志
- `print_header()` - 打印标题头
- `print_section()` - 打印章节头

## 统一入口

也可以使用统一入口脚本运行所有功能：

```bash
./scripts/run-all.sh --all
```

### 运行特定类别的脚本

运行所有测试：

```bash
./scripts/run-all.sh --tests
```

运行所有验证：

```bash
./scripts/run-all.sh --validation
```

运行所有预提交检查：

```bash
./scripts/run-all.sh --pre-commit
```

运行性能测试：

```bash
./scripts/run-all.sh --performance
```