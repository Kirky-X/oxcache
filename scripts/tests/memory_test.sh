#!/bin/bash
# 内存测试脚本
# 使用 cargo test 运行内存泄漏检测、内存安全测试和内存使用分析

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

# 默认配置
DEFAULT_TIMEOUT=300
DEFAULT_OUTPUT_DIR="test-reports"

# 帮助信息
show_help() {
    cat << EOF
内存测试工具

用法: $0 [选项]

选项:
  -m, --mode MODE       测试模式: cargo, miri, all (默认: all)
                          cargo  — 使用 cargo test 运行内存测试（stable 可用）
                          miri   — 使用 cargo miri（需要 nightly）
                          all    — 先 cargo，再尝试 miri
  -t, --timeout SECONDS  测试超时时间 (默认: $DEFAULT_TIMEOUT)
  -o, --output DIR       输出目录 (默认: $DEFAULT_OUTPUT_DIR)
  -v, --verbose          详细输出
  -h, --help             显示帮助信息

示例:
  $0                        # 运行所有内存测试
  $0 -m cargo               # 仅 cargo test 内存测试
  $0 -m miri                # 仅 Miri（需 nightly）
EOF
}

# 解析命令行参数
MODE="all"
TIMEOUT="$DEFAULT_TIMEOUT"
OUTPUT_DIR="$DEFAULT_OUTPUT_DIR"
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -m|--mode)      MODE="$2"; shift 2 ;;
        -t|--timeout)   TIMEOUT="$2"; shift 2 ;;
        -o|--output)    OUTPUT_DIR="$2"; shift 2 ;;
        -v|--verbose)   VERBOSE=true; shift ;;
        -h|--help)      show_help; exit 0 ;;
        *)              log_error "未知选项: $1"; show_help; exit 1 ;;
    esac
done

# ==================== cargo test 内存测试 ====================
run_cargo_memory_tests() {
    print_section "cargo test 内存泄漏检测"

    local output_file="$OUTPUT_DIR/memory_cargo_test.log"
    local start_time=$(date +%s)
    local failed=0

    # 1) 运行 memory_leak_test 模块
    log_info "运行内存泄漏测试 (memory_leak_test)..."
    if timeout "$TIMEOUT" cargo test --test performance --features full \
        memory_leak_test -- --nocapture >> "$output_file" 2>&1; then
        local count=$(grep -c "test result:" "$output_file" 2>/dev/null || echo 0)
        log_success "  ✅ memory_leak_test 通过"
    else
        log_error "  ❌ memory_leak_test 失败"
        failed=$((failed + 1))
    fi

    # 2) 运行 memory_tests 模块
    log_info "运行内存压力测试 (memory_tests)..."
    if timeout "$TIMEOUT" cargo test --test performance --features full \
        memory_tests -- --nocapture >> "$output_file" 2>&1; then
        log_success "  ✅ memory_tests 通过"
    else
        log_error "  ❌ memory_tests 失败"
        failed=$((failed + 1))
    fi

    # 3) 运行 miri_memory_test 模块（无需 nightly，作为普通测试运行）
    log_info "运行内存安全测试 (miri_memory_test)..."
    if timeout "$TIMEOUT" cargo test --test performance --features full \
        miri_memory_test -- --nocapture >> "$output_file" 2>&1; then
        log_success "  ✅ miri_memory_test 通过"
    else
        log_error "  ❌ miri_memory_test 失败"
        failed=$((failed + 1))
    fi

    local duration=$(( $(date +%s) - start_time ))

    # 统计测试数量
    local total_passed=$(grep "test result:" "$output_file" 2>/dev/null \
        | awk '{sum += $4} END {print sum+0}')
    local total_failed=$(grep "test result:" "$output_file" 2>/dev/null \
        | awk '{sum += $6} END {print sum+0}')
    local total_ignored=$(grep "test result:" "$output_file" 2>/dev/null \
        | awk '{sum += $8} END {print sum+0}')

    log_info "  测试统计: ${total_passed} passed, ${total_failed} failed, ${total_ignored} ignored"

    if [[ $failed -eq 0 ]]; then
        log_success "✅ cargo 内存测试全部通过 (${duration}s)"
        return 0
    else
        log_error "❌ cargo 内存测试有 ${failed} 项失败 (${duration}s)"
        return 1
    fi
}

# ==================== Miri 内存测试（需要 nightly）====================
run_miri_tests() {
    print_section "Miri 内存安全检查（需要 nightly）"

    local output_file="$OUTPUT_DIR/memory_miri_test.log"

    # 检查 nightly 工具链
    if ! rustup toolchain list | grep -q "nightly"; then
        log_warning "nightly 工具链未安装，跳过 Miri 测试"
        log_info "安装: rustup toolchain install nightly"
        return 0
    fi

    # 检查 miri 组件
    if ! rustup +nightly component list --installed 2>/dev/null | grep -q "miri"; then
        log_warning "miri 组件未安装，跳过 Miri 测试"
        log_info "安装: rustup +nightly component add miri"
        return 0
    fi

    local start_time=$(date +%s)

    log_info "初始化 Miri..."
    cargo +nightly miri setup > /dev/null 2>&1 || true

    log_info "运行 Miri 测试..."
    if MIRIFLAGS='-Zmiri-disable-isolation' timeout "$TIMEOUT" \
        cargo +nightly miri test --test performance --features full \
        miri_memory_test >> "$output_file" 2>&1; then
        local duration=$(( $(date +%s) - start_time ))
        log_success "✅ Miri 测试通过 (${duration}s)"
        return 0
    else
        local duration=$(( $(date +%s) - start_time ))
        log_warning "⚠️  Miri 测试发现问题 (${duration}s)"
        return 1
    fi
}

# ==================== 内存使用监控 ====================
run_memory_monitor() {
    print_section "内存使用监控"

    local csv_file="$OUTPUT_DIR/memory_usage.csv"

    log_info "运行内存测试并监控内存使用..."
    echo "timestamp_ms,rss_kb" > "$csv_file"

    # 后台运行内存压力测试
    cargo test --test performance --features full \
        test_concurrent_access -- --nocapture > "$OUTPUT_DIR/memory_monitor.log" 2>&1 &
    local test_pid=$!

    local i=0
    local peak_rss=0
    while kill -0 "$test_pid" 2>/dev/null && [[ $i -lt 60 ]]; do
        local rss
        rss=$(ps -o rss= -p "$test_pid" 2>/dev/null | tr -d ' ')
        rss=${rss:-0}
        echo "$((i * 500)),$rss" >> "$csv_file"
        if [[ $rss -gt $peak_rss ]]; then
            peak_rss=$rss
        fi
        sleep 0.5
        ((i++)) || true
    done

    wait "$test_pid" 2>/dev/null || true

    local peak_mb=$((peak_rss / 1024))
    log_info "  峰值内存: ${peak_mb} MB (${peak_rss} KB)"
    log_info "  数据文件: $csv_file"

    if [[ $peak_rss -gt 0 ]]; then
        log_success "✅ 内存监控完成，峰值 ${peak_mb} MB"
    else
        log_warning "⚠️  未能采集到内存数据（测试可能执行过快）"
    fi
    return 0
}

# ==================== 生成报告 ====================
generate_report() {
    local cargo_result="$1"
    local miri_result="$2"
    local report_file="$OUTPUT_DIR/memory_test_report.md"

    local cargo_passed=0
    local cargo_failed=0
    if [[ -f "$OUTPUT_DIR/memory_cargo_test.log" ]]; then
        cargo_passed=$(grep "test result:" "$OUTPUT_DIR/memory_cargo_test.log" 2>/dev/null \
            | awk '{sum += $4} END {print sum+0}')
        cargo_failed=$(grep "test result:" "$OUTPUT_DIR/memory_cargo_test.log" 2>/dev/null \
            | awk '{sum += $6} END {print sum+0}')
    fi

    local peak_mem="N/A"
    if [[ -f "$OUTPUT_DIR/memory_usage.csv" ]]; then
        local peak_kb
        peak_kb=$(tail -n +2 "$OUTPUT_DIR/memory_usage.csv" 2>/dev/null \
            | cut -d',' -f2 | sort -n | tail -1)
        if [[ -n "$peak_kb" && "$peak_kb" -gt 0 ]] 2>/dev/null; then
            peak_mem="$((peak_kb / 1024)) MB (${peak_kb} KB)"
        fi
    fi

    cat > "$report_file" << EOF
# 内存测试报告

生成时间: $(date '+%Y-%m-%d %H:%M:%S')
Rust 版本: $(rustc --version)

## 测试结果

### cargo test 内存测试
- 状态: $cargo_result
- 通过: $cargo_passed
- 失败: $cargo_failed
- 测试模块: memory_leak_test, memory_tests, miri_memory_test

### Miri 内存安全测试
- 状态: $miri_result

### 内存监控
- 峰值内存: $peak_mem

## 测试覆盖
- L1 缓存内存泄漏
- L2 缓存内存泄漏
- 两级缓存内存泄漏
- 批量操作内存
- 大量小对象
- 大对象存储
- 并发访问内存安全
- 缓冲区溢出防护
- 循环引用检测
- 内存对齐验证
- 未初始化内存防护
EOF

    log_info "报告已生成: $report_file"
}

# ==================== 主函数 ====================
main() {
    print_header "Oxcache 内存测试工具"

    check_cargo || exit 1
    ensure_directory "$OUTPUT_DIR"

    # 清空旧日志
    > "$OUTPUT_DIR/memory_cargo_test.log"

    local cargo_result="⚠️ 未运行"
    local miri_result="⚠️ 未运行"

    case "$MODE" in
        cargo)
            if run_cargo_memory_tests; then
                cargo_result="✅ 通过"
            else
                cargo_result="❌ 失败"
            fi
            run_memory_monitor
            ;;
        miri)
            if run_miri_tests; then
                miri_result="✅ 通过"
            else
                miri_result="❌ 失败"
            fi
            ;;
        all)
            if run_cargo_memory_tests; then
                cargo_result="✅ 通过"
            else
                cargo_result="❌ 失败"
            fi
            if run_miri_tests; then
                miri_result="✅ 通过"
            else
                miri_result="❌ 失败"
            fi
            run_memory_monitor
            ;;
        *)
            log_error "无效模式: $MODE"
            show_help
            exit 1
            ;;
    esac

    generate_report "$cargo_result" "$miri_result"

    print_header "内存测试完成"

    if [[ "$cargo_result" == "❌ 失败" ]]; then
        log_error "内存测试存在失败项，请检查日志"
        exit 1
    fi

    log_success "🎉 内存测试全部完成"
    exit 0
}

main "$@"
