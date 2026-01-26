#!/bin/bash

# Cargo Audit安全审计脚本
# 用于检测Rust项目依赖库中的安全漏洞

# 引入公共库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

# 默认配置
DEFAULT_AUDIT_OPTIONS="--json"
DEFAULT_OUTPUT_FORMAT="human"  # human, json, both
DEFAULT_FAIL_ON_WARNING=true
DEFAULT_IGNORE_ADVISORIES=""   # 逗号分隔的advisory ID列表

# 帮助信息
show_help() {
    cat << EOF
Cargo Audit安全审计脚本

用法: $0 [选项]

选项:
  -f, --format FORMAT    输出格式: human, json, both (默认: $DEFAULT_OUTPUT_FORMAT)
  -o, --output FILE      输出结果文件
  -i, --ignore IDS       忽略特定的advisory ID (逗号分隔)
  -w, --warnings-only    只显示警告，不将警告视为错误
  -v, --verbose          详细输出
  -h, --help             显示帮助信息

示例:
  $0                                    # 基本审计
  $0 -f json -o audit_report.json     # JSON格式输出到文件
  $0 -i RUSTSEC-2023-0001,RUSTSEC-2023-0002  # 忽略特定advisory
  $0 -w                                # 只显示警告

EOF
}

# 解析命令行参数
OUTPUT_FORMAT="$DEFAULT_OUTPUT_FORMAT"
OUTPUT_FILE=""
IGNORE_ADVISORIES="$DEFAULT_IGNORE_ADVISORIES"
FAIL_ON_WARNING="$DEFAULT_FAIL_ON_WARNING"
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -f|--format)
            OUTPUT_FORMAT="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_FILE="$2"
            shift 2
            ;;
        -i|--ignore)
            IGNORE_ADVISORIES="$2"
            shift 2
            ;;
        -w|--warnings-only)
            FAIL_ON_WARNING=false
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

    if ! cargo audit --version &> /dev/null 2>&1; then
        log_info "安装cargo-audit..."
        if cargo install cargo-audit; then
            log_success "cargo-audit安装成功"
        else
            log_error "cargo-audit安装失败"
            exit 1
        fi
    fi

    log_success "依赖检查通过"
}

# 获取项目信息
get_project_info() {
    log_info "获取项目信息..."
    
    PROJECT_NAME=$(cargo metadata --no-deps --format-version 1 | grep -o '"name":"[^"]*"' | head -1 | cut -d'"' -f4)
    PROJECT_VERSION=$(cargo metadata --no-deps --format-version 1 | grep -o '"version":"[^"]*"' | head -1 | cut -d'"' -f4)
    
    if [[ -n "$PROJECT_NAME" ]]; then
        log_info "项目名称: $PROJECT_NAME"
        log_info "项目版本: $PROJECT_VERSION"
    else
        log_warning "无法获取项目信息"
    fi
}

# 构建忽略列表
build_ignore_list() {
    local ignore_options=""
    
    if [[ -n "$IGNORE_ADVISORIES" ]]; then
        IFS=',' read -ra ADVISORIES <<< "$IGNORE_ADVISORIES"
        for advisory in "${ADVISORIES[@]}"; do
            ignore_options="$ignore_options --ignore $advisory"
            log_info "忽略advisory: $advisory"
        done
    fi
    
    echo "$ignore_options"
}

# 运行cargo audit
run_cargo_audit() {
    log_info "运行Cargo安全审计..."
    
    # 检查是否存在配置文件
    if [[ -f ".cargo/audit.toml" ]]; then
        log_info "发现配置文件: .cargo/audit.toml"
        log_info "配置文件将被cargo audit自动加载"
    fi
    
    local ignore_options=$(build_ignore_list)
    local audit_cmd="cargo audit $ignore_options --stale"
    
    if [[ "$OUTPUT_FORMAT" == "json" ]] || [[ "$OUTPUT_FORMAT" == "both" ]]; then
        audit_cmd="$audit_cmd --json"
    fi
    
    if [[ "$VERBOSE" == true ]]; then
        audit_cmd="$audit_cmd --verbose"
    fi
    
    log_info "执行命令: $audit_cmd"
    echo ""
    
    # 运行审计
    local output
    local exit_code
    
    if output=$($audit_cmd 2>&1); then
        exit_code=0
    else
        exit_code=1
        
        # 检查是否是网络连接问题
        if echo "$output" | grep -q "couldn't fetch advisory database\|network\|timeout\|IO error\|git operation failed"; then
            log_warning "网络连接失败，尝试离线模式..."
            
            # 尝试使用离线模式（如果本地有缓存的数据库）
            local offline_cmd="cargo audit $ignore_options --no-fetch"
            if [[ "$OUTPUT_FORMAT" == "json" ]] || [[ "$OUTPUT_FORMAT" == "both" ]]; then
                offline_cmd="$offline_cmd --json"
            fi
            
            log_info "执行离线命令: $offline_cmd"
            
            if output=$($offline_cmd 2>&1); then
                exit_code=0
                log_info "离线模式运行成功"
            else
                exit_code=1
                log_warning "离线模式也失败，将显示原始错误信息"
            fi
        fi
    fi
    
    # 处理输出
    process_audit_output "$output" "$exit_code"
}

