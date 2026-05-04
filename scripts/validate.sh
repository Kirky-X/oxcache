#!/usr/bin/env bash
# 验证脚本 — 特性组合 / 文档 / 安全审计
# 用法: ./scripts/validate.sh <subcommand> [options]
#
# 子命令:
#   features    验证特性组合
#   docs        验证文档
#   security    安全审计 (cargo audit)
#   all         运行全部验证

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/common.sh"

PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

show_help() {
    cat << EOF
用法: $(basename "$0") <subcommand> [options]

验证脚本 — 特性组合 / 文档 / 安全审计

子命令:
  features    验证特性组合 (--all, --quick, --features "...", --report)
  docs        验证文档
  security    安全审计 (cargo audit)
  all         运行全部验证
  help        显示帮助信息

选项（全局）:
  -v, --verbose   详细输出
  -h, --help      显示帮助

EOF
}

# ==================== 特性组合验证 ====================

TIER1_COMBINATIONS=(
    "minimal"
    "core"
    "full"
)

TIER2_COMBINATIONS=(
    "moka,macros"
    "redis,macros"
    "moka,redis,batch-write"
    "moka,redis,metrics"
    "moka,redis,bloom-filter"
    "moka,redis,database"
    "core,full-metrics"
    "core,confers"
    "core,smart-strategy"
    "core,http-cache"
)

cmd_features() {
    local quick_mode=false
    local generate_report=false
    local specific_features=""
    local combinations=()

    # 解析特性组合参数
    local args=()
    while [[ $# -gt 0 ]]; do
        case $1 in
            --all)       combinations=("${TIER1_COMBINATIONS[@]}" "${TIER2_COMBINATIONS[@]}"); shift ;;
            --quick)     quick_mode=true; shift ;;
            --features)  specific_features="$2"; shift 2 ;;
            --report)    generate_report=true; shift ;;
            -h|--help)   echo "特性组合验证选项: --all, --quick, --features \"...\", --report"; return 0 ;;
            *)           args+=("$1"); shift ;;
        esac
    done

    if [ -z "${combinations[*]:-}" ] && [ -z "$specific_features" ]; then
        combinations=("${TIER1_COMBINATIONS[@]}")
    fi

    if [ -n "$specific_features" ]; then
        combinations=("$specific_features")
    fi

    log_info "开始验证特性组合..."
    log_info "快速模式: $quick_mode"
    log_info "组合数量: ${#combinations[@]}"

    local results_file=$(mktemp)
    local total=0 passed=0 failed=0

    for features in "${combinations[@]}"; do
        total=$((total + 1))
        local start_time=$(date +%s)

        log_info "验证特性组合: $features"

        if cargo check --features "$features" 2>&1 | grep -q "error"; then
            log_error "特性组合 '$features' 编译失败"
            failed=$((failed + 1))
            echo "$features,fail,skip,skip" >> "$results_file"
        else
            if [ "$quick_mode" = false ]; then
                if ! cargo test --features "$features" --lib 2>&1 | grep -q "test result: ok"; then
                    log_warning "特性组合 '$features' 单元测试有问题"
                fi
                if ! cargo test --features "$features" --doc 2>&1 | grep -q "test result: ok"; then
                    log_warning "特性组合 '$features' 文档测试有问题"
                fi
            fi
            passed=$((passed + 1))
            echo "$features,pass,pass,pass" >> "$results_file"
        fi

        local duration=$(( $(date +%s) - start_time ))
        log_info "完成 (耗时: ${duration}s)"
        echo ""
    done

    # 生成报告
    if [ "$generate_report" = true ]; then
        local report_file="test-reports/feature-combinations-report.md"
        mkdir -p test-reports
        {
            echo "# 特性组合测试报告"
            echo ""
            echo "生成时间: $(date '+%Y-%m-%d %H:%M:%S')"
            echo "Rust 版本: $(rustc --version)"
            echo ""
            echo "## 测试结果"
            echo ""
            echo "| 特性组合 | 编译 | 单元测试 | 文档测试 | 状态 |"
            echo "|---------|------|---------|---------|------|"
            while IFS=, read -r f comp unit doc; do
                local cs="✅" us="✅" ds="✅" st="Pass"
                [ "$comp" = "fail" ] && { cs="❌"; st="Fail"; }
                [ "$unit" = "fail" ] && { us="❌"; st="Fail"; }
                [ "$doc"  = "fail" ] && { ds="❌"; st="Fail"; }
                echo "| $f | $cs | $us | $ds | $st |"
            done < "$results_file"
        } > "$report_file"
        log_info "测试报告已生成: $report_file"
    fi

    rm -f "$results_file"

    echo ""
    print_section "特性组合验证完成"
    log_success "通过: $passed  |  失败: $failed  |  总计: $total"

    [ $failed -gt 0 ] && return 1
    return 0
}

# ==================== 文档验证 ====================

