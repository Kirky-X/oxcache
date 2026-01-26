#!/bin/bash

# Pre-commit Hook: Secret Detection
# ====================================
# Detects accidental secret/key exposure

set -e

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/common.sh"

log_info "Checking for accidental secrets..."

# Check if detect-secrets is available
if command -v detect-secrets &> /dev/null; then
    if detect-secrets scan --baseline .secrets.baseline 2>/dev/null | \
       jq -e '.results | keys | length == 0' 2>/dev/null; then
        log_success "No secrets detected"
        exit 0
    else
        print_header "可能检测到敏感信息"
        
        detect-secrets scan --baseline .secrets.baseline 2>/dev/null | \
            jq -r '.results | to_entries[] | "\n\(.key):\n\(.value | .[] | "  - \(.type) at line \(.line_number)")"' | head -30 || true
        
        echo ""
        echo "🔧 解决方案:"
        echo ""
        echo "  1. 【验证是否为真阳性】检查检测到的内容是否真是敏感信息"
        echo ""
        echo "  2. 【如果是误报】添加到基线:"
        echo "     detect-secrets scan --baseline .secrets.baseline"
        echo "     # 在.secrets.baseline中为该行添加\"\"false_positive\": true\""
        echo ""
        echo "  3. 【如果是真敏感】立即从代码中移除,并轮换相关密钥"
        echo ""
        
        echo "📚 敏感信息类型:"
        echo "  - API密钥、密码、令牌"
        echo "  - 私钥、证书"
        echo "  - 数据库连接字符串"
        echo "  - AWS/GCP等云服务凭证"
        echo ""
        
        exit 1
    fi
elif [ -f ".secrets.baseline" ]; then
    # Fallback to git-secrets if available
    if command -v git-secrets &> /dev/null; then
        git secrets --baseline ".secrets.baseline" || log_warning "检测到潜在问题,请手动检查"
    else
        log_warning "未配置密钥检测工具(跳过)"
        echo "   建议安装: pip install detect-secrets"
        echo "   初始化: detect-secrets scan > .secrets.baseline"
    fi
else
    log_warning "未配置密钥检测工具(跳过)"
    echo "   建议安装: pip install detect-secrets"
    echo "   初始化: detect-secrets scan > .secrets.baseline"
fi

exit 0
