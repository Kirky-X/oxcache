#!/usr/bin/env bash
# 综合测试运行器
# 运行所有测试、Redis 生命周期管理、覆盖率报告
#
# 用法:
#   ./scripts/tests/run_all_tests.sh                    # 运行全部测试
#   ./scripts/tests/run_all_tests.sh unit               # 仅单元测试
#   ./scripts/tests/run_all_tests.sh integration        # 仅集成测试
#   ./scripts/tests/run_all_tests.sh coverage           # 测试 + 覆盖率
#   ./scripts/tests/run_all_tests.sh clean              # 清理环境
#   ./scripts/tests/run_all_tests.sh -o reports -t 900  # 自定义输出和超时

set -euo pipefail

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REAL_ENV_DIR="$PROJECT_ROOT/tests/real_env"

# 默认配置
DEFAULT_TEST_TIMEOUT=600
DEFAULT_OUTPUT_DIR="test-reports"
DEFAULT_PARALLEL=true

# 解析命令行参数
OUTPUT_DIR="$DEFAULT_OUTPUT_DIR"
TEST_TIMEOUT="$DEFAULT_TEST_TIMEOUT"
PARALLEL="$DEFAULT_PARALLEL"
VERBOSE=false

# 显示帮助
show_help() {
    cat << EOF
综合测试运行器

用法: $0 [选项] [测试类型]

测试类型:
  unit          运行单元测试
  integration   运行集成测试
  e2e           运行端到端测试
  chaos         运行混沌测试
  security      运行安全审计
  memory        运行内存泄漏测试
  redis-comp    运行Redis兼容性测试
  quality       运行代码质量检查
  coverage      运行测试 + 覆盖率报告
  clean         清理测试环境
  all           运行全部（默认）

选项:
  -o, --output DIR       输出目录 (默认: $DEFAULT_OUTPUT_DIR)
  -t, --timeout SECONDS  测试超时时间 (默认: $DEFAULT_TEST_TIMEOUT)
  -s, --sequential       串行运行测试（默认并行）
  -v, --verbose          详细输出
  -h, --help             显示帮助信息

示例:
  $0                       # 运行所有测试
  $0 unit                  # 仅单元测试
  $0 integration           # 仅集成测试
  $0 coverage              # 测试 + 覆盖率
  $0 clean                 # 清理环境
  $0 -o reports -t 900     # 15分钟超时，输出到reports目录
EOF
}

while [[ $# -gt 0 ]]; do
    case $1 in
        -o|--output)    OUTPUT_DIR="$2"; shift 2 ;;
        -t|--timeout)   TEST_TIMEOUT="$2"; shift 2 ;;
        -s|--sequential) PARALLEL=false; shift ;;
        -v|--verbose)   VERBOSE=true; shift ;;
        -h|--help)      show_help; exit 0 ;;
        unit|integration|e2e|chaos|security|memory|redis-comp|quality|coverage|clean|all)
            TEST_TYPE="$1"; shift ;;
        *)  log_error "未知选项: $1"; show_help; exit 1 ;;
    esac
done

# 默认运行全部
TEST_TYPE="${TEST_TYPE:-all}"

# ==================== Redis 生命周期管理 ====================

start_redis() {
    log_info "启动 Redis 服务..."
    if [ -f "$REAL_ENV_DIR/docker-compose.yml" ]; then
        (cd "$REAL_ENV_DIR" && docker-compose up -d)
        log_info "等待 Redis 就绪..."
        sleep 5
        for i in {1..10}; do
            if docker exec oxcache-redis-test redis-cli ping 2>/dev/null | grep -q "PONG"; then
                log_success "Redis 已就绪"
                export REDIS_URL="redis://127.0.0.1:6379"
                export OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS
                return 0
            fi
            sleep 1
        done
        log_warning "Redis 未能在预期时间内就绪"
    fi
}

stop_redis() {
    log_info "停止 Redis 服务..."
    if [ -f "$REAL_ENV_DIR/docker-compose.yml" ]; then
        (cd "$REAL_ENV_DIR" && docker-compose down)
        log_success "Redis 服务已停止"
    fi
}

