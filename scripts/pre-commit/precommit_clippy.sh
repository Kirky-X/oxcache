#!/bin/bash

# Pre-commit Hook: Clippy Static Analysis
# =========================================
# Runs cargo clippy and provides actionable error messages

set -e

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

log_info "Running Rust Clippy static analysis..."

# Run clippy and capture output
if cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tee /tmp/clippy_output.txt; then
    log_success "Clippy passed - no code quality issues found"
    exit 0
else
    print_header "Clippy发现代码质量问题"
    
    # Categorize and display warnings
    print_section "问题分类:"
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
    print_section "前10个警告示例:"
    
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
