#!/bin/bash

# Pre-commit Hook: Secret Detection
# ====================================
# Detects accidental secret/key exposure

set -e

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

log_info "Checking for accidental secrets..."

# Check if detect-secrets is available
if command -v detect-secrets &> /dev/null; then
    if [ -f ".secrets.baseline" ]; then
        # 对于已存在的项目，基线中已记录的内容不再报警
        # 只检查是否有新增的高置信度 secrets
        high_confidence=$(detect-secrets scan --baseline .secrets.baseline 2>&1 | \
            jq -r '.results // {} | to_entries[] | select(.value[].type | test("AWS|Generic|Hex")) | .key' 2>/dev/null | head -5)

        if [ -n "$high_confidence" ]; then
            print_header "可能检测到高置信度敏感信息"

            echo "$high_confidence" | while read file; do
                echo "  - $file"
            done

            echo ""
            echo "🔧 解决方案:"
            echo "  请检查上述文件中的敏感信息"
            echo ""
            exit 1
        fi

        log_success "No high-confidence secrets detected"
        exit 0
    else
        # 没有基线文件，跳过检查
        log_warning "未配置密钥检测基线(跳过)"
        echo "   建议运行: detect-secrets scan > .secrets.baseline"
        exit 0
    fi
else
    log_warning "未安装 detect-secrets(跳过)"
    echo "   建议安装: pip install detect-secrets"
    exit 0
fi
