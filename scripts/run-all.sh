#!/usr/bin/env bash
# Run All Scripts — 统一入口

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/common.sh"

show_help() {
    cat << EOF
用法: $0 [选项]

选项:
  --pre-commit        运行预提交 7 阶段检查
  --tests             运行所有测试
  --validation        运行全部验证 (特性/文档/安全)
  --performance       运行 Redis 性能测试
  --all               运行所有
  -h, --help          显示帮助

示例:
  $0 --pre-commit      # 代码检查
  $0 --validation      # 验证
EOF
}

case "${1:-all}" in
    --pre-commit)
        log_info "Running pre-commit checks..."
        if [ -f "$SCRIPT_DIR/hooks/pre-commit.sh" ]; then
            bash "$SCRIPT_DIR/hooks/pre-commit.sh"
        else
            log_error "scripts/hooks/pre-commit.sh 未找到"
            log_info "请运行: bash scripts/hooks/setup-hooks.sh"
            exit 1
        fi
        ;;
    --tests)
        log_info "Running all tests..."
        bash "$SCRIPT_DIR/tests/run_all_tests.sh" all
        ;;
    --validation)
        log_info "Running validation..."
        bash "$SCRIPT_DIR/validate.sh" all
        ;;
    --performance)
        log_info "Running Redis performance tests..."
        bash "$SCRIPT_DIR/redis.sh" perf
        ;;
    --all)
        log_info "Running all scripts..."

        # 预提交检查
        if [ -f "$SCRIPT_DIR/hooks/pre-commit.sh" ]; then
            bash "$SCRIPT_DIR/hooks/pre-commit.sh" || true
        fi

        # 验证
        log_info "Running validation..."
        bash "$SCRIPT_DIR/validate.sh" all || true

        # 测试
        log_info "Running tests..."
        bash "$SCRIPT_DIR/tests/run_all_tests.sh" all || true

        # 性能
        log_info "Running performance tests..."
        bash "$SCRIPT_DIR/redis.sh" perf || true
        ;;
    -h|--help)
        show_help
        exit 0
        ;;
    *)
        log_error "Unknown option: $1"
        show_help
        exit 1
        ;;
esac
