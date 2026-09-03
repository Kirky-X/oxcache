# 贡献指南

感谢您对 oxcache 项目的关注！本文档描述了参与开发所需的工具、流程和规范。

## 开发环境

- **Rust 1.88+**（edition 2024）
- **cargo**、**rustfmt**、**clippy**（随 rustup 安装）
- **pre-commit**：安装 hooks

```bash
pip install pre-commit
pre-commit install
```

## TDD 工作流

每个开发任务组遵循以下循环（Red → Green → Commit → Analyze → Next）：

1. **定接口**：先定义 trait / API 签名（`trait Xxx { ... }`），不写实现
2. **写测试**：基于接口编写单元测试（`#[cfg(test)] mod tests { ... }`），此时测试应失败（red）
3. **写代码**：实现接口，使测试通过（green）
4. **跑测试**：`cargo test --features <对应特性> --lib`，确保所有测试通过
5. **commit**：`git commit -m "feat(<模块>): <描述>"`
6. **gitnexus analyze**：用 gitnexus 工具分析本任务对其他模块的影响，识别需联动修改的代码
7. **继续下一个**：基于 analyze 结果调整后续任务，再开始下一轮循环

## Pre-commit Hooks

项目使用 pre-commit 管理代码质量 hooks：

| Hook | 阶段 | 说明 |
|------|------|------|
| `trailing-whitespace` | commit | 移除行尾空格 |
| `end-of-file-fixer` | commit | 确保文件末尾换行 |
| `check-yaml` / `check-toml` | commit | 语法检查 |
| `detect-private-key` | commit | 检测私钥泄露 |
| `detect-secrets` | commit | 检测密钥泄露 |
| `no-commit-to-branch` | commit | 禁止直接提交到 `main` / `master` |
| `cargo-fmt` | pre-push | Rust 格式检查 |
| `cargo-check` | pre-push | Rust 编译检查 |
| `cargo-clippy` | pre-push | Rust lint（`-D warnings`） |

> **禁止使用 `--no-verify` 跳过 hooks。** 这是安全红线。

## 代码质量

项目使用以下工具进行代码质量审查：

- **diting**：代码简化、架构优化、性能审查
- **tiangang**：SAST 安全扫描（发布前必须 0 CRITICAL）
- **kueiku**：硬性 bug 分析与根因定位

## Pull Request 流程

1. 从 `main` 创建 feature 分支（如 `feat/<功能>` 或 `fix/<问题>`）
2. 确保 pre-commit hooks 全部通过
3. 确保测试通过：
   ```bash
   cargo test --all-features
   cargo test --no-default-features --features core --test feature_core
   cargo test --no-default-features --features minimal --test feature_minimal
   ```
4. PR 描述包含：变更说明、测试结果、影响的模块
5. 等待 CI 全部通过后请求 review

## 代码风格

- 遵循现有代码库的命名和架构惯例（snake_case 函数名、模块组织方式等）
- **简洁优先**：只写能解决问题的最少代码，不写投机性功能
- 依赖必须通过 feature 门控，禁止使用默认特性引入不必要的依赖
- 中文注释（与现有代码库一致）
- 错误必须显性化：抛出、返回或上报，严禁吞掉或藏在默认值背后

## 常用命令

```bash
# 构建（全特性）
cargo build --all-features

# 测试（全特性）
cargo test --all-features --lib

# 窄特性测试
cargo test --no-default-features --features core --test feature_core
cargo test --no-default-features --features minimal --test feature_minimal

# 格式化
cargo fmt

# Clippy 检查
cargo clippy --all-features -- -D warnings
```
