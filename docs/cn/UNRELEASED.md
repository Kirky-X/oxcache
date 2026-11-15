# 未发布变更

> 本文件记录尚未发布的变更；已发布版本历史见 [docs/CHANGELOG.md](CHANGELOG.md)。

## Unreleased

## 开发路线图

> 本节内容迁自 README 开发路线图章节。

| 状态 | 事项 | 说明 |
|:----:|------|------|
| ✅ | **0.5.0 正式发布** | 已落地：0.5.0 于 2026-11-14 发布、0.5.1 于 2026-11-15 发布（tag 已推送，`release.yml` 自动发布到 crates.io） |
| 📋 | **下游版本传导** | dbnexus、inklog、limiteron、sdforge 同步对 oxcache 的依赖要求至 0.5（path + version 双写） |
| ✅ | **Valkey 集成测试环境门控** | 8 个 Valkey 与 6 个 Dragonfly 集成测试经 `container_or_skip` 门控：无 Docker 时带原因跳过，CI 置位 `OXCACHE_TEST_STRICT=1` 转 fail-closed；语义见 tests/README.md「容器可用性门控」 |
| ✅ | **质量审查留档项跟进** | 5 项全部收口（2 Medium + 1 Low 当轮修复，其余项于 2026-09-30 待办修复轮收口） |