start_redis_cluster() {
    log_info "启动 Redis Cluster..."
    if [ -f "$REAL_ENV_DIR/docker-compose.cluster.yml" ]; then
        (cd "$REAL_ENV_DIR" && docker-compose -f docker-compose.cluster.yml up -d)
        log_info "等待 Redis Cluster 就绪..."
        sleep 15
        export REDIS_CLUSTER_AVAILABLE=1
        log_success "Redis Cluster 已启动"
    fi
}

stop_redis_cluster() {
    log_info "停止 Redis Cluster..."
    if [ -f "$REAL_ENV_DIR/docker-compose.cluster.yml" ]; then
        (cd "$REAL_ENV_DIR" && docker-compose -f docker-compose.cluster.yml down)
        log_success "Redis Cluster 已停止"
    fi
}

start_redis_sentinel() {
    log_info "启动 Redis Sentinel..."
    if [ -f "$REAL_ENV_DIR/docker-compose.sentinel.yml" ]; then
        (cd "$REAL_ENV_DIR" && docker-compose -f docker-compose.sentinel.yml up -d)
        log_info "等待 Redis Sentinel 就绪..."
        sleep 20
        export REDIS_SENTINEL_AVAILABLE=1
        log_success "Redis Sentinel 已启动"
    fi
}

stop_redis_sentinel() {
    log_info "停止 Redis Sentinel..."
    if [ -f "$REAL_ENV_DIR/docker-compose.sentinel.yml" ]; then
        (cd "$REAL_ENV_DIR" && docker-compose -f docker-compose.sentinel.yml down)
        log_success "Redis Sentinel 已停止"
    fi
}

