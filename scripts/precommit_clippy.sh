#!/bin/bash

# Pre-commit Hook: Clippy Static Analysis
# =========================================
# Runs cargo clippy and provides actionable error messages

set -e

echo "Running Rust Clippy static analysis..."

# Run clippy and capture output
if cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tee /tmp/clippy_output.txt; then
    echo ""
    echo "✅ Clippy passed - no code quality issues found"
    exit 0
else
    echo ""
    echo "=========================================="
    echo "🚨 Clippy发现代码质量问题"
    echo "=========================================="
    echo ""
    
    # Categorize and display warnings
    echo "🔍 问题分类:"
    echo ""
    
    # Extract and categorize warnings
    if grep -q "clippy::" /tmp/clippy_output.txt; then
        echo "📌 Clippy建议类型:"
        grep "clippy::" /tmp/clippy_output.txt | sort | uniq -c | sort -rn | head -10
        echo ""
    fi
    
    # Common fixes section
    echo "🔧 修复方法:"
    echo ""
    echo "  1. 【自动修复】运行以下命令尝试自动修复:"
    echo "     cargo clippy --workspace --fix"
    echo ""
    echo "  2. 【手动修复】查看详细警告:"
    echo "     cargo clippy --workspace --all-targets --all-features 2>&1 | grep -A 3 'warning:'"
    echo ""
    echo "  3. 【跳过检查】如果是误报,可以在代码中添加:"
    echo "     #[allow(clippy::lint_name)]"
    echo ""
    
    # Show first few warnings
    echo "📋 前10个警告示例:"
    echo ""
    grep -E "^.*warning:" /tmp/clippy_output.txt | head -10 | sed 's/^/   /'
    echo ""
    
    # Reference links
    echo "📚 参考文档:"
    echo "  - Clippy lint列表: https://rust-lang.github.io/rust-clippy/master/"
    echo "  - 跳过警告: https://doc.rust-lang.org/clippy/faq.html#how-can-i-allow-clippy-warnings"
    echo ""
    echo "💡 提示: 使用 'cargo clippy --fix' 可以自动修复大部分问题"
    echo ""
    
    exit 1
fi
