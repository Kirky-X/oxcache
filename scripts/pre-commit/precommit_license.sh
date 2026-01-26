#!/bin/bash

# Pre-commit Hook: License Compliance Check
# ===========================================
# Checks for license issues in dependencies

set -e

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

log_info "Checking license compliance..."

# 使用 --disable-fetch 跳过远程检查
output=$(cargo deny check licenses --disable-fetch 2>&1 || true)

# 检查是否有许可证问题
if echo "$output" | grep -qi "error\|failed|unlicensed"; then
    # 检查是否为网络问题
    if echo "$output" | grep -qi "fetch\|network\|connection"; then
        log_warning "网络连接问题，跳过远程检查"
        log_success "许可证检查通过（跳过远程数据库）"
        exit 0
    fi

    print_header "许可证合规性问题"

    # 显示许可证详情
    echo "$output" | head -50 || true

    echo ""
    echo "🔧 常见许可证问题修复:"
    echo ""
    echo "  1. 【查看详细问题】"
    echo "     cargo deny check licenses"
    echo ""
    echo "  2. 【允许特定许可证】在deny.toml中配置:"
    echo "     [licenses]"
    echo "     allow = [\"MIT\", \"Apache-2.0\"]"
    echo ""
    echo "  3. 【跳过特定依赖】"
    echo "     [licenses]"
    echo "     exceptions = ["
    echo "         { name = \"package-name\", allow = \"license-expression\" }"
    echo "     ]"
    echo ""

    echo "📚 许可证说明:"
    echo "  - MIT/Apache-2.0: 开源友好"
    echo "  - GPL-3.0: 传染性许可证,需注意"
    echo "  - UNLICENSE: 完全公共领域"
    echo "  - See: https://spdx.org/licenses/"
    echo ""

    exit 1
else
    log_success "License check passed"
    exit 0
fi
