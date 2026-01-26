#!/bin/bash

# Pre-commit Hook: Fast Unit Tests
# ==================================
# Runs fast unit tests, skipping slow integration tests

set -e

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/common.sh"

log_info "Running fast unit tests..."

# Run tests with timeout and limited output
if timeout 120 cargo test --lib --bins -- --test-threads=4 2>&1 | head -150; then
    log_success "Unit tests passed"
    exit 0
else
    TEST_EXIT=$?
    print_header "测试失败"
    
    echo "🔧 排查步骤:"
    echo ""
    echo "  1. 【查看完整输出】运行详细测试:"
    echo "     cargo test --lib -- --nocapture"
    echo ""
    echo "  2. 【检查编译错误】先确保代码能编译:"
    echo "     cargo check --all-features"
    echo ""
    echo "  3. 【查看具体失败】运行单个测试:"
    echo "     cargo test --lib test_name -- --nocapture"
    echo ""
    echo "  4. 【跳过测试】如果只是提交文档/配置:"
    echo "     git commit --no-verify"
    echo ""
    
    # Show test failures if available
    if [ -f /tmp/test_output.txt ]; then
        echo "📋 测试失败详情:"
        grep -A 5 "FAILED\|test result:" /tmp/test_output.txt | head -20
    fi
    
    echo ""
    echo "💡 注意: 此钩子只运行快速单元测试"
    echo "         完整的集成测试在CI中运行"
    echo ""
    
    exit $TEST_EXIT
fi
