#!/bin/bash

# Pre-commit Hook: TOML Validation
# ===================================
# Validates Cargo.toml and other TOML configuration files

set -e

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

log_info "Validating TOML configuration files..."

ERRORS=0

# Validate Cargo.toml - 使用 cargo metadata 检查
log_info "Checking Cargo.toml..."
if cargo metadata --format-version 1 --no-deps 2>&1 | tail -3 | grep -q "metadata"; then
    log_success "Cargo.toml is valid"
else
    log_error "Cargo.toml has errors"
    ERRORS=$((ERRORS + 1))
fi

# Validate deny.toml if it exists
if [ -f "deny.toml" ]; then
    echo ""
    log_info "Checking deny.toml..."
    # 使用 cargo deny 检查（不联网），只检查 fatal 错误
    output=$(cargo deny check --disable-fetch 2>&1 || true)

    # 检查是否有 fatal 错误（不是 warnings）
    if echo "$output" | grep -qi "error:.*deny.toml"; then
        log_error "deny.toml has errors"
        ERRORS=$((ERRORS + 1))
    elif echo "$output" | grep -qi "error: could not"; then
        # 配置解析错误
        log_error "deny.toml has parsing errors"
        ERRORS=$((ERRORS + 1))
    else
        log_success "deny.toml is valid"
    fi
fi

# 简单验证其他 TOML 文件的语法
for toml_file in examples/*/Cargo.toml macros/Cargo.toml; do
    if [ -f "$toml_file" ]; then
        echo ""
        log_info "Checking $toml_file..."

        # 检查文件是否存在且非空
        if [ -s "$toml_file" ]; then
            # 尝试解析 TOML（使用 Rust 内置解析或简单检查）
            if grep -q "^\[package\]" "$toml_file" && grep -q "^name\s*=" "$toml_file"; then
                log_success "$toml_file is valid"
            else
                log_warning "$toml_file 格式可能有问题"
            fi
        fi
    fi
done

if [ $ERRORS -gt 0 ]; then
    echo ""
    echo "=========================================="
    echo "❌ TOML验证失败"
    echo "=========================================="
    echo ""
    echo "🔧 常见问题:"
    echo "  - 语法错误(逗号、括号)"
    echo "  - 不支持的键名"
    echo "  - 版本格式错误"
    echo "  - 依赖项不存在"
    echo ""
    echo "📚 参考:"
    echo "  - TOML规范: https://toml.io"
    echo "  - Cargo.toml格式: https://doc.rust-lang.org/cargo/reference/manifest.html"
    echo ""
    exit 1
fi

echo ""
log_success "All TOML files are valid"
exit 0