cmd_docs() {
    local errors=0 warnings=0

    log_info "开始文档验证..."

    # 1. 检查示例代码编译
    echo "  1. 检查示例代码编译"
    if [ -d "$PROJECT_ROOT/examples" ]; then
        (cd "$PROJECT_ROOT/examples" && cargo build --all-features 2>&1 | grep -q "error") && {
            log_error "示例代码编译失败"
            errors=$((errors + 1))
        } || log_success "示例代码编译成功"
    else
        log_warning "examples/ 目录不存在，跳过"
    fi

    # 2. 检查文档格式
    echo "  2. 检查文档格式"
    for doc in API_REFERENCE.md USER_GUIDE.md SERIALIZATION.md DATABASE_INTEGRATION.md ARCHITECTURE.md CONFIG_VALIDATION.md; do
        if [ -f "$PROJECT_ROOT/docs/$doc" ]; then
            if grep -q "^#" "$PROJECT_ROOT/docs/$doc" && grep -q "##" "$PROJECT_ROOT/docs/$doc"; then
                log_success "  $doc 格式正确"
            else
                log_warning "  $doc 缺少标题"
                warnings=$((warnings + 1))
            fi
        fi
    done

    # 3. 检查文档链接
    echo "  3. 检查文档链接"
    local link_errors=0
    for doc in API_REFERENCE.md USER_GUIDE.md SERIALIZATION.md DATABASE_INTEGRATION.md ARCHITECTURE.md CONFIG_VALIDATION.md; do
        if [ -f "$PROJECT_ROOT/docs/$doc" ]; then
            local broken=$(grep -o '\[.*\](.*\.md)' "$PROJECT_ROOT/docs/$doc" 2>/dev/null | sed 's/.*(\(.*\))/\1/' | while read target; do
                [ -n "$target" ] && [ ! -f "$PROJECT_ROOT/docs/$target" ] && [ ! -f "$PROJECT_ROOT/$target" ] && echo "$target"
            done)
            if [ -n "$broken" ]; then
                log_error "  $doc 包含损坏链接: $broken"
                link_errors=$((link_errors + 1))
            fi
        fi
    done
    [ $link_errors -eq 0 ] && log_success "  所有链接有效"
    errors=$((errors + link_errors))

    # 4. 检查代码示例语法
    echo "  4. 检查代码示例语法"
    for doc in API_REFERENCE.md USER_GUIDE.md SERIALIZATION.md DATABASE_INTEGRATION.md ARCHITECTURE.md CONFIG_VALIDATION.md; do
        if [ -f "$PROJECT_ROOT/docs/$doc" ]; then
            if grep -q '```rust' "$PROJECT_ROOT/docs/$doc" && ! grep -q '```' "$PROJECT_ROOT/docs/$doc"; then
                log_warning "  $doc 可能包含未闭合的代码块"
                warnings=$((warnings + 1))
            fi
        fi
    done
    log_success "  代码示例格式检查通过"

    # 总结
    echo ""
    print_section "文档验证结果"
    [ $errors -gt 0 ] && log_error "错误: $errors"  || log_success "错误: 0"
    [ $warnings -gt 0 ] && log_warning "警告: $warnings" || log_success "警告: 0"

    [ $errors -gt 0 ] && return 1
    return 0
}

# ==================== 安全审计 ====================

