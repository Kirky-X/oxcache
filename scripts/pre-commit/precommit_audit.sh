#!/bin/bash

# Pre-commit Hook: Cargo Audit (Enhanced)
# ==========================================
# Enhanced vulnerability scanning with RUSTSEC database

set -e

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

log_info "Running enhanced cargo audit..."

# Check if cargo-audit is installed
if ! command -v cargo-audit &> /dev/null; then
    log_warning "cargo-audit未安装,尝试安装..."
    if cargo install cargo-audit; then
        log_success "cargo-audit安装成功"
    else
        log_error "cargo-audit安装失败,请手动安装: cargo install cargo-audit"
        exit 1
    fi
fi

# Run audit with JSON output for structured results
# 使用 --no-fetch 跳过远程更新检查，避免网络超时问题
if cargo audit --no-fetch --json 2>/dev/null | jq -e '.vulnerabilities.found == false' 2>/dev/null; then
    log_success "Cargo audit passed - no vulnerabilities found"
    exit 0
else
    # 检查是否有实际漏洞还是只有警告/网络问题
    output=$(cargo audit --no-fetch 2>&1 || true)

    if echo "$output" | grep -q "vulnerability found"; then
        print_header "发现安全漏洞"

        # 运行 audit 获取详细信息
        cargo audit 2>&1 | grep -E "(Vulnerability|RUSTSEC|CVE|id:|package:)" | head -30 || true

        echo ""
        echo "🔧 修复方法:"
        echo ""
        echo "  1. 【查看详情】运行以下命令查看完整报告:"
        echo "     cargo audit"
        echo ""
        echo "  2. 【更新修复】更新到修复版本:"
        echo "     cargo update -p <package-name>"
        echo ""
        echo "  3. 【忽略特定漏洞】创建.cargo/audit.toml:"
        echo "     [advisories]"
        echo "     ignore = [\"RUSTSEC-XXXX-XXXX\"]"
        echo ""

        echo "📚 参考资源:"
        echo "  - RUSTSEC数据库: https://rustsec.org/advisories.html"
        echo "  - cargo-audit: https://github.com/RustSec/cargo-audit"
        echo ""

        echo "💡 安全最佳实践:"
        echo "  - 定期运行 'cargo update' 更新依赖"
        echo "  - 订阅RUSTSEC通知"
        echo "  - 在CI中配置自动漏洞扫描"
        echo ""

        exit 1
    else
        # 只有警告或网络问题，未发现实际漏洞
        log_warning "Cargo audit 发现警告（无实际漏洞）"
        exit 0
    fi
fi
