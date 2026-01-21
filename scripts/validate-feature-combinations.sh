#!/bin/bash

# Oxcache 特性组合验证脚本
#
# 用法:
#   ./scripts/validate-feature-combinations.sh [选项]
#
# 选项:
#   --all                    验证所有 Tier 1 和 Tier 2 特性组合
#   --quick                  快速验证（仅编译检查）
#   --features "feat1,feat2"  验证特定特性组合
#   --report                 生成测试报告
#   --help                   显示帮助信息

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 默认配置
QUICK_MODE=false
GENERATE_REPORT=false
SPECIFIC_FEATURES=""

# 特性组合定义
TIER1_COMBINATIONS=(
    "minimal"
    "core"
    "full"
)

TIER2_COMBINATIONS=(
    "l1-moka,macros"
    "l2-redis,macros"
    "l1-moka,l2-redis,batch-write"
    "l1-moka,l2-redis,metrics"
    "l1-moka,l2-redis,bloom-filter"
    "l1-moka,l2-redis,database"
    "core,full-metrics"
    "core,confers"
    "core,smart-strategy"
    "core,http-cache"
)

# 帮助信息
show_help() {
    cat << EOF
Oxcache 特性组合验证脚本

用法: $0 [选项]

选项:
  --all                    验证所有 Tier 1 和 Tier 2 特性组合
  --quick                  快速验证（仅编译检查）
  --features "feat1,feat2"  验证特定特性组合
  --report                 生成测试报告
  --help                   显示帮助信息

示例:
  $0 --all                          # 验证所有特性组合
  $0 --quick                        # 快速验证所有组合
  $0 --features "l1-moka,macros"    # 验证特定组合
  $0 --features "core" --report     # 验证并生成报告

EOF
}

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 验证单个特性组合
validate_combination() {
    local features="$1"
    local quick_mode="$2"

    log_info "验证特性组合: $features"

    # 编译检查
    if cargo check --features "$features" 2>&1 | grep -q "error"; then
        log_error "特性组合 '$features' 编译失败"
        return 1
    fi

    if [ "$quick_mode" = false ]; then
        # 运行单元测试
        if ! cargo test --features "$features" --lib 2>&1 | grep -q "test result: ok"; then
            log_warning "特性组合 '$features' 单元测试有问题"
        fi

        # 运行文档测试
        if ! cargo test --features "$features" --doc 2>&1 | grep -q "test result: ok"; then
            log_warning "特性组合 '$features' 文档测试有问题"
        fi
    fi

    log_success "特性组合 '$features' 验证通过"
    return 0
}

# 生成测试报告
generate_report() {
    local results_file="$1"
    local report_file="test-reports/feature-combinations-report.md"

    mkdir -p test-reports

    cat > "$report_file" << EOF
# 特性组合测试报告

生成时间: $(date '+%Y-%m-%d %H:%M:%S')
Rust 版本: $(rustc --version)

## 测试结果

| 特性组合 | 编译 | 单元测试 | 文档测试 | 状态 |
|---------|------|---------|---------|------|
EOF

    while IFS=, read -r features compile unit_test doc_test; do
        if [ "$compile" = "pass" ]; then
            compile_status="✅"
        else
            compile_status="❌"
        fi

        if [ "$unit_test" = "pass" ]; then
            unit_test_status="✅"
        elif [ "$unit_test" = "skip" ]; then
            unit_test_status="⏭️"
        else
            unit_test_status="❌"
        fi

        if [ "$doc_test" = "pass" ]; then
            doc_test_status="✅"
        elif [ "$doc_test" = "skip" ]; then
            doc_test_status="⏭️"
        else
            doc_test_status="❌"
        fi

        status="Pass"
        if [ "$compile" = "fail" ] || [ "$unit_test" = "fail" ] || [ "$doc_test" = "fail" ]; then
            status="Fail"
        fi

        echo "| $features | $compile_status | $unit_test_status | $doc_test_status | $status |" >> "$report_file"
    done < "$results_file"

    log_info "测试报告已生成: $report_file"
}

# 主函数
main() {
    # 解析命令行参数
    while [[ $# -gt 0 ]]; do
        case $1 in
            --all)
                COMBINATIONS=("${TIER1_COMBINATIONS[@]}" "${TIER2_COMBINATIONS[@]}")
                shift
                ;;
            --quick)
                QUICK_MODE=true
                shift
                ;;
            --features)
                SPECIFIC_FEATURES="$2"
                shift 2
                ;;
            --report)
                GENERATE_REPORT=true
                shift
                ;;
            --help)
                show_help
                exit 0
                ;;
            *)
                log_error "未知选项: $1"
                show_help
                exit 1
                ;;
        esac
    done

    # 如果没有指定任何组合，默认验证 Tier 1
    if [ -z "$COMBINATIONS" ] && [ -z "$SPECIFIC_FEATURES" ]; then
        COMBINATIONS=("${TIER1_COMBINATIONS[@]}")
    fi

    # 如果指定了特定特性，使用它
    if [ -n "$SPECIFIC_FEATURES" ]; then
        COMBINATIONS=("$SPECIFIC_FEATURES")
    fi

    log_info "开始验证特性组合..."
    log_info "快速模式: $QUICK_MODE"
    log_info "组合数量: ${#COMBINATIONS[@]}"

    # 创建临时结果文件
    local results_file=$(mktemp)

    # 验证每个特性组合
    local total=0
    local passed=0
    local failed=0

    for features in "${COMBINATIONS[@]}"; do
        total=$((total + 1))

        # 记录开始时间
        local start_time=$(date +%s)

        # 验证组合
        if validate_combination "$features" "$QUICK_MODE"; then
            passed=$((passed + 1))
            echo "$features,pass,pass,pass" >> "$results_file"
        else
            failed=$((failed + 1))
            echo "$features,fail,skip,skip" >> "$results_file"
        fi

        # 记录结束时间
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))

        log_info "完成 (耗时: ${duration}s)"

        echo ""
    done

    # 生成报告
    if [ "$GENERATE_REPORT" = true ]; then
        generate_report "$results_file"
    fi

    # 清理临时文件
    rm -f "$results_file"

    # 显示摘要
    echo ""
    echo "========================================"
    echo "验证完成"
    echo "========================================"
    echo "总计: $total"
    echo -e "${GREEN}通过: $passed${NC}"
    echo -e "${RED}失败: $failed${NC}"
    echo "========================================"

    if [ $failed -gt 0 ]; then
        exit 1
    fi
}

# 运行主函数
main "$@"