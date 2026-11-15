# 🤝 Oxcache Contributing Guide

Thank you for your interest in the oxcache project! This document describes the tools, processes, and conventions required for participating in development.

## 🧰 Development Environment

- **Rust 1.97.1+** (edition 2024)
- **cargo**, **rustfmt**, **clippy** (installed with rustup)
- **pre-commit / lefthook**: installs the git hooks (see `.pre-commit-config.yaml` and `lefthook.yml` for configuration)

```bash
pip install pre-commit
pre-commit install
```

## 🔁 TDD Workflow

Every development task group follows this loop (Red → Green → Commit → Analyze → Next):

1. **Define the interface**: define the trait / API signatures first (`trait Xxx { ... }`), without implementation
2. **Write tests**: write unit tests against the interface (`#[cfg(test)] mod tests { ... }`); the tests should fail at this point (red)
3. **Write code**: implement the interface so the tests pass (green)
4. **Run tests**: `cargo test --features <relevant-feature> --lib`, make sure all tests pass
5. **Commit**: `git commit -m "feat(<module>): <description>"`
6. **codenexus analyze**: use the codenexus tool to analyze this task's impact on other modules and identify code that needs coordinated changes
7. **Continue to the next one**: adjust the follow-up tasks based on the analyze result, then start the next loop iteration

## 🪝 Git Hooks

The project uses pre-commit and lefthook, two layers of hooks, to carry the code-quality gates:

**pre-commit hooks** (commit stage):

| Hook | Description |
|------|-------------|
| `trailing-whitespace` / `end-of-file-fixer` | Removes trailing spaces, ensures a newline at end of file |
| `check-yaml` / `check-toml` | YAML / TOML syntax checks |
| `check-added-large-files` | Rejects files larger than 1 MB |
| `check-merge-conflict` | Detects unresolved merge-conflict markers |
| `detect-private-key` / `detect-secrets` | Detects private-key and secret leaks |
| `typos` | Spell check |
| `cargo-fmt` / `cargo-check` / `cargo-clippy` | Rust format, compile and lint (`-D warnings`) checks |

**lefthook** (per-stage gates):

| Stage | Content |
|-------|---------|
| pre-commit | `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo deny check`, private-key pattern scan |
| commit-msg | conventional commits format validation (`feat` / `fix` / `refactor` / `docs` etc.) |
| pre-push | `cargo audit` security audit, `cargo llvm-cov --fail-under-lines 90` line-coverage gate |

> **Never bypass hooks with `--no-verify`.** This is a security red line.

## 🔎 Code Quality

The project uses the following tools for code-quality review:

- **Code-quality review**: code simplification, architecture optimization, performance review
- **SAST security scanning**: 0 CRITICAL required before release
- **kueiku**: hard-bug analysis and root-cause location

## 📥 Pull Request Process

1. Create a feature branch from `main` (e.g. `feat/<feature>` or `fix/<issue>`)
2. Make sure all pre-commit hooks pass
3. Make sure the tests pass:

   ```bash
   cargo test --all-features
   # Narrow feature-combination compile checks (same as the CI feature-redis-tier / feature-minimal jobs;
   # the repo has no feature_redis_tier / feature_minimal test targets)
   cargo check -p oxcache --no-default-features --features redis-tier
   cargo check -p oxcache --no-default-features --features minimal
   ```

4. The PR description includes: what changed, test results, affected modules
5. After all CI checks pass, request review

## 🎨 Code Style

- Follow the existing codebase's naming and architecture conventions (snake_case function names, module organization, etc.)
- **Conciseness first**: write only the minimum code that solves the problem; no speculative features
- Dependencies must be feature-gated; never pull unneeded dependencies in through default features
- Chinese comments (consistent with the existing codebase)
- Errors must be explicit: thrown, returned, or reported; never swallowed or hidden behind defaults
- After changing documentation (`docs/` and README), run the documentation structure validation: the script lives at the base workspace root `scripts/base.py` (**not inside this repository**); run `python3 scripts/base.py docs check oxcache` from the base workspace root. External contributors may skip this (CI's Documentation job covers `cargo doc` as a fallback)

## ⌨️ Common Commands

```bash
# Build (all features)
cargo build --all-features

# Test (all features)
cargo test --all-features --lib

# Narrow feature-combination compile checks (same as CI; redis-tier is the canonical name, the legacy core remains as a compat alias)
cargo check -p oxcache --no-default-features --features redis-tier
cargo check -p oxcache --no-default-features --features minimal

# Format
cargo fmt

# Clippy check
cargo clippy --all-features -- -D warnings
```

## 📚 Related Documents

- [📖 User Guide](USER_GUIDE.md): feature explanations and hands-on tutorial from the user's perspective
- [📘 API Reference](API_REFERENCE.md): signatures of public APIs and the error-code table
- [🏗️ Architecture](ARCHITECTURE.md): module responsibilities, data flow and design decisions
- 🔒 Security document: read before changing key validation, the Lua sandbox, or redaction logic
- [🧪 Test Scenario Matrix](TEST_SCENARIOS.md): mapping from feature domains to test targets, with execution policy
- [📋 Changelog](CHANGELOG.md): add an entry whenever a user-visible capability is added
