# 安全策略 Security Policy

## 支持的版本

| 版本 | 支持状态 |
|---|---|
| 最新稳定版 | ✅ 完整支持 |
| 最新 rc/预发布版 | ⚠️ 尽力支持 |
| 更早版本 | ❌ 不支持 |

## 报告漏洞

**请勿通过公开 issue 报告安全漏洞。**

请使用 GitHub 的 [Security Advisories](https://github.com/Kirky-X/oxcache/security/advisories/new) 私密披露通道提交报告("Report a vulnerability")。

报告请包含:

- 受影响的版本与 feature 组合
- 复现步骤或最小复现代码
- 影响评估(CWE 分类如可判断)
- 可行的临时缓解措施(如有)

## 响应承诺

- **48 小时内**确认收到
- **7 天内**给出初步评估(接受/拒绝/需补充信息)
- 修复发布前,报告者将获得补丁的预览验证机会
- 修复发布后,在 CHANGELOG 与 advisory 中致谢报告者(可匿名)

## 安全设计约定

本仓库遵循的安全工程基线:

- `#![forbid(unsafe_code)]`(如 crate 适用)
- 零告警门槛:`cargo clippy -D warnings`、`cargo deny check`、`cargo audit` 全部通过
- 密钥扫描(pre-commit detect-secrets)与 CI 安全检查常开
- 源码、示例、测试中不写入可用凭据字面量;配置一律从环境变量或密钥服务读取