cleanup() {
    log_info "清理测试环境..."
    stop_redis
    stop_redis_cluster
    stop_redis_sentinel
    rm -f "$PROJECT_ROOT"/*.db "$PROJECT_ROOT"/*_wal.db 2>/dev/null || true
    log_success "清理完成"
}

# ==================== 测试函数 ====================

check_dependencies() {
    log_info "检查依赖..."
    if ! check_cargo; then exit 1; fi
    if ! command -v docker &> /dev/null; then
        log_warning "docker 未安装，部分测试将被跳过"
    fi
    log_success "依赖检查通过"
}

ensure_output_dir() {
    ensure_directory "$OUTPUT_DIR"
}

run_unit_tests() {
    log_info "运行单元测试..."
    local output="$OUTPUT_DIR/unit_tests.log"
    local start_time=$(date +%s)
    if cargo test --lib --features full -- --nocapture > "$output" 2>&1 && \
       cargo test --test unit --features full -- --nocapture >> "$output" 2>&1; then
        local duration=$(( $(date +%s) - start_time ))
        log_success "✅ 单元测试通过 (${duration}s)"
        return 0
    else
        local duration=$(( $(date +%s) - start_time ))
        log_error "❌ 单元测试失败 (${duration}s)"
        [[ "$VERBOSE" == true ]] && cat "$output"
        return 1
    fi
}

run_integration_tests() {
    log_info "运行集成测试..."
    local output="$OUTPUT_DIR/integration_tests.log"
    start_redis
    local start_time=$(date +%s)
    if timeout "$TEST_TIMEOUT" cargo test --test integration --features full -- --nocapture >> "$output" 2>&1; then
        local duration=$(( $(date +%s) - start_time ))
        log_success "✅ 集成测试通过 (${duration}s)"
        stop_redis
        return 0
    else
        local duration=$(( $(date +%s) - start_time ))
        log_warning "⚠️  集成测试发现问题或跳过 (${duration}s)"
        stop_redis
        return 1
    fi
}

run_e2e_tests() {
    log_info "运行端到端测试..."
    local output="$OUTPUT_DIR/e2e_tests.log"
    local start_time=$(date +%s)
    if timeout "$TEST_TIMEOUT" cargo test --test e2e --features full -- --nocapture > "$output" 2>&1; then
        local duration=$(( $(date +%s) - start_time ))
        log_success "✅ 端到端测试通过 (${duration}s)"
        return 0
    else
        local duration=$(( $(date +%s) - start_time ))
        log_warning "⚠️  端到端测试发现问题 (${duration}s)"
        return 1
    fi
}

run_chaos_tests() {
    log_info "运行混沌测试..."
    local output="$OUTPUT_DIR/chaos_tests.log"
    start_redis
    local start_time=$(date +%s)
    if timeout "$TEST_TIMEOUT" cargo test --test chaos --features full -- --nocapture > "$output" 2>&1; then
        local duration=$(( $(date +%s) - start_time ))
        log_success "✅ 混沌测试通过 (${duration}s)"
    else
        local duration=$(( $(date +%s) - start_time ))
        log_warning "⚠️  混沌测试发现问题 (${duration}s)"
    fi
    stop_redis
}

run_security_audit() {
    log_info "运行安全审计..."
    local output="$OUTPUT_DIR/security_audit.log"
    local start_time=$(date +%s)
    if [ -f "$PROJECT_ROOT/scripts/validation/security_audit.sh" ]; then
        if timeout "$TEST_TIMEOUT" bash "$PROJECT_ROOT/scripts/validation/security_audit.sh" > "$output" 2>&1; then
            log_success "✅ 安全审计通过 ($(( $(date +%s) - start_time ))s)"
            return 0
        else
            log_warning "⚠️  安全审计发现问题 ($(( $(date +%s) - start_time ))s)"
            return 1
        fi
    else
        log_warning "安全审计脚本不存在，跳过"
        return 0
    fi
}

run_memory_leak_tests() {
    log_info "运行内存泄漏测试..."
    local output="$OUTPUT_DIR/memory_test.log"
    local start_time=$(date +%s)
    if [ -f "$SCRIPT_DIR/memory_test.sh" ]; then
        if timeout "$TEST_TIMEOUT" bash "$SCRIPT_DIR/memory_test.sh" -m cargo -o "$OUTPUT_DIR" > "$output" 2>&1; then
            log_success "✅ 内存测试通过 ($(( $(date +%s) - start_time ))s)"
            return 0
        else
            log_warning "⚠️  内存测试发现问题 ($(( $(date +%s) - start_time ))s)"
            return 1
        fi
    else
        log_warning "内存测试脚本不存在，跳过"
        return 0
    fi
}

run_redis_compatibility_tests() {
    log_info "运行 Redis 版本兼容性测试..."
    local output="$OUTPUT_DIR/redis_compatibility.log"
    export REDIS_VERSION_TEST_ENABLED=1
    local start_time=$(date +%s)
    if timeout "$TEST_TIMEOUT" cargo test redis_version_compatibility > "$output" 2>&1; then
        log_success "✅ Redis 兼容性测试通过 ($(( $(date +%s) - start_time ))s)"
        return 0
    else
        log_warning "⚠️  Redis 兼容性测试发现问题 ($(( $(date +%s) - start_time ))s)"
        return 1
    fi
}

run_code_quality_checks() {
    log_info "运行代码质量检查..."
    local output="$OUTPUT_DIR/code_quality.log"
    local start_time=$(date +%s)
    local failed=0

    log_info "  检查代码格式化..."
    if cargo fmt --check > "$output" 2>&1; then
        log_success "  ✅ 格式化检查通过"
    else
        log_error "  ❌ 格式化检查失败，运行 cargo fmt 修复"
        failed=$((failed + 1))
    fi

    log_info "  运行 Clippy 静态分析..."
    if cargo clippy -- -D warnings >> "$output" 2>&1; then
        log_success "  ✅ Clippy 检查通过"
    else
        log_error "  ❌ Clippy 检查失败"
        failed=$((failed + 1))
    fi

    log_info "  检查文档..."
    if cargo doc --no-deps >> "$output" 2>&1; then
        log_success "  ✅ 文档生成成功"
    else
        log_warning "  ⚠️  文档生成失败"
        failed=$((failed + 1))
    fi

    if [ $failed -eq 0 ]; then
        log_success "✅ 代码质量检查通过 ($(( $(date +%s) - start_time ))s)"
        return 0
    else
        log_error "❌ 代码质量检查失败: ${failed} 项未通过 ($(( $(date +%s) - start_time ))s)"
        return 1
    fi
}

run_coverage() {
    log_info "生成测试覆盖率报告..."
    if command -v cargo-tarpaulin &> /dev/null; then
        cargo tarpaulin --out Html --output-dir "$PROJECT_ROOT/target/coverage"
        log_success "覆盖率报告已生成: $PROJECT_ROOT/target/coverage/index.html"
    else
        log_warning "cargo-tarpaulin 未安装，跳过覆盖率报告"
        log_info "安装: cargo install cargo-tarpaulin"
    fi
}

# ==================== 报告生成 ====================

generate_test_report() {
    local total_tests="$1" passed="$2" failed="$3" skipped="$4"
    local report_file="$OUTPUT_DIR/test_report.md"

    cat > "$report_file" << EOF
# 综合测试报告

生成时间: $(date '+%Y-%m-%d %H:%M:%S')

## 测试结果摘要

- **总测试数**: $total_tests
- **通过**: $passed
- **失败**: $failed
- **跳过**: $skipped
EOF

    for log_file in "$OUTPUT_DIR"/*.log; do
        if [ -f "$log_file" ]; then
            local test_name=$(basename "$log_file" .log)
            local status="✅ PASSED"
            grep -q "FAILED\|失败\|Error\|error" "$log_file" && status="❌ FAILED"
            grep -q "WARNING\|警告\|跳过\|skipped" "$log_file" && status="⚠️  WARNING"
            {
                echo ""
                echo "### $test_name"
                echo "状态: $status"
                echo ""
                echo '```'
                tail -50 "$log_file"
                echo '```'
            } >> "$report_file"
        fi
    done
    log_info "测试报告已生成: $report_file"
}

# ==================== 主函数 ====================

main() {
    print_header "综合测试运行器"

    check_dependencies
    ensure_output_dir

    run_test_group() {
        local total=0 passed=0 failed=0 skipped=0
        local tests_to_run=("$@")
        for test_info in "${tests_to_run[@]}"; do
            IFS=':' read -r test_func test_name <<< "$test_info"
            echo ""
            log_info "▶ 运行 $test_name..."
            total=$((total + 1))
            if $test_func; then
                passed=$((passed + 1))
            else
                failed=$((failed + 1))
            fi
        done
        generate_test_report "$total" "$passed" "$failed" "$skipped"
        echo ""
        print_header "执行完成"
        log_success "通过: $passed  |  失败: $failed  |  跳过: $skipped"
        return $failed
    }

    case "$TEST_TYPE" in
        unit)
            run_unit_tests
            ;;
        integration)
            trap cleanup EXIT
            run_integration_tests
            ;;
        e2e)
            run_e2e_tests
            ;;
        chaos)
            trap cleanup EXIT
            run_chaos_tests
            ;;
        security)
            run_security_audit
            ;;
        memory)
            run_memory_leak_tests
            ;;
        redis-comp)
            run_redis_compatibility_tests
            ;;
        quality)
            run_code_quality_checks
            ;;
        coverage)
            run_unit_tests
            run_coverage
            ;;
        clean)
            cleanup
            exit 0
            ;;
        all)
            trap cleanup EXIT
            run_test_group \
                "run_unit_tests:单元测试" \
                "run_integration_tests:集成测试" \
                "run_e2e_tests:端到端测试" \
                "run_chaos_tests:混沌测试" \
                "run_security_audit:安全审计" \
                "run_memory_leak_tests:内存泄漏测试" \
                "run_redis_compatibility_tests:Redis兼容性测试" \
                "run_code_quality_checks:代码质量检查"
            ;;
    esac
}

main "$@"
