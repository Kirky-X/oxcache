# Pre-commit Hook

本目录包含 Oxcache 项目的 pre-commit hook 配置和脚本。

## 安装

### 方法一：使用安装脚本（推荐）

```bash
# 从项目根目录运行
./scripts/pre-commit/install-pre-commit.sh
```

### 方法二：手动安装

```bash
# 安装 pre-commit 工具
pip install pre-commit

# 安装 git hooks
pre-commit install

# 更新 hooks 到最新版本
pre-commit autoupdate
```

### 方法三：使用自定义脚本

```bash
# 创建符号链接
ln -s ../../scripts/pre-commit/pre-commit .git/hooks/pre-commit

# 确保脚本可执行
chmod +x scripts/pre-commit/pre-commit
```

## 检查内容

每次提交前，pre-commit hook 会自动运行以下检查：

### 1. 基础文件检查

| 检查项 | 说明 |
|--------|------|
| `trailing-whitespace` | 检查并修复行尾空白 |
| `end-of-file-fixer` | 确保文件以换行符结尾 |
| `check-yaml` | 验证 YAML 文件语法 |
| `check-toml` | 验证 TOML 文件语法 |
| `check-added-large-files` | 防止提交大文件（>1MB） |
| `detect-private-key` | 检测私钥泄露 |
| `check-merge-conflict` | 检测未解决的合并冲突 |
| `no-commit-to-branch` | 防止直接提交到 main/master 分支 |
| `mixed-line-ending` | 统一行尾为 LF |

### 2. 密钥检测

使用 `detect-secrets` 检测潜在的敏感信息泄露。

### 3. Rust 代码检查

| 检查项 | 命令 | 说明 |
|--------|------|------|
| 代码格式化 | `cargo fmt -- --check` | 确保代码符合 rustfmt 规范 |
| 编译检查 | `cargo check --lib` | 验证代码编译通过 |
| Clippy 静态分析 | `cargo clippy --lib -- -D warnings` | 代码质量检查 |

## 跳过 Hook

### 跳过单次提交

```bash
git commit --no-verify -m "Your commit message"
# 或
git commit -n -m "Your commit message"
```

### 跳过特定检查

```bash
# 跳过所有检查
SKIP=cargo-fmt,cargo-clippy git commit -m "message"

# 仅运行特定检查
pre-commit run cargo-fmt --files src/main.rs
```

## 手动运行

### 运行所有检查

```bash
# 对所有文件运行
pre-commit run --all-files

# 对暂存文件运行
pre-commit run
```

### 运行自定义脚本

```bash
# 运行完整的 pre-commit 检查（包括测试）
./scripts/pre-commit/pre-commit

# 运行所有预提交检查脚本
./scripts/pre-commit/run-all.sh
```

## 配置

配置文件位于项目根目录的 `.pre-commit-config.yaml`。

### 添加新的检查

编辑 `.pre-commit-config.yaml`：

```yaml
repos:
  - repo: local
    hooks:
      - id: my-custom-check
        name: My Custom Check
        entry: ./scripts/my-check.sh
        language: script
        types: [rust]
```

### 禁用特定检查

在 `.pre-commit-config.yaml` 中注释掉或删除对应的 hook。

## 故障排除

### Hook 未运行

确保 pre-commit hook 可执行：

```bash
chmod +x .git/hooks/pre-commit
chmod +x scripts/pre-commit/pre-commit
```

### pre-commit 命令未找到

安装 pre-commit：

```bash
pip install pre-commit
# 或
pip3 install pre-commit
```

### 检查速度慢

1. 减少检查范围：修改 `.pre-commit-config.yaml` 中的 `types` 或 `files` 过滤器
2. 跳过耗时检查：使用 `SKIP` 环境变量
3. 使用缓存：pre-commit 会自动缓存环境

### Clippy 误报

如果 Clippy 产生误报：

1. 更新 Rust 工具链：
   ```bash
   rustup update stable
   cargo update
   ```

2. 在代码中添加允许标记：
   ```rust
   #[allow(clippy::lint_name)]
   ```

3. 在 `.pre-commit-config.yaml` 中调整 Clippy 参数

## CI 集成

Pre-commit 检查与 GitHub CI 工作流保持一致：

- CI 工作流：[`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)
- Pre-commit 检查是 CI 检查的快速子集
- 完整 CI 在推送到 `main` 和 `develop` 分支时运行

## 相关文件

```
oxcache/
├── .pre-commit-config.yaml      # Pre-commit 配置文件
├── .git/hooks/pre-commit        # Git hook（由 pre-commit 生成）
└── scripts/pre-commit/
    ├── pre-commit               # 自定义 hook 脚本
    ├── install-pre-commit.sh    # 安装脚本
    ├── run-all.sh               # 运行所有检查
    ├── precommit_audit.sh       # 安全审计
    ├── precommit_clippy.sh      # Clippy 检查
    ├── precommit_deny.sh        # 依赖安全检查
    ├── precommit_secrets.sh     # 密钥检测
    ├── precommit_license.sh     # 许可证检查
    ├── precommit_toml.sh        # TOML 验证
    ├── precommit_tests.sh       # 测试检查
    └── README.md                # 本文档
```

## 参考资料

- [Pre-commit 官方文档](https://pre-commit.com/)
- [Pre-commit Hooks 仓库](https://github.com/pre-commit/pre-commit-hooks)
- [Rust Clippy 文档](https://rust-lang.github.io/rust-clippy/)
- [detect-secrets 文档](https://github.com/Yelp/detect-secrets)