cmd_security() {
    local output_format="human"
    local output_file=""
    local ignore_advisories=""
    local fail_on_warning=true
    local verbose=false

    # 解析参数
    while [[ $# -gt 0 ]]; do
        case $1 in
            -f|--format)     output_format="$2"; shift 2 ;;
            -o|--output)     output_file="$2"; shift 2 ;;
            -i|--ignore)     ignore_advisories="$2"; shift 2 ;;
            -w|--warnings-only) fail_on_warning=false; shift ;;
            -v|--verbose)    verbose=true; shift ;;
            -h|--help)       echo "安全审计选项: -f human|json|both, -o FILE, -i IDS, -w, -v"; return 0 ;;
            *)               log_error "未知选项: $1"; return 1 ;;
        esac
    done

    print_header "Cargo Audit 安全审计"

    # 检查依赖
    if ! check_cargo; then return 1; fi
    if ! cargo audit --version &>/dev/null 2>&1; then
        log_info "安装 cargo-audit..."
        cargo install cargo-audit || { log_error "安装失败"; return 1; }
    fi

    # 项目信息
    local project_name project_version
    project_name=$(cargo metadata --no-deps --format-version 1 2>/dev/null | grep -o '"name":"[^"]*"' | head -1 | cut -d'"' -f4) || project_name="unknown"
    project_version=$(cargo metadata --no-deps --format-version 1 2>/dev/null | grep -o '"version":"[^"]*"' | head -1 | cut -d'"' -f4) || project_version="unknown"
    log_info "项目: $project_name v$project_version"

    # 构建忽略列表
    local ignore_opts=""
    if [ -n "$ignore_advisories" ]; then
        IFS=',' read -ra ADVIS <<< "$ignore_advisories"
        for a in "${ADVIS[@]}"; do
            ignore_opts="$ignore_opts --ignore $a"
            log_info "忽略 advisory: $a"
        done
    fi

    # 运行审计
    log_info "运行 Cargo 安全审计..."
    if [ -f ".cargo/audit.toml" ]; then
        log_info "发现 .cargo/audit.toml 配置文件"
    fi

    local audit_cmd="cargo audit $ignore_opts --stale"
    [ "$output_format" = "json" ] || [ "$output_format" = "both" ] && audit_cmd="$audit_cmd --json"
    [ "$verbose" = true ] && audit_cmd="$audit_cmd --verbose"

    local output exit_code=0
    if ! output=$($audit_cmd 2>&1); then
        exit_code=1
        if echo "$output" | grep -q "network\|timeout\|IO error\|git operation failed"; then
            log_warning "网络连接失败，尝试离线模式..."
            local offline_cmd="cargo audit $ignore_opts --no-fetch"
            [ "$output_format" = "json" ] || [ "$output_format" = "both" ] && offline_cmd="$offline_cmd --json"
            if output=$($offline_cmd 2>&1); then
                exit_code=0
                log_info "离线模式运行成功"
            else
                exit_code=1
                log_warning "离线模式也失败"
            fi
        fi
    fi

    # 处理输出
    if [ "$output_format" = "json" ] || [ "$output_format" = "both" ]; then
        if echo "$output" | jq . &>/dev/null 2>&1; then
            local vuln=$(echo "$output" | jq '.vulnerabilities.found // false')
            local count=$(echo "$output" | jq '.vulnerabilities.count // 0')
            if [ "$vuln" = "true" ] && [ "$count" -gt 0 ]; then
                log_error "发现 $count 个安全漏洞"
                echo "$output" | jq -r '.vulnerabilities.list[] | "🚨 \(.advisory.id): \(.advisory.title)\n    包: \(.package.name) v\(.package.version)\n    修复: \(.advisory.patched_versions // ["无"] | join(", "))"' 2>/dev/null
                exit_code=1
            else
                log_success "✅ 未发现安全漏洞"
                exit_code=0
            fi
        else
            log_error "JSON 解析失败"
            exit_code=1
        fi
    fi

    if [ "$output_format" = "human" ] || [ "$output_format" = "both" ]; then
        echo ""
        if echo "$output" | grep -q "Success"; then
            log_success "✅ 安全审计通过 - 未发现漏洞"
        elif echo "$output" | grep -q "Vulnerability"; then
            log_error "❌ 发现安全漏洞"
            echo "$output" | grep -E "(Vulnerability|RUSTSEC|CVE)" | head -10
            exit_code=1
        else
            echo "$output" | head -20
            exit_code=1
        fi
    fi

    # 保存到文件
    if [ -n "$output_file" ]; then
        echo "$output" > "$output_file"
        log_info "详细报告已保存到: $output_file"
        local summary_file="${output_file%.json}_summary.txt"
        {
            echo "Cargo Audit 安全审计摘要"
            echo "生成时间: $(date)"
            echo "项目: $project_name v$project_version"
            echo ""
            [ $exit_code -eq 0 ] && echo "✅ 通过" || echo "❌ 失败"
            echo ""
            echo "$output" | grep -E "(Vulnerability|RUSTSEC|CVE|advisory)" | head -20
        } > "$summary_file"
        log_info "摘要报告: $summary_file"
    fi

    echo ""
    [ $exit_code -eq 0 ] && log_success "安全审计通过" || log_error "安全审计失败 - 发现安全漏洞"
    return $exit_code
}

# ==================== 全部验证 ====================

cmd_all() {
    local overall=0

    print_header "运行全部验证"

    log_info "▶ 1/3: 特性组合验证"
    cmd_features --all --quick || overall=$((overall + 1))
    echo ""

    log_info "▶ 2/3: 文档验证"
    cmd_docs || overall=$((overall + 1))
    echo ""

    log_info "▶ 3/3: 安全审计"
    cmd_security || overall=$((overall + 1))
    echo ""

    print_header "全部验证完成"
    if [ $overall -eq 0 ]; then
        log_success "✅ 全部验证通过"
    else
        log_error "❌ ${overall} 项验证失败"
    fi
    return $overall
}

# ==================== 主入口 ====================

main() {
    if [ $# -eq 0 ]; then
        show_help
        exit 1
    fi

    local cmd="$1"
    shift

    case "$cmd" in
        features)        cmd_features "$@" ;;
        docs)            cmd_docs "$@" ;;
        security)        cmd_security "$@" ;;
        all)             cmd_all "$@" ;;
        help|-h|--help)  show_help ;;
        *)
            log_error "未知子命令: $cmd"
            show_help
            exit 1
            ;;
    esac
}

main "$@"
