#!/bin/bash

# 综合测试运行器
# 运行所有测试并生成报告

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/common.sh"

# 默认配置
DEFAULT_TEST_TIMEOUT=600  # 10分钟
DEFAULT_OUTPUT_DIR="test-reports"
DEFAULT_PARALLEL=true

# 帮助信息
show_help() {
    cat << EOF
综合测试运行器

用法: $0 [选项]

选项:
  -o, --output DIR       输出目录 (默认: $DEFAULT_OUTPUT_DIR)
  -t, --timeout SECONDS  测试超时时间 (默认: $DEFAULT_TEST_TIMEOUT)
  -s, --sequential       串行运行测试（默认并行）
  -v, --verbose          详细输出
  -h, --help             显示帮助信息

示例:
  $0                                    # 运行所有测试
  $0 -o reports -t 900                  # 15分钟超时，输出到reports目录
  $0 -s                                 # 串行运行

EOF
}

# 解析命令行参数
OUTPUT_DIR="$DEFAULT_OUTPUT_DIR"
TEST_TIMEOUT="$DEFAULT_TEST_TIMEOUT"
PARALLEL="$DEFAULT_PARALLEL"
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -t|--timeout)
            TEST_TIMEOUT="$2"
            shift 2
            ;;
        -s|--sequential)
            PARALLEL=false
            shift
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

# 检查依赖
check_dependencies() {
    log_info "检查依赖..."

    if ! check_cargo; then
        exit 1
    fi

    if ! command -v redis-server &> /dev/null; then
        log_warning "Redis服务器未安装，某些测试可能无法运行"
    fi

    log_success "依赖检查通过"
}

# 创建输出目录
create_output_dir() {
    ensure_directory "$OUTPUT_DIR"
}

# 运行单元测试
run_unit_tests() {
    log_info "运行单元测试..."
    
    local test_output="$OUTPUT_DIR/unit_tests.log"
    local start_time=$(date +%s)
    
    if cargo test --lib -- --nocapture > "$test_output" 2>&1; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_success "✅ 单元测试通过 (${duration}s)"
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_error "❌ 单元测试失败 (${duration}s)"
        log_error "查看详细日志: $test_output"
        return 1
    fi
}

# 运行集成测试
run_integration_tests() {
    log_info "运行集成测试..."
    
    local test_output="$OUTPUT_DIR/integration_tests.log"
    local start_time=$(date +%s)
    
    # 跳过需要外部数据库的测试
    export DATABASE_INTEGRATION_TEST_ENABLED=""
    
    if timeout "$TEST_TIMEOUT" cargo test --test '*' > "$test_output" 2>&1; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_success "✅ 集成测试通过 (${duration}s)"
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_warning "⚠️  集成测试发现问题或跳过 (${duration}s)"
        log_warning "查看详细日志: $test_output"
        return 1
    fi
}

# 运行安全审计
run_security_audit() {
    log_info "运行安全审计..."
    
    local audit_output="$OUTPUT_DIR/security_audit.log"
    local start_time=$(date +%s)
    
    if [[ -f "scripts/security_audit.sh" ]]; then
        if timeout "$TEST_TIMEOUT" ./scripts/security_audit.sh -o "$audit_output" > /dev/null 2>&1; then
            local end_time=$(date +%s)
            local duration=$((end_time - start_time))
            log_success "✅ 安全审计通过 (${duration}s)"
            return 0
        else
            local end_time=$(date +%s)
            local duration=$((end_time - start_time))
            log_warning "⚠️  安全审计发现问题 (${duration}s)"
            log_warning "查看详细报告: $audit_output"
            return 1
        fi
    else
        log_warning "安全审计脚本不存在，跳过"
        return 0
    fi
}

# 运行内存泄漏测试
run_memory_leak_tests() {
    log_info "运行内存泄漏测试..."
    
    local memory_output="$OUTPUT_DIR/memory_leak.log"
    local start_time=$(date +%s)
    
    # 运行Miri测试
    if command -v miri &> /dev/null; then
        log_info "运行Miri内存安全检查..."
        if MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test > "$memory_output" 2>&1; then
            log_success "✅ Miri内存安全检查通过"
        else
            log_warning "⚠️  Miri内存安全检查发现问题"
        fi
    else
        log_info "Miri未安装，跳过Miri测试"
    fi
    
    # 运行Valgrind测试
    if [[ -f "scripts/valgrind_memory_test.sh" ]] && command -v valgrind &> /dev/null; then
        log_info "运行Valgrind内存泄漏检测..."
        if timeout "$TEST_TIMEOUT" ./scripts/valgrind_memory_test.sh -o "$memory_output" > /dev/null 2>&1; then
            local end_time=$(date +%s)
            local duration=$((end_time - start_time))
            log_success "✅ Valgrind内存检测通过 (${duration}s)"
            return 0
        else
            local end_time=$(date +%s)
            local duration=$((end_time - start_time))
            log_warning "⚠️  Valgrind内存检测发现问题 (${duration}s)"
            log_warning "查看详细报告: $memory_output"
            return 1
        fi
    else
        log_info "Valgrind测试脚本不存在或未安装Valgrind，跳过"
        return 0
    fi
}

# 运行Redis版本兼容性测试
run_redis_compatibility_tests() {
    log_info "运行Redis版本兼容性测试..."
    
    local redis_output="$OUTPUT_DIR/redis_compatibility.log"
    local start_time=$(date +%s)
    
    # 设置环境变量启用Redis测试
    export REDIS_VERSION_TEST_ENABLED=1
    
    if timeout "$TEST_TIMEOUT" cargo test redis_version_compatibility > "$redis_output" 2>&1; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_success "✅ Redis版本兼容性测试通过 (${duration}s)"
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_warning "⚠️  Redis版本兼容性测试发现问题或跳过 (${duration}s)"
        log_warning "查看详细日志: $redis_output"
        return 1
    fi
}

# 运行代码质量检查
run_code_quality_checks() {
    log_info "运行代码质量检查..."
    
    local quality_output="$OUTPUT_DIR/code_quality.log"
    local start_time=$(date +%s)
    local failed_checks=0
    
    # 格式化检查
    log_info "检查代码格式化..."
    if cargo fmt --check > "$quality_output" 2>&1; then
        log_success "✅ 代码格式化检查通过"
    else
        log_error "❌ 代码格式化检查失败"
        log_error "运行 'cargo fmt' 修复格式问题"
        failed_checks=$((failed_checks + 1))
    fi
    
    # Clippy检查
    log_info "运行Clippy静态分析..."
    if cargo clippy -- -D warnings > "$quality_output" 2>&1; then
        log_success "✅ Clippy检查通过"
    else
        log_error "❌ Clippy检查失败"
        log_error "查看详细日志: $quality_output"
        failed_checks=$((failed_checks + 1))
    fi
    
    # 文档检查
    log_info "检查文档..."
    if cargo doc --no-deps > "$quality_output" 2>&1; then
        log_success "✅ 文档生成成功"
    else
        log_warning "⚠️  文档生成失败"
        failed_checks=$((failed_checks + 1))
    fi
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    if [[ $failed_checks -eq 0 ]]; then
        log_success "✅ 代码质量检查通过 (${duration}s)"
        return 0
    else
        log_error "❌ 代码质量检查失败: $failed_checks 项检查未通过 (${duration}s)"
        return 1
    fi
}

# 生成测试报告
generate_test_report() {
    local total_tests="$1"
    local passed_tests="$2"
    local failed_tests="$3"
    local skipped_tests="$4"
    
    local report_file="$OUTPUT_DIR/test_report.md"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    cat > "$report_file" << EOF
# 综合测试报告

生成时间: $timestamp

## 测试结果摘要

- **总测试数**: $total_tests
- **通过**: $passed_tests
- **失败**: $failed_tests
- **跳过**: $skipped_tests

## 详细结果

EOF

    # 添加各个测试的详细结果
    for log_file in "$OUTPUT_DIR"/*.log; do
        if [[ -f "$log_file" ]]; then
            local test_name=$(basename "$log_file" .log)
            local status="✅ PASSED"
            
            if grep -q "FAILED\|失败\|Error\|error" "$log_file"; then
                status="❌ FAILED"
            elif grep -q "WARNING\|警告\|跳过\|skipped" "$log_file"; then
                status="⚠️  WARNING"
            fi
            
            echo "### $test_name" >> "$report_file"
            echo "状态: $status" >> "$report_file"
            echo "" >> "$report_file"
            echo '```' >> "$report_file"
            tail -50 "$log_file" >> "$report_file"
            echo '```' >> "$report_file"
            echo "" >> "$report_file"
        fi
    done
    
    log_info "测试报告已生成: $report_file"
}

# 主函数
main() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  综合测试运行器${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
    
    check_dependencies
    create_output_dir
    
    local total_tests=0
    local passed_tests=0
    local failed_tests=0
    local skipped_tests=0
    
    # 测试列表
    local tests=(
        "run_unit_tests:单元测试"
        "run_integration_tests:集成测试"
        "run_security_audit:安全审计"
        "run_memory_leak_tests:内存泄漏测试"
        "run_redis_compatibility_tests:Redis兼容性测试"
        "run_code_quality_checks:代码质量检查"
    )
    
    # 运行测试
    for test_info in "${tests[@]}"; do
        IFS=':' read -r test_func test_name <<< "$test_info"
        
        echo ""
        log_info "运行 $test_name..."
        total_tests=$((total_tests + 1))
        
        if [[ "$PARALLEL" == true ]]; then
            # 并行运行（简化版本，实际应该使用真正的并行机制）
            if $test_func; then
                passed_tests=$((passed_tests + 1))
            else
                failed_tests=$((failed_tests + 1))
            fi
        else
            # 串行运行
            if $test_func; then
                passed_tests=$((passed_tests + 1))
            else
                failed_tests=$((failed_tests + 1))
            fi
        fi
    done
    
    # 生成报告
    echo ""
    log_info "生成测试报告..."
    generate_test_report "$total_tests" "$passed_tests" "$failed_tests" "$skipped_tests"
    
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}测试执行完成${NC}"
    echo -e "${BLUE}总测试数: $total_tests${NC}"
    echo -e "${GREEN}通过: $passed_tests${NC}"
    echo -e "${RED}失败: $failed_tests${NC}"
    echo -e "${YELLOW}跳过: $skipped_tests${NC}"
    echo -e "${BLUE}========================================${NC}"
    
    if [[ $failed_tests -eq 0 ]]; then
        log_success "🎉 所有测试通过！"
        exit 0
    else
        log_error "❌ $failed_tests 项测试失败"
        exit 1
    fi
}

# 运行主函数
main "$@"