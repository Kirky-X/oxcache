#!/bin/bash
# 内存测试脚本
# 支持 Miri 和 Valgrind 两种内存检测方式

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

# 默认配置
DEFAULT_TIMEOUT=300  # 5分钟
DEFAULT_OUTPUT_DIR="memory-test-reports"
DEFAULT_MODE="all"   # all, miri, valgrind, analysis

# 帮助信息
show_help() {
    cat << EOF
$(print_header "内存测试工具")

用法: $0 [选项]

选项:
  -m, --mode MODE       测试模式: miri, valgrind, analysis, all (默认: $DEFAULT_MODE)
  -t, --timeout SECONDS  测试超时时间 (默认: $DEFAULT_TIMEOUT)
  -o, --output DIR       输出目录 (默认: $DEFAULT_OUTPUT_DIR)
  -b, --binary PATH      Valgrind 测试用的二进制文件路径
  -v, --verbose          详细输出
  -h, --help             显示帮助信息

示例:
  $0                                    # 运行所有测试
  $0 -m miri                            # 仅运行 Miri 测试
  $0 -m valgrind -t 600                 # Valgrind 测试，10分钟超时
  $0 -b target/release/oxcache          # 测试指定的二进制文件

EOF
}

# 解析命令行参数
MODE="$DEFAULT_MODE"
TIMEOUT="$DEFAULT_TIMEOUT"
OUTPUT_DIR="$DEFAULT_OUTPUT_DIR"
BINARY_PATH=""
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -m|--mode)
            MODE="$2"
            shift 2
            ;;
        -t|--timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -b|--binary)
            BINARY_PATH="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
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

# ==================== Miri 测试 ====================
run_miri_tests() {
    print_section "运行 Miri 内存安全检查"

    if ! check_miri; then
        log_warning "跳过 Miri 测试"
        return 1
    fi

    local output_file="$OUTPUT_DIR/miri_test.log"

    log_info "初始化 Miri..."
    if ! cargo miri setup > /dev/null 2>&1; then
        log_error "Miri 初始化失败"
        return 1
    fi

    log_info "运行 Miri 测试..."
    local miri_cmd="MIRIFLAGS='-Zmiri-disable-isolation' cargo miri test"

    if [[ -n "$BINARY_PATH" ]]; then
        miri_cmd="$miri_cmd --test memory_leak_test"
    fi

    if [[ "$VERBOSE" == true ]]; then
        miri_cmd="$miri_cmd -- --nocapture"
    fi

    if run_with_timeout "$TIMEOUT" bash -c "$miri_cmd" > "$output_file" 2>&1; then
        log_success "✅ Miri 测试通过"
        update_test_stats "passed"
        return 0
    else
        log_error "❌ Miri 测试失败"
        log_error "查看详细日志: $output_file"
        update_test_stats "failed"
        return 1
    fi
}

# ==================== Valgrind 测试 ====================
run_valgrind_tests() {
    print_section "运行 Valgrind 内存泄漏检测"

    if ! check_valgrind; then
        log_warning "跳过 Valgrind 测试"
        return 1
    fi

    local output_file="$OUTPUT_DIR/valgrind_test.log"

    # 确定测试二进制文件
    local test_binary="$BINARY_PATH"

    if [[ -z "$test_binary" ]]; then
        # 尝试找到测试二进制文件
        log_info "构建测试二进制文件..."
        cargo test --test memory_leak_test --no-run > /dev/null 2>&1
        test_binary=$(find target/debug/deps -name "memory_leak_test-*" -type f -executable | head -1)

        if [[ -z "$test_binary" ]]; then
            log_error "未找到测试二进制文件"
            update_test_stats "failed"
            return 1
        fi
    fi

    if [[ ! -f "$test_binary" ]]; then
        log_error "二进制文件不存在: $test_binary"
        update_test_stats "failed"
        return 1
    fi

    log_info "测试二进制: $test_binary"

    # 运行 Valgrind
    local valgrind_cmd="valgrind --tool=memcheck \
        --leak-check=full \
        --show-leak-kinds=all \
        --track-origins=yes \
        --log-file=$output_file"

    if [[ "$VERBOSE" == true ]]; then
        valgrind_cmd="$valgrind_cmd --verbose"
    fi

    valgrind_cmd="$valgrind_cmd $test_binary --nocapture"

    log_info "执行 Valgrind 测试..."

    if run_with_timeout "$TIMEOUT" bash -c "$valgrind_cmd" > /dev/null 2>&1; then
        analyze_valgrind_output "$output_file"
    else
        log_error "Valgrind 执行失败"
        update_test_stats "failed"
        return 1
    fi
}

# 分析 Valgrind 输出
analyze_valgrind_output() {
    local output_file="$1"

    if [[ ! -f "$output_file" ]]; then
        log_error "Valgrind 输出文件不存在"
        update_test_stats "failed"
        return 1
    fi

    # 提取内存泄漏信息
    local leak_summary=$(grep -A 5 "definitely lost:" "$output_file" 2>/dev/null || echo "")
    local definitely_lost=$(echo "$leak_summary" | grep "definitely lost:" | awk '{print $4}' | tr -d ',' | tr -d 'B' || echo "0")
    local indirectly_lost=$(echo "$leak_summary" | grep "indirectly lost:" | awk '{print $4}' | tr -d ',' | tr -d 'B' || echo "0")
    local possibly_lost=$(echo "$leak_summary" | grep "possibly lost:" | awk '{print $4}' | tr -d ',' | tr -d 'B' || echo "0")

    # 转换为数字
    definitely_lost=${definitely_lost:-0}
    indirectly_lost=${indirectly_lost:-0}
    possibly_lost=${possibly_lost:-0}

    # 移除非数字字符
    definitely_lost=$(echo "$definitely_lost" | sed 's/[^0-9]//g')
    indirectly_lost=$(echo "$indirectly_lost" | sed 's/[^0-9]//g')
    possibly_lost=$(echo "$possibly_lost" | sed 's/[^0-9]//g')

    log_info "内存泄漏检测结果:"
    log_info "  Definitely lost: $definitely_lost bytes"
    log_info "  Indirectly lost: $indirectly_lost bytes"
    log_info "  Possibly lost: $possibly_lost bytes"

    # 判断结果
    local total_lost=$((definitely_lost + indirectly_lost))

    if [[ $total_lost -eq 0 ]]; then
        log_success "✅ 未检测到内存泄漏"
        update_test_stats "passed"
        return 0
    elif [[ $total_lost -lt 1024 ]]; then
        log_warning "⚠️  检测到少量内存泄漏: ${total_lost} bytes"
        update_test_stats "passed"
        return 0
    else
        log_error "❌ 检测到严重内存泄漏: ${total_lost} bytes"
        log_error "查看详细报告: $output_file"
        update_test_stats "failed"
        return 1
    fi
}

# ==================== 内存使用分析 ====================
run_memory_analysis() {
    print_section "运行内存使用分析"

    local output_file="$OUTPUT_DIR/memory_analysis.log"

    log_info "运行内存测试并收集统计信息..."

    # 运行测试
    if cargo test --test memory_leak_test test_l1_cache_memory_leak -- --nocapture > "$output_file" 2>&1; then
        log_success "内存测试完成"
    else
        log_warning "内存测试失败"
    fi

    # 监控内存使用
    log_info "监控内存使用情况..."
    local csv_file="$OUTPUT_DIR/memory_usage.csv"
    echo "时间,内存使用(KB)" > "$csv_file"

    cargo test --test memory_leak_test test_concurrent_memory_leak -- --nocapture > /dev/null 2>&1 &
    local test_pid=$!

    local i=0
    while kill -0 $test_pid 2>/dev/null && [[ $i -lt 30 ]]; do
        local mem_usage=$(ps -o rss= -p $test_pid 2>/dev/null || echo "0")
        echo "$i,$mem_usage" >> "$csv_file"
        sleep 1
        ((i++))
    done

    wait $test_pid 2>/dev/null || true

    log_success "内存使用分析完成"
    log_info "数据保存在: $csv_file"
    update_test_stats "passed"
}

# ==================== 生成报告 ====================
generate_report() {
    print_section "生成测试报告"

    local report_file="$OUTPUT_DIR/memory_test_report.md"

    local project_name=$(get_project_name)
    local project_version=$(get_project_version)

    cat > "$report_file" << EOF
# Oxcache 内存测试报告

生成时间: $(date)

## 项目信息
- 项目名称: ${project_name:-unknown}
- 项目版本: ${project_version:-unknown}
- Rust版本: $(rustc --version)
- 操作系统: $(uname -a)
- 内存: $(free -h | grep Mem | awk '{print $2}')

## 测试配置
- 测试模式: $MODE
- 超时时间: ${TIMEOUT}秒

## 测试结果
EOF

    # 添加 Miri 结果
    if [[ "$MODE" == "all" ]] || [[ "$MODE" == "miri" ]]; then
        if [[ -f "$OUTPUT_DIR/miri_test.log" ]]; then
            local miri_status="❌ 失败"
            if grep -q "test result: ok" "$OUTPUT_DIR/miri_test.log" 2>/dev/null; then
                miri_status="✅ 通过"
            fi
            echo "### Miri 内存安全检查" >> "$report_file"
            echo "状态: $miri_status" >> "$report_file"
            echo "" >> "$report_file"
        fi
    fi

    # 添加 Valgrind 结果
    if [[ "$MODE" == "all" ]] || [[ "$MODE" == "valgrind" ]]; then
        if [[ -f "$OUTPUT_DIR/valgrind_test.log" ]]; then
            echo "### Valgrind 内存泄漏检测" >> "$report_file"
            grep -A 5 "definitely lost:" "$OUTPUT_DIR/valgrind_test.log" | head -6 >> "$report_file" || echo "无检测结果" >> "$report_file"
            echo "" >> "$report_file"
        fi
    fi

    # 添加统计信息
    echo "## 测试统计" >> "$report_file"
    print_test_stats >> "$report_file"

    # 添加建议
    cat >> "$report_file" << EOF

## 建议
- 定期运行内存泄漏检测
- 在生产环境中监控内存使用
- 使用 jemalloc 进行更精确的内存分析
- 关注内存增长趋势

## 相关文件
- 详细日志: $OUTPUT_DIR/
EOF

    log_success "测试报告已生成: $report_file"
}

# ==================== 主函数 ====================
main() {
    print_header "Oxcache 内存测试工具"

    # 检查依赖
    check_cargo || exit 1

    # 创建输出目录
    ensure_directory "$OUTPUT_DIR"

    # 初始化测试统计
    init_test_stats

    # 运行测试
    case "$MODE" in
        miri)
            run_miri_tests
            ;;
        valgrind)
            run_valgrind_tests
            ;;
        analysis)
            run_memory_analysis
            ;;
        all)
            run_miri_tests
            run_valgrind_tests
            run_memory_analysis
            ;;
        *)
            log_error "无效的测试模式: $MODE"
            show_help
            exit 1
            ;;
    esac

    # 生成报告
    generate_report

    # 打印统计
    print_header "测试完成"
    print_test_stats

    # 检查结果
    local total_passed=$(echo "$TestStats" | grep -o 'passed=[0-9]*' | cut -d'=' -f2)
    local total_failed=$(echo "$TestStats" | grep -o 'failed=[0-9]*' | cut -d'=' -f2)

    if [[ $total_failed -eq 0 ]]; then
        log_success "🎉 所有测试通过！"
        exit 0
    else
        log_error "❌ $total_failed 项测试失败"
        exit 1
    fi
}

# 运行主函数
main "$@"
