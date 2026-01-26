#!/bin/bash

# Run All Pre-commit Checks
# ==================================
# Executes all pre-commit hooks in sequence

set -e

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

log_info "Starting pre-commit checks..."

# 定义要运行的检查脚本
checks=(
    "audit.sh"
    "clippy.sh" 
    "deny.sh"
    "license.sh"
    "secrets.sh"
    "tests.sh"
    "toml.sh"
)

# 记录开始时间
start_time=$(date +%s)

# 运行每个检查
failed_checks=0
total_checks=${#checks[@]}

for check in "${checks[@]}"; do
    echo ""
    print_section "Running $check"
    
    if "$SCRIPT_DIR/$check"; then
        log_success "$check passed"
    else
        log_error "$check failed"
        ((failed_checks++))
    fi
done

# 计算总耗时
end_time=$(date +%s)
duration=$((end_time - start_time))

echo ""
print_header "Pre-commit Checks Summary"
echo "Total checks: $total_checks"
echo "Passed: $((total_checks - failed_checks))"
echo "Failed: $failed_checks"
echo "Duration: ${duration}s"

if [ $failed_checks -gt 0 ]; then
    log_error "Some checks failed"
    exit 1
else
    log_success "All pre-commit checks passed"
    exit 0
fi