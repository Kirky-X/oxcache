#!/bin/bash

# Pre-commit Hook: TOML Validation
# ===================================
# Validates Cargo.toml and other TOML configuration files

set -e

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/common.sh"

log_info "Validating TOML configuration files..."

ERRORS=0

# Validate Cargo.toml
log_info "Checking Cargo.toml..."
if cargo validate-manifest 2>&1; then
    log_success "Cargo.toml is valid"
else
    log_error "Cargo.toml has errors"
    ERRORS=$((ERRORS + 1))
fi

# Validate deny.toml if it exists
if [ -f "deny.toml" ]; then
    echo ""
    log_info "Checking deny.toml..."
    if cargo deny check config 2>&1; then
        log_success "deny.toml is valid"
    else
        log_error "deny.toml has errors"
        ERRORS=$((ERRORS + 1))
    fi
fi

# Validate other TOML files
for toml_file in Cargo.toml examples/*/Cargo.toml macros/Cargo.toml; do
    if [ -f "$toml_file" ]; then
        echo ""
        log_info "Checking $toml_file..."
        if cargo validate-manifest --manifest-path "$toml_file" 2>&1; then
            log_success "$toml_file is valid"
        else
            log_error "$toml_file has errors"
            ERRORS=$((ERRORS + 1))
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
