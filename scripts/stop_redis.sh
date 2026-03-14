#!/bin/bash
# 停止 Redis 测试服务
# 用法: ./scripts/stop_redis.sh [type]
# type: standalone (默认), cluster, sentinel, all

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
REAL_ENV_DIR="$PROJECT_ROOT/tests/real_env"

# 颜色输出
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

stop_standalone() {
    log_info "停止单机 Redis..."

    if [ -f "$REAL_ENV_DIR/docker-compose.yml" ]; then
        cd "$REAL_ENV_DIR"
        docker-compose down
        cd "$PROJECT_ROOT"
        log_success "单机 Redis 已停止"
    fi
}

stop_cluster() {
    log_info "停止 Redis Cluster..."

    if [ -f "$REAL_ENV_DIR/docker-compose.cluster.yml" ]; then
        cd "$REAL_ENV_DIR"
        docker-compose -f docker-compose.cluster.yml down
        cd "$PROJECT_ROOT"
        log_success "Redis Cluster 已停止"
    fi
}

stop_sentinel() {
    log_info "停止 Redis Sentinel..."

    if [ -f "$REAL_ENV_DIR/docker-compose.sentinel.yml" ]; then
        cd "$REAL_ENV_DIR"
        docker-compose -f docker-compose.sentinel.yml down
        cd "$PROJECT_ROOT"
        log_success "Redis Sentinel 已停止"
    fi
}

stop_all() {
    stop_standalone
    stop_cluster
    stop_sentinel
}

main() {
    local type="${1:-standalone}"

    case "$type" in
        standalone)
            stop_standalone
            ;;
        cluster)
            stop_cluster
            ;;
        sentinel)
            stop_sentinel
            ;;
        all)
            stop_all
            ;;
        *)
            echo "用法: $0 [standalone|cluster|sentinel|all]"
            exit 1
            ;;
    esac
}

main "$@"