# 处理审计输出
process_audit_output() {
    local output="$1"
    local exit_code="$2"
    
    # 保存原始输出
    local raw_output="$output"
    
    if [[ "$OUTPUT_FORMAT" == "json" ]] || [[ "$OUTPUT_FORMAT" == "both" ]]; then
        # JSON格式输出
        if echo "$output" | jq . &> /dev/null 2>&1; then
            local vulnerabilities=$(echo "$output" | jq '.vulnerabilities.found // false')
            local count=$(echo "$output" | jq '.vulnerabilities.count // 0')
            
            if [[ "$vulnerabilities" == "true" ]] && [[ $count -gt 0 ]]; then
                log_error "发现 $count 个安全漏洞"
                
                # 提取漏洞详情
                echo "$output" | jq -r '.vulnerabilities.list[] | 
                    "🚨 \(.advisory.id): \(.advisory.title)
                       严重程度: \(.advisory.severity // "unknown")
                       包: \(.package.name) v\(.package.version)
                       修复版本: \(.advisory.patched_versions // ["无"] | join(", "))
                       详情: \(.advisory.description)
                    "' 2>/dev/null || echo "$output"
                
                exit_code=1
            else
                log_success "✅ 未发现安全漏洞"
                exit_code=0
            fi
        else
            log_error "JSON解析失败"
            exit_code=1
        fi
    fi
    
    if [[ "$OUTPUT_FORMAT" == "human" ]] || [[ "$OUTPUT_FORMAT" == "both" ]]; then
        # 人类可读格式
        echo ""
        log_info "审计结果摘要:"
        
        if echo "$raw_output" | grep -q "Success"; then
            log_success "✅ 安全审计通过 - 未发现漏洞"
        elif echo "$raw_output" | grep -q "Vulnerability"; then
            log_error "❌ 发现安全漏洞"
            
            # 提取关键信息
            echo "$raw_output" | grep -E "(Vulnerability|RUSTSEC|CVE)" | head -10
            
            exit_code=1
        else
            echo "$raw_output"
            exit_code=1
        fi
    fi
    
    # 保存到文件
    if [[ -n "$OUTPUT_FILE" ]]; then
        echo "$raw_output" > "$OUTPUT_FILE"
        log_info "详细报告已保存到: $OUTPUT_FILE"
        
        # 生成摘要报告
        generate_summary_report "$raw_output" "$exit_code"
    fi
    
    return $exit_code
}

# 生成摘要报告
generate_summary_report() {
    local output="$1"
    local exit_code="$2"
    
    local summary_file="${OUTPUT_FILE%.json}_summary.txt"
    
    cat > "$summary_file" << EOF
Cargo Audit安全审计摘要报告
生成时间: $(date)
项目: ${PROJECT_NAME:-unknown} v${PROJECT_VERSION:-unknown}

$(if [[ $exit_code -eq 0 ]]; then
    echo "✅ 审计结果: 通过 - 未发现安全漏洞"
else
    echo "❌ 审计结果: 失败 - 发现安全漏洞"
fi)

$(echo "$output" | grep -E "(Vulnerability|RUSTSEC|CVE|advisory)" | head -20)

详细结果请查看: $OUTPUT_FILE
EOF
    
    log_info "摘要报告: $summary_file"
}

# 检查CI配置
check_ci_config() {
    log_info "检查CI配置..."
    
    local ci_files=(".github/workflows/security.yml" ".gitlab-ci.yml" "Jenkinsfile" "azure-pipelines.yml")
    local found_ci=false
    
    for ci_file in "${ci_files[@]}"; do
        if [[ -f "$ci_file" ]]; then
            log_info "发现CI配置文件: $ci_file"
            found_ci=true
            
            # 检查是否包含cargo audit
            if grep -q "cargo.*audit\|audit" "$ci_file"; then
                log_success "CI配置已包含安全审计"
            else
                log_warning "CI配置未包含cargo audit，建议添加"
                suggest_ci_integration "$ci_file"
            fi
            break
        fi
    done
    
    if [[ "$found_ci" == false ]]; then
        log_warning "未发现CI配置文件"
        suggest_ci_integration ""
    fi
}

# 建议CI集成
suggest_ci_integration() {
    local ci_file="$1"
    
    echo ""
    log_info "建议的CI集成配置:"
    echo ""
    
    if [[ "$ci_file" == *"github"* ]] || [[ -z "$ci_file" ]]; then
        cat << 'EOF'
# GitHub Actions 示例 (.github/workflows/security.yml)
name: Security Audit

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]
  schedule:
    - cron: '0 0 * * 1'  # 每周一运行

jobs:
  security-audit:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true
    - name: Install cargo-audit
      run: cargo install cargo-audit
    - name: Run security audit
      run: cargo audit
EOF
    fi
    
    if [[ "$ci_file" == *"gitlab"* ]] || [[ -z "$ci_file" ]]; then
        echo ""
        cat << 'EOF'
# GitLab CI 示例 (.gitlab-ci.yml)
security_audit:
  stage: test
  script:
    - cargo install cargo-audit
    - cargo audit
  only:
    - main
    - develop
    - merge_requests
  allow_failure: false
EOF
    fi
}

# 主函数
main() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  Cargo Audit安全审计工具${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
    
    check_dependencies
    get_project_info
    
    # 运行审计
    run_cargo_audit
    local audit_result=$?
    
    # 检查CI配置
    check_ci_config
    
    echo ""
    echo -e "${BLUE}========================================${NC}"
    
    if [[ $audit_result -eq 0 ]]; then
        echo -e "${GREEN}安全审计完成: 通过${NC}"
        exit 0
    else
        echo -e "${RED}安全审计完成: 失败 - 发现安全漏洞${NC}"
        exit 1
    fi
}

# 运行主函数
main "$@"