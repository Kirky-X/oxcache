# 🔒 oxcache 安全文档

本文档描述 oxcache 的支持版本、漏洞报告流程与供应链门禁。所有条目均按仓库现有配置逐项核对（`Cargo.toml`、`.github/workflows/`、`.cargo/audit.toml`、`deny.toml`、`.pre-commit-config.yaml`、`src/lib.rs`）。

## 📌 支持版本

oxcache 处于 0.x 阶段，仅对**最新次版本线**提供安全修复。

| 版本 | 支持状态 |
|------|----------|
| 0.5.x（最新次版本，当前 `0.5.0-rc.8`） | ✅ 支持安全更新 |
| 更早的 0.x 版本 | ❌ 不支持 |

MSRV 为 **Rust 1.97.1**（根 `Cargo.toml` 的 `[workspace.package] rust-version`），edition 为 2024。

## 🐛 漏洞报告流程

**请勿通过公开 GitHub Issue 报告安全漏洞。**

1. **首选渠道**：通过 GitHub [Security Advisories](https://github.com/Kirky-X/oxcache/security/policy) 私密披露通道提交报告（仓库页 **Security → Report a vulnerability**）。
2. **备选渠道**：仅当首选通道不可用时，发送邮件至 [Kirky-X@outlook.com](mailto:Kirky-X@outlook.com)（受理人：Kirky.X）。

报告请尽量包含：

- 受影响的版本与 feature 组合（如 `--features full`、`redis`/`valkey`/`dragonfly`/`aerospike` 后端选择、`serde-bincode` 等序列化特性）
- 复现步骤或最小示例（含 L1/L2/L3 层级与 key 形态）
- 影响评估（CWE 分类如可判断）
- 可行的临时缓解措施

### 响应承诺

| 阶段 | 时间线 |
|------|--------|
| 确认收到 | 48 小时内 |
| 初步评估（接受 / 拒绝 / 需补充信息） | 7 天内 |
| 已接受漏洞的修复 | 视严重程度尽快发布 |
| 协同披露 | 修复版本可用后，与报告者约定日期 |

修复发布前，报告者可获得补丁的预览验证机会；致谢方式（可匿名）与报告者协商确定。

## 🔗 供应链与门禁

以下门禁已在仓库中实际配置并执行：

| 门禁 | 位置 | 内容 |
|------|------|------|
| 格式与 Lint | `ci.yml` · `fmt` / `clippy` | `cargo fmt --check`；`cargo clippy -- -D warnings`（零告警） |
| 编译与特性矩阵 | `ci.yml` · `check` / `feature-redis-tier` / `feature-minimal`；`feature-matrix.yml` | 含 Redis 分层与最小特性组合的编译检查 |
| 测试 | `ci.yml` · `test` / `test-critical-combinations` | 全量测试与关键特性组合测试 |
| 依赖漏洞审计 | `ci.yml` · `security`；`dependency-check.yml` | `cargo audit`（读取 `.cargo/audit.toml`）、`cargo deny check` |
| 静态安全分析 | `codeql.yml` | CodeQL Rust 分析 |
| 覆盖率门禁 | `ci.yml` · `coverage` | `cargo llvm-cov --features full --workspace --fail-under-lines 90` |
| 本地钩子 | `.pre-commit-config.yaml` | `detect-private-key`、`detect-secrets`、`typos`、`cargo fmt` / `check` / `clippy`（local hooks） |
| 依赖与许可证治理 | `deny.toml`、`.cargo/audit.toml` | 许可证 / banned / advisory 检查与审计豁免 |
| 依赖更新 | `.github/dependabot.yml` | cargo / github-actions 生态位每周跟踪 |
| 不安全代码约束 | `src/lib.rs` | `#![deny(unsafe_code)]` |

### 已知豁免（透明披露）

`.cargo/audit.toml` 与 `deny.toml` 同口径记录以下两条 RustSec 豁免，均为**依赖树可达但不可利用/无升级路径**的形态：

- `RUSTSEC-2025-0141`（bincode 被作者标记 unmaintained）：已升级至 2.0.1，仅在 `serde-bincode` 特性下用于本地缓存条目序列化，无网络输入面；出路为 `postcard`（已在依赖中）。
- `RUSTSEC-2024-0449`（unsafe-libyaml unmaintained）：经测试依赖 `limiteron`/`inklog`/`confers` 的 `serde_yaml_ng` 传递入图；源头豁免与 `serde-saphyr` 迁移裁决在 `limiteron` 仓库跟踪。

豁免不构成安全行为变更；依赖故障与降级语义见 [🏗️ 架构文档](ARCHITECTURE.md)。

## ✅ 使用侧安全建议

- 生产环境按最小特性集启用后端，避免暴露未使用的连接面；连接串与凭据一律来自环境变量或密钥服务，不写入配置文件明文。
- 缓存 key 由不可信输入构成时，先做长度与字符集约束再入表（`validate_redis_key` 与 `redact_value` / `redact_connection_string` 见 `example_security` / `example_security_validation` 示例）。
- 关注 `cargo audit` 与 `cargo deny` 的输出，保持依赖与工具链更新。

## 📚 相关文档

- [🤝 贡献指南](CONTRIBUTING.md) — 开发环境与提交流程
- [📜 更新日志](CHANGELOG.md) — 安全修复记录
- [📖 用户指南](USER_GUIDE.md) — 配置与安全用法
- [🏠 README](../README.md) — 能力清单与安全内建说明

---

维护人：Kirky.X。
