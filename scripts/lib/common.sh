#!/bin/bash
# Oxcache Scripts Common Library
# 公共函数库，提供颜色输出、日志、依赖检查等功能

# ==================== 颜色定义 ====================
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# ==================== 日志函数 ====================
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

# ==================== 进度显示函数 ====================
print_header() {
    local title="$1"
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  $title${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
}

print_section() {
    local section="$1"
    echo ""
    echo -e "${YELLOW}=== $section ===${NC}"
}

# ==================== 依赖检查函数 ====================
check_command() {
    local cmd="$1"
    local install_hint="$2"

    if ! command -v "$cmd" &> /dev/null; then
        log_error "$cmd 未安装"
        if [[ -n "$install_hint" ]]; then
            log_error "安装提示: $install_hint"
        fi
        return 1
    fi
    return 0
}

check_cargo() {
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo未安装。请安装Rust工具链。"
        return 1
    fi
    return 0
}

check_rustup_component() {
    local component="$1"
    if ! rustup component list --installed | grep -q "$component"; then
        log_warning "$component 未安装"
        return 1
    fi
    return 0
}

# ==================== 工具检查函数 ====================
check_miri() {
    if ! rustup component list --installed | grep -q miri; then
        log_warning "miri 未安装，正在安装..."
        if rustup component add miri; then
            log_success "miri 安装成功"
            return 0
        else
            log_error "miri 安装失败"
            return 1
        fi
    fi
    return 0
}

check_valgrind() {
    if ! command -v valgrind &> /dev/null; then
        log_warning "valgrind 未安装"
        log_info "安装提示:"
        log_info "  Ubuntu/Debian: sudo apt-get install valgrind"
        log_info "  CentOS/RHEL: sudo yum install valgrind"
        log_info "  macOS: brew install valgrind"
        return 1
    fi
    return 0
}

check_docker() {
    if ! command -v docker &> /dev/null; then
        log_error "Docker未安装"
        return 1
    fi

    if ! command -v docker-compose &> /dev/null; then
        log_error "Docker Compose未安装"
        return 1
    fi

    if ! docker info &> /dev/null; then
        log_error "Docker服务未运行"
        return 1
    fi

    return 0
}

# ==================== 报告生成函数 ====================
generate_markdown_report() {
    local report_file="$1"
    local title="$2"
    shift 2
    local sections=("$@")

    cat > "$report_file" << EOF
# $title

生成时间: $(date)

EOF

    for section in "${sections[@]}"; do
        echo "$section" >> "$report_file"
        echo "" >> "$report_file"
    done

    log_info "报告已生成: $report_file"
}

# ==================== 超时执行函数 ====================
run_with_timeout() {
    local timeout="$1"
    shift
    local command=("$@")

    if timeout "$timeout" "${command[@]}"; then
        return 0
    else
        local exit_code=$?
        if [[ $exit_code -eq 124 ]]; then
            log_error "命令执行超时（${timeout}秒）"
        else
            log_error "命令执行失败（退出码: $exit_code）"
        fi
        return 1
    fi
}

# ==================== 文件操作函数 ====================
ensure_directory() {
    local dir="$1"
    if [[ ! -d "$dir" ]]; then
        mkdir -p "$dir"
        log_info "创建目录: $dir"
    fi
}

# ==================== 版本信息函数 ====================
get_project_version() {
    if [[ -f "Cargo.toml" ]]; then
        grep "^version = " Cargo.toml | head -1 | sed 's/version = "//' | sed 's/"//'
    fi
}

get_project_name() {
    if [[ -f "Cargo.toml" ]]; then
        cargo metadata --no-deps --format-version 1 2>/dev/null | grep -o '"name":"[^"]*"' | head -1 | cut -d'"' -f4
    fi
}

# ==================== 测试结果统计 ====================
TestStats=""
init_test_stats() {
    TestStats="total=0 passed=0 failed=0 skipped=0"
}

update_test_stats() {
    local result="$1"  # passed, failed, skipped
    TestStats=$(echo "$TestStats" | awk -v result="$result" '{
        total += 1
        if (result == "passed") passed += 1
        else if (result == "failed") failed += 1
        else if (result == "skipped") skipped += 1
        print "total=" total " passed=" passed " failed=" failed " skipped=" skipped
    }')
}

print_test_stats() {
    echo "$TestStats" | awk '{
        print "总测试数: " total
        print "\033[0;32m通过: " passed "\033[0m"
        print "\033[0;31m失败: " failed "\033[0m"
        print "\033[1;33m跳过: " skipped "\033[0m"
    }'
}

# ==================== 环境变量工具 ====================
set_env_if_empty() {
    local var_name="$1"
    local default_value="$2"
    if [[ -z "${!var_name}" ]]; then
        export "$var_name"="$default_value"
    fi
}

# ==================== 退出处理 ====================
cleanup_on_exit() {
    local cleanup_func="$1"
    trap "$cleanup_func" EXIT
}

# ==================== 初始化 ====================
# 设置错误处理
set -e

# 初始化测试统计
init_test_stats
