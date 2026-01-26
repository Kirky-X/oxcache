#!/bin/bash

# Pre-commit Hook: Cargo Deny Security Audit
# ============================================
# Checks for security vulnerabilities using cargo-deny

set -e

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/common.sh"

log_info "Running cargo deny security audit..."

if cargo deny check 2>&1; then
    log_success "Dependency security check passed"
    exit 0
else
    print_header "发现依赖安全问题"
    
    echo "🔧 修复方法:"
    echo ""
    echo "  1. 【查看详细问题】运行以下命令:"
    echo "     cargo deny check"
    echo ""
    echo "  2. 【更新依赖】尝试更新有问题的包:"
    echo "     cargo update -p <package-name>"
    echo ""
    echo "  3. 【临时忽略】在deny.toml中添加:"
    echo "     [advisories]"
    echo "     ignore = [\"RUSTSEC-XXXX-XXXX\"]"
    echo ""
    echo "  4. 【查看修复版本】检查哪个版本修复了问题:"
    echo "     cargo tree -i <package-name>"
    echo ""
    
    echo "📚 参考资源:"
    echo "  - RUSTSEC咨询数据库: https://rustsec.org/"
    echo "  - 项目deny配置: deny.toml"
    echo "  - cargo-deny文档: https://github.comEmbarkStudios/cargo-deny"
    echo ""
    
    echo "💡 提示: 定期运行 'cargo update' 更新依赖可以减少安全问题"
    echo ""
    
    exit 1
fi
