#!/usr/bin/env bash
# Copyright © 2026 Kirky.X. All rights reserved.
# Pre-commit hook: 终极 7 阶段检查（Pre-flight → Format → Static Analysis → Dependency → Doc → Spell → Test）
# 触发：git commit
# 标准：零错误、零告警、全检查、自动修复
# 可选工具优雅降级 — 未安装则黄字跳过，不阻塞提交

set -euo pipefail
cd "$(git rev-parse --show-toplevel)" || exit 1

# ── ANSI 颜色 ──────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

ERROR_COUNT=0
ERROR_DETAILS=""

banner() {
    local phase="$1"
    local title="$2"
    echo ""
    echo "${BLUE}━━━ Phase ${phase} — ${title} ━━━${NC}"
}

check_pass() {
    local label="$1"
    echo "  ${GREEN}✓${NC} ${label}"
}

check_fail() {
    local label="$1"
    echo "  ${RED}✗${NC} ${label}"
    ERROR_COUNT=$((ERROR_COUNT + 1))
    ERROR_DETAILS="${ERROR_DETAILS}  ${RED}✗${NC} ${label}\n"
}

check_skip() {
    local label="$1"
    echo "  ${YELLOW}⚠${NC} ${label} — 跳过（未安装）"
}

network_safe() {
    local label="$1"
    shift
    local rc=0
    local out
    out=$("$@" 2>&1) || rc=$?
    if [ $rc -ne 0 ]; then
        if echo "$out" | grep -qi "error\|Could not\|Connection refused\|timeout\|dns"; then
            echo "  ${YELLOW}⚠${NC} ${label} — 网络异常，跳过"
            echo "$out" | head -5
        else
            check_fail "${label}"
            echo "$out"
        fi
    else
        check_pass "${label}"
    fi
}

echo "${CYAN}${BOLD}═══════════════════════════════════════════════${NC}"
echo "${CYAN}${BOLD}  Oxcache Pre-Commit 终极 7 阶段检查${NC}"
echo "${CYAN}${BOLD}═══════════════════════════════════════════════${NC}"

# ═══════════════════════════════════════════════════════════════════
# Phase 0 — Pre-flight（极速预检）
# ═══════════════════════════════════════════════════════════════════
banner "0" "Pre-flight（极速预检）"

# 0a — 空白字符 & 合并冲突标记
if git diff --check --cached 2>&1; then
    check_pass "git diff --check — 无空白字符 / 合并冲突"
else
    check_fail "git diff --check — 存在空白字符或合并冲突标记"
fi

# 0b — 文件大小检查（>1MB 拒绝）
oversized=0
while IFS= read -r -d '' file; do
    if [ -f "$file" ]; then
        size=$(stat -c%s "$file" 2>/dev/null || echo 0)
        if [ "$size" -gt 1048576 ]; then
            echo "  ${RED}✗${NC} 文件过大（$(( size / 1024 / 1024 ))MB）: ${file}"
            echo "    请使用 git-lfs 跟踪此文件"
            oversized=1
        fi
    fi
done < <(git diff --cached --name-only -z 2>/dev/null)

if [ "$oversized" -eq 1 ]; then
    check_fail "存在 >1MB 的暂存文件"
else
    check_pass "文件大小均在 1MB 以内"
fi

# ═══════════════════════════════════════════════════════════════════
# Phase 1 — Format
# ═══════════════════════════════════════════════════════════════════
banner "1" "Format（格式检查 + 自动修复）"

# 1a — cargo fmt
if cargo fmt --check 2>&1; then
    check_pass "cargo fmt — 格式正确"
else
    echo "  ${YELLOW}⚠ 格式不一致，自动修复...${NC}"
    cargo fmt 2>&1
    if cargo fmt --check 2>&1; then
        check_pass "cargo fmt — 已自动修复"
    else
        check_fail "cargo fmt — 自动修复失败，需手动处理"
    fi
fi

# ═══════════════════════════════════════════════════════════════════
# Phase 2 — Static Analysis
# ═══════════════════════════════════════════════════════════════════
banner "2" "Static Analysis（静态分析 + 自动修复）"

# 2a — cargo check（全目标+全特性）
echo "  ${CYAN}▶${NC} cargo check --all-targets --all-features..."
if cargo check --all-targets --all-features 2>&1; then
    check_pass "cargo check — 编译通过"
else
    check_fail "cargo check — 编译错误"
fi

# 2b — cargo clippy 自动修复
echo "  ${CYAN}▶${NC} cargo clippy --fix（自动修复可修复 lint）..."
cargo clippy --fix --allow-dirty --allow-staged 2>&1 || true

# 2c — cargo clippy 严格零告警
if cargo clippy --all-targets --all-features -- -D warnings 2>&1; then
    check_pass "cargo clippy — 零告警"
else
    check_fail "cargo clippy — 存在告警，需手动修复"
fi

