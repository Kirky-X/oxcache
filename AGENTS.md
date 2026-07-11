# Agents Guide

## Overview

oxcache 是高性能 Rust 双层缓存库（L1 Moka + L2 Redis），提供统一的 `Cache<K, V>` 接口，支持 `#[cached]` 宏零侵入式缓存、同步/异步双 API 路径、per-entry TTL、布隆过滤器负查询过滤、批量写入等特性。edition 2024，rust 1.85+，MIT 许可证。

## Project Structure

```
oxcache/
├── src/
│   ├── backend/          # 缓存后端实现
│   │   └── memory/       # L1 内存后端（Moka + DashMap）
│   ├── cache/            # 核心 Cache<K,V> API
│   │   ├── api/          # Cache 操作接口
│   │   └── builder/      # CacheBuilder 构建器
│   ├── config/           # 配置模块
│   ├── core/             # 核心类型与事件
│   ├── features/         # 可选功能模块
│   │   └── bloom_filter/ # 布隆过滤器后端
│   ├── i18n/             # ICU4X 国际化（feature-gated）
│   ├── infra/            # 基础设施
│   │   ├── metrics/      # OpenTelemetry 指标
│   │   └── serialization/# 序列化
│   ├── integrations/     # 外部框架适配器
│   │   └── kit/          # trait-kit 集成
│   ├── security/         # Redis 安全验证
│   ├── testing/          # 测试工具（仅 test）
│   ├── traits/           # CacheKey trait
│   ├── utils/            # 工具函数
│   ├── lib.rs            # crate 入口
│   ├── error.rs          # 错误类型
│   └── internal.rs       # #[cached] 宏内部支持
├── macros/               # oxcache_macros proc-macro crate
│   └── src/
├── tests/                # 集成测试
│   ├── feature_core.rs   # core 特性窄测试
│   ├── feature_minimal.rs# minimal 特性窄测试
│   └── ...               # 其他集成测试
├── benches/              # 基准测试
├── examples/             # 示例代码
├── docs/                 # 文档
└── .github/workflows/    # CI 配置
```

## Where to Look

| 功能 | 位置 |
|------|------|
| 核心缓存 API | `src/cache/` |
| L1 内存后端 | `src/backend/memory/` |
| L2 Redis 后端 | `src/backend/`（feature-gated: `redis`） |
| `#[cached]` 宏 | `macros/src/`（feature-gated: `macros`） |
| 序列化 | `src/infra/serialization/` |
| 指标 | `src/infra/metrics/` |
| 布隆过滤器 | `src/features/bloom_filter/`（feature-gated: `bloom-filter`） |
| 安全验证 | `src/security/` |
| 错误类型 | `src/error.rs` |
| 公共 API 导出 | `src/lib.rs` |

## Conventions

- **edition 2024**，rust 1.85+，MIT License
- 依赖必须通过 feature 门控，禁止使用默认特性引入不必要的依赖
- TDD 开发流程（Red → Green → Commit → Analyze → Next）
- 中文注释（与现有代码库一致）
- 错误必须显性化：抛出、返回或上报，严禁吞掉
- 禁止 `unsafe`（`#![deny(unsafe_code)]`，edition 2024 中 `set_var`/`remove_var` 等需 `unsafe` 块）
- pre-commit hooks：`no-commit-to-branch` 保护 `main`/`master`，Rust 检查在 pre-push 阶段

## Commands

```bash
# 构建（全特性）
cargo build --all-features

# 测试（全特性，仅 lib）
cargo test --all-features --lib

# 窄特性测试
cargo test --no-default-features --features core --test feature_core
cargo test --no-default-features --features minimal --test feature_minimal

# 格式化
cargo fmt

# Clippy 检查
cargo clippy --all-features -- -D warnings
```

---

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **oxcache** (3687 symbols, 8235 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/oxcache/context` | Codebase overview, check index freshness |
| `gitnexus://repo/oxcache/clusters` | All functional areas |
| `gitnexus://repo/oxcache/processes` | All execution flows |
| `gitnexus://repo/oxcache/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
