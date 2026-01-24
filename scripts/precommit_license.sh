#!/bin/bash

# Pre-commit Hook: License Compliance Check
# ===========================================
# Checks for license issues in dependencies

set -e

echo "Checking license compliance..."

# Use cargo-deny to check licenses
if cargo deny check licenses 2>&1 | grep -q "check licenses passed\|no restrictions"; then
    echo ""
    echo "✅ License check passed"
    exit 0
else
    echo ""
    echo "=========================================="
    echo "⚠️  许可证合规性问题"
    echo "=========================================="
    echo ""
    
    # Show license details
    cargo deny check licenses 2>&1 | head -50 || true
    
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
fi