# 2d — 检查 unused dependencies（可选）
banner "2d" "Unused Dependencies（未使用依赖检查）"
if cargo machete --version &>/dev/null; then
    if cargo machete 2>&1; then
        check_pass "cargo machete — 无未使用依赖"
    else
        check_fail "cargo machete — 存在未使用依赖"
    fi
else
    check_skip "cargo machete"
fi

# ═══════════════════════════════════════════════════════════════════
# Phase 3 — Dependency Checks
# ═══════════════════════════════════════════════════════════════════
banner "3" "Dependency Checks（依赖检查）"

# 3a — cargo sort（可选）
if cargo sort --version &>/dev/null; then
    if cargo sort --check 2>&1; then
        check_pass "cargo sort — 依赖排序正确"
    else
        check_fail "cargo sort — 依赖排序错误"
    fi
else
    check_skip "cargo sort"
fi

# 3b — cargo audit（可选，网络失败不阻塞）
if cargo audit --version &>/dev/null; then
    network_safe "cargo audit" cargo audit
else
    check_skip "cargo audit"
fi

# 3c — cargo deny（可选，需 deny.toml 存在）
if cargo deny --version &>/dev/null; then
    if [ -f "deny.toml" ]; then
        if cargo deny check 2>&1; then
            check_pass "cargo deny — 依赖合规"
        else
            check_fail "cargo deny — 依赖不合规"
        fi
    else
        echo "  ${YELLOW}⚠${NC} deny.toml 不存在，跳过 cargo deny"
    fi
else
    check_skip "cargo deny"
fi

# ═══════════════════════════════════════════════════════════════════
# Phase 4 — Documentation
# ═══════════════════════════════════════════════════════════════════
banner "4" "Documentation（文档检查）"

if RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features 2>&1; then
    check_pass "cargo doc — 文档编译通过，无警告"
else
    check_fail "cargo doc — 文档存在错误或警告"
fi

# ═══════════════════════════════════════════════════════════════════
# Phase 5 — Spell Check
# ═══════════════════════════════════════════════════════════════════
banner "5" "Spell Check（拼写检查）"

if typos --version &>/dev/null; then
    if typos 2>&1; then
        check_pass "typos — 拼写正确"
    else
        check_fail "typos — 存在拼写错误"
    fi
else
    check_skip "typos"
    echo "    安装: cargo install typos-cli"
fi

# ═══════════════════════════════════════════════════════════════════
# Phase 6 — Tests & Coverage
# ═══════════════════════════════════════════════════════════════════
banner "6" "Tests & Coverage（测试 + 覆盖率）"

# 6a — 全测试
echo "  ${CYAN}▶${NC} cargo test --all-features..."
if cargo test --all-features 2>&1; then
    check_pass "cargo test — 全部通过"
else
    check_fail "cargo test — 存在失败测试"
fi

# 6b — 覆盖率（可选，≥85%）
if cargo llvm-cov --version &>/dev/null; then
    echo "  ${CYAN}▶${NC} cargo llvm-cov --all-features --fail-under-lines 85..."
    if cargo llvm-cov --all-features --fail-under-lines 85 2>&1; then
        check_pass "cargo llvm-cov — 覆盖率 ≥85%"
    else
        check_fail "cargo llvm-cov — 覆盖率低于 85%"
    fi
else
    check_skip "cargo llvm-cov"
    echo "    安装: cargo install cargo-llvm-cov"
fi

# 6c — 清理覆盖率中间文件
echo "  ${CYAN}▶${NC} 清理 profraw/profdata 中间文件..."
profraw_count=$(find . -maxdepth 1 -name "*.profraw" -o -name "*.profdata" 2>/dev/null | wc -l)
if [ "$profraw_count" -gt 0 ]; then
    rm -f ./*.profraw ./*.profdata 2>/dev/null || true
    echo "  ${GREEN}✓${NC} 已清理 ${profraw_count} 个覆盖率中间文件"
else
    echo "  ${GREEN}✓${NC} 无覆盖率中间文件需要清理"
fi

# ═══════════════════════════════════════════════════════════════════
# Phase 7 — 结果汇总
# ═══════════════════════════════════════════════════════════════════
echo ""
echo "${CYAN}${BOLD}═══════════════════════════════════════════════${NC}"

if [ "$ERROR_COUNT" -eq 0 ]; then
    echo "${GREEN}${BOLD}  ✅ PASS — 全部 7 阶段检查通过${NC}"
    echo "${GREEN}${BOLD}═══════════════════════════════════════════════${NC}"
    exit 0
else
    echo "${RED}${BOLD}  ❌ FAIL — ${ERROR_COUNT} 个阶段存在失败${NC}"
    echo ""
    echo -e "${ERROR_DETAILS}"
    echo "${RED}${BOLD}  请修复上述问题后重新提交${NC}"
    echo "${RED}${BOLD}═══════════════════════════════════════════════${NC}"
    exit 1
fi
