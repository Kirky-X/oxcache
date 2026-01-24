#!/bin/bash

# Pre-commit Hook: TOML Validation
# ===================================
# Validates Cargo.toml and other TOML configuration files

set -e

echo "Validating TOML configuration files..."

ERRORS=0

# Validate Cargo.toml
echo "Checking Cargo.toml..."
if cargo validate-manifest 2>&1; then
    echo "✅ Cargo.toml is valid"
else
    echo "❌ Cargo.toml has errors"
    ERRORS=$((ERRORS + 1))
fi

# Validate deny.toml if it exists
if [ -f "deny.toml" ]; then
    echo ""
    echo "Checking deny.toml..."
    if cargo deny check config 2>&1; then
        echo "✅ deny.toml is valid"
    else
        echo "❌ deny.toml has errors"
        ERRORS=$((ERRORS + 1))
    fi
fi

# Validate other TOML files
for toml_file in Cargo.toml examples/*/Cargo.toml macros/Cargo.toml; do
    if [ -f "$toml_file" ]; then
        echo ""
        echo "Checking $toml_file..."
        if cargo validate-manifest --manifest-path "$toml_file" 2>&1; then
            echo "✅ $toml_file is valid"
        else
            echo "❌ $toml_file has errors"
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
echo "✅ All TOML files are valid"
exit 0
