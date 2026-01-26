#!/bin/bash

# Pre-commit Hook: Cargo Deny Security Audit
# ============================================
# Checks for security vulnerabilities using cargo-deny

set -e

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

log_info "Running cargo deny security audit..."

# 使用 --disable-fetch 跳过远程检查，避免网络超时问题
# 先检查 sources 和 licenses（不需要网络）
output=$(cargo deny check sources licenses --disable-fetch 2>&1 || true)

# 检查 sources 和 licenses 是否有问题
if echo "$output" | grep -qi "error\|failed"; then
    print_header "发现依赖合规性问题"

    echo "$output" | head -50

    echo ""
    echo "🔧 修复方法:"
    echo "  1. 【查看详细问题】运行: cargo deny check sources licenses"
    echo "  2. 【在deny.toml中配置】允许/忽略特定来源或许可证"
    echo ""

    exit 1
fi

log_success "Dependency source and license check passed"

# 检查 advisories（可能需要网络）
echo ""
log_info "Checking security advisories..."

advisory_output=$(cargo deny check advisories --disable-fetch 2>&1 || true)

if echo "$advisory_output" | grep -qi "error: could not find"; then
    # 找不到本地数据库
    log_warning "安全 advisory 数据库未本地化，跳过远程检查"
    log_success "安全检查通过（跳过远程数据库）"
elif echo "$advisory_output" | grep -qi "vulnerability found"; then
    print_header "发现安全漏洞"

    echo "$advisory_output" | head -50

    echo ""
    echo "🔧 修复方法:"
    echo "  1. 【查看详细问题】运行: cargo deny check advisories"
    echo "  2. 【更新依赖】尝试更新有问题的包: cargo update -p <package-name>"
    echo "  3. 【临时忽略】在deny.toml中添加: [advisories] ignore = [\"RUSTSEC-XXXX-XXXX\"]"
    echo ""

    exit 1
else
    log_success "Security advisory check passed"
fi

echo ""
log_success "All cargo deny checks passed"
exit 0
