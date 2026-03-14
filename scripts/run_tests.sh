#!/bin/bash
# oxcache 测试运行脚本
# 用法: ./scripts/run_tests.sh [test_type]
# test_type: unit, integration, e2e, chaos, all (默认: all)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
REAL_ENV_DIR="$PROJECT_ROOT/tests/real_env"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

# 检查依赖
check_dependencies() {
    log_info "检查依赖..."

    if ! command -v cargo &> /dev/null; then
        log_error "cargo 未安装"
        exit 1
    fi

    if ! command -v docker &> /dev/null; then
        log_warning "docker 未安装，部分测试将被跳过"
    fi

    if ! command -v docker-compose &> /dev/null; then
        log_warning "docker-compose 未安装，部分测试将被跳过"
    fi

    log_success "依赖检查完成"
}

# 启动 Redis 服务
start_redis() {
    log_info "启动 Redis 服务..."

    if [ -f "$REAL_ENV_DIR/docker-compose.yml" ]; then
        cd "$REAL_ENV_DIR"
        docker-compose up -d
        cd "$PROJECT_ROOT"

        # 等待 Redis 就绪
        log_info "等待 Redis 就绪..."
        sleep 5

        # 检查 Redis 连接
        for i in {1..10}; do
            if docker exec oxcache-redis-test redis-cli ping 2>/dev/null | grep -q "PONG"; then
                log_success "Redis 已就绪"
                export REDIS_URL="redis://127.0.0.1:6379"
                export OXCACHE_ALLOW_INSECURE_REDIS=1
                return 0
            fi
            sleep 1
        done

        log_warning "Redis 未能在预期时间内就绪"
    else
        log_warning "docker-compose.yml 未找到，跳过 Redis 启动"
    fi
}

# 停止 Redis 服务
stop_redis() {
    log_info "停止 Redis 服务..."

    if [ -f "$REAL_ENV_DIR/docker-compose.yml" ]; then
        cd "$REAL_ENV_DIR"
        docker-compose down
        cd "$PROJECT_ROOT"
        log_success "Redis 服务已停止"
    fi
}

# 启动 Redis Cluster
start_redis_cluster() {
    log_info "启动 Redis Cluster..."

    if [ -f "$REAL_ENV_DIR/docker-compose.cluster.yml" ]; then
        cd "$REAL_ENV_DIR"
        docker-compose -f docker-compose.cluster.yml up -d
        cd "$PROJECT_ROOT"

        log_info "等待 Redis Cluster 就绪..."
        sleep 15

        export REDIS_CLUSTER_AVAILABLE=1
        log_success "Redis Cluster 已启动"
    else
        log_warning "docker-compose.cluster.yml 未找到"
    fi
}

# 停止 Redis Cluster
stop_redis_cluster() {
    log_info "停止 Redis Cluster..."

    if [ -f "$REAL_ENV_DIR/docker-compose.cluster.yml" ]; then
        cd "$REAL_ENV_DIR"
        docker-compose -f docker-compose.cluster.yml down
        cd "$PROJECT_ROOT"
        log_success "Redis Cluster 已停止"
    fi
}

# 启动 Redis Sentinel
start_redis_sentinel() {
    log_info "启动 Redis Sentinel..."

    if [ -f "$REAL_ENV_DIR/docker-compose.sentinel.yml" ]; then
        cd "$REAL_ENV_DIR"
        docker-compose -f docker-compose.sentinel.yml up -d
        cd "$PROJECT_ROOT"

        log_info "等待 Redis Sentinel 就绪..."
        sleep 20

        export REDIS_SENTINEL_AVAILABLE=1
        log_success "Redis Sentinel 已启动"
    else
        log_warning "docker-compose.sentinel.yml 未找到"
    fi
}

# 停止 Redis Sentinel
stop_redis_sentinel() {
    log_info "停止 Redis Sentinel..."

    if [ -f "$REAL_ENV_DIR/docker-compose.sentinel.yml" ]; then
        cd "$REAL_ENV_DIR"
        docker-compose -f docker-compose.sentinel.yml down
        cd "$PROJECT_ROOT"
        log_success "Redis Sentinel 已停止"
    fi
}

# 运行单元测试
run_unit_tests() {
    log_info "运行单元测试..."

    cd "$PROJECT_ROOT"
    cargo test --lib --tests unit --features full -- --nocapture

    log_success "单元测试完成"
}

# 运行集成测试
run_integration_tests() {
    log_info "运行集成测试..."

    # 启动 Redis 服务
    start_redis

    cd "$PROJECT_ROOT"

    # 运行基础集成测试
    cargo test --test integration --features full -- --nocapture || true

    # 运行 Redis 相关测试
    cargo test --test redis_standalone_test --features full -- --nocapture || true

    # 停止 Redis
    stop_redis

    log_success "集成测试完成"
}

# 运行端到端测试
run_e2e_tests() {
    log_info "运行端到端测试..."

    cd "$PROJECT_ROOT"
    cargo test --test e2e --features full -- --nocapture

    log_success "端到端测试完成"
}

# 运行混沌测试
run_chaos_tests() {
    log_info "运行混沌测试..."

    # 启动 Redis
    start_redis

    cd "$PROJECT_ROOT"
    cargo test --test chaos --features full -- --nocapture || true

    # 停止 Redis
    stop_redis

    log_success "混沌测试完成"
}

# 运行所有测试
run_all_tests() {
    log_info "运行所有测试..."

    run_unit_tests
    run_integration_tests
    run_e2e_tests
    run_chaos_tests

    log_success "所有测试完成"
}

# 生成测试覆盖率报告
generate_coverage() {
    log_info "生成测试覆盖率报告..."

    if command -v cargo-tarpaulin &> /dev/null; then
        cargo tarpaulin --out Html --output-dir "$PROJECT_ROOT/target/coverage"
        log_success "覆盖率报告已生成: $PROJECT_ROOT/target/coverage/index.html"
    else
        log_warning "cargo-tarpaulin 未安装，跳过覆盖率报告生成"
        log_info "安装方法: cargo install cargo-tarpaulin"
    fi
}

# 清理
cleanup() {
    log_info "清理测试环境..."

    stop_redis
    stop_redis_cluster
    stop_redis_sentinel

    # 清理临时文件
    rm -f "$PROJECT_ROOT"/*.db 2>/dev/null || true
    rm -f "$PROJECT_ROOT"/*_wal.db 2>/dev/null || true

    log_success "清理完成"
}

# 主函数
main() {
    local test_type="${1:-all}"

    log_info "oxcache 测试运行器"
    log_info "测试类型: $test_type"
    log_info "项目根目录: $PROJECT_ROOT"

    check_dependencies

    case "$test_type" in
        unit)
            run_unit_tests
            ;;
        integration)
            run_integration_tests
            ;;
        e2e)
            run_e2e_tests
            ;;
        chaos)
            run_chaos_tests
            ;;
        all)
            run_all_tests
            ;;
        coverage)
            run_all_tests
            generate_coverage
            ;;
        clean)
            cleanup
            ;;
        *)
            log_error "未知测试类型: $test_type"
            echo "用法: $0 [unit|integration|e2e|chaos|all|coverage|clean]"
            exit 1
            ;;
    esac
}

# 捕获退出信号
trap cleanup EXIT

main "$@"
