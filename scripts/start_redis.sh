#!/bin/bash
# 启动 Redis 测试服务
# 用法: ./scripts/start_redis.sh [type]
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

start_standalone() {
    log_info "启动单机 Redis..."

    if [ -f "$REAL_ENV_DIR/docker-compose.yml" ]; then
        cd "$REAL_ENV_DIR"
        docker-compose up -d
        cd "$PROJECT_ROOT"

        sleep 3

        if docker exec oxcache-redis-test redis-cli ping 2>/dev/null | grep -q "PONG"; then
            log_success "单机 Redis 已启动 (端口 6379)"
            export REDIS_URL="redis://127.0.0.1:6379"
            export OXCACHE_ALLOW_INSECURE_REDIS=1
            echo "REDIS_URL=redis://127.0.0.1:6379"
        fi
    fi
}

start_cluster() {
    log_info "启动 Redis Cluster..."

    if [ -f "$REAL_ENV_DIR/docker-compose.cluster.yml" ]; then
        cd "$REAL_ENV_DIR"
        docker-compose -f docker-compose.cluster.yml up -d
        cd "$PROJECT_ROOT"

        log_info "等待集群初始化..."
        sleep 20

        log_success "Redis Cluster 已启动 (端口 7000-7005)"
        echo "REDIS_CLUSTER_AVAILABLE=1"
    fi
}

start_sentinel() {
    log_info "启动 Redis Sentinel..."

    if [ -f "$REAL_ENV_DIR/docker-compose.sentinel.yml" ]; then
        cd "$REAL_ENV_DIR"
        docker-compose -f docker-compose.sentinel.yml up -d
        cd "$PROJECT_ROOT"

        log_info "等待 Sentinel 初始化..."
        sleep 25

        log_success "Redis Sentinel 已启动"
        echo "REDIS_SENTINEL_AVAILABLE=1"
    fi
}

start_all() {
    start_standalone
    start_cluster
    start_sentinel
}

main() {
    local type="${1:-standalone}"

    case "$type" in
        standalone)
            start_standalone
            ;;
        cluster)
            start_cluster
            ;;
        sentinel)
            start_sentinel
            ;;
        all)
            start_all
            ;;
        *)
            echo "用法: $0 [standalone|cluster|sentinel|all]"
            exit 1
            ;;
    esac
}

main "$@"
