#!/usr/bin/env bash
# Copyright © 2026 Kirky.X. All rights reserved.
# 安装/卸载 Git Hooks（symlink 方式，Windows 兼容）
# 运行：bash scripts/hooks/setup-hooks.sh
# 卸载：bash scripts/hooks/setup-hooks.sh --uninstall

set -euo pipefail
cd "$(git rev-parse --show-toplevel)" || exit 1

HOOKS_DIR=".git/hooks"
SCRIPTS_HOOKS="scripts/hooks"

# ── 卸载模式 ──────────────────────────────────
if [ "${1:-}" = "--uninstall" ]; then
    if [ -L "$HOOKS_DIR/pre-commit" ] || [ -f "$HOOKS_DIR/pre-commit" ]; then
        rm -f "$HOOKS_DIR/pre-commit"
        echo "✓ pre-commit hook 已卸载"
    else
        echo "⚠ pre-commit hook 未安装"
    fi
    exit 0
fi

echo "安装 Oxcache Git Hooks..."
echo ""

# ── 平台检测（Windows 需要 cp 替代 ln -sf） ──
OS="$(uname -s)"
case "$OS" in
    MINGW*|MSYS*|CYGWIN*)
        echo "  ⚠ Windows 环境检测到，使用复制模式安装"
        INSTALL_CMD="cp"
        ;;
    *)
        INSTALL_CMD="ln -sf"
        ;;
esac

# ── 检查可选工具（仅提示，不阻塞安装） ─────────
echo "  ── 可选工具检查 ──"
MISSING_TOOLS=()

if ! cargo llvm-cov --version &>/dev/null; then
    MISSING_TOOLS+=("cargo-llvm-cov (代码覆盖率)")
fi

if ! cargo audit --version &>/dev/null; then
    MISSING_TOOLS+=("cargo-audit (安全审计)")
fi

if ! cargo deny --version &>/dev/null; then
    MISSING_TOOLS+=("cargo-deny (依赖合规)")
fi

if ! cargo sort --version &>/dev/null; then
    MISSING_TOOLS+=("cargo-sort (依赖排序)")
fi

if ! typos --version &>/dev/null; then
    MISSING_TOOLS+=("typos (拼写检查)")
fi

if [ ${#MISSING_TOOLS[@]} -gt 0 ]; then
    echo "  ⚠ 以下可选工具未安装（hook 会自动跳过对应检查）："
    for tool in "${MISSING_TOOLS[@]}"; do
        echo "      • $tool"
    done
    echo ""
    echo "  一键安装所有可选工具："
    echo "    cargo install cargo-llvm-cov cargo-audit cargo-deny cargo-sort"
    echo "    cargo install typos-cli"
    echo ""
fi

# ── 安装 pre-commit hook ─────────────────────
if [ -f "$SCRIPTS_HOOKS/pre-commit.sh" ]; then
    echo "  ── 安装 pre-commit hook ──"
    if [ "$INSTALL_CMD" = "ln -sf" ]; then
        ln -sf "../../$SCRIPTS_HOOKS/pre-commit.sh" "$HOOKS_DIR/pre-commit"
    else
        cp "$SCRIPTS_HOOKS/pre-commit.sh" "$HOOKS_DIR/pre-commit"
    fi
    chmod +x "$HOOKS_DIR/pre-commit"

    # 安装后验证
    if [ -f "$HOOKS_DIR/pre-commit" ] && [ -x "$HOOKS_DIR/pre-commit" ]; then
        echo "  ✓ pre-commit hook 已安装（$HOOKS_DIR/pre-commit）"
    else
        echo "  ✗ pre-commit hook 安装失败"
        exit 1
    fi
else
    echo "✗ $SCRIPTS_HOOKS/pre-commit.sh 不存在"
    exit 1
fi

echo ""
echo "✓ Hooks 安装完成"
echo ""
echo "  运行方式：git commit 时自动触发"
echo "  手动运行：bash scripts/hooks/pre-commit.sh"
echo "  卸载：    bash scripts/hooks/setup-hooks.sh --uninstall"
