#!/usr/bin/env bash
# Redis 服务管理 + 性能测试
# 用法: ./scripts/redis.sh <subcommand> [options]
#
# 子命令:
#   start           启动单机 Redis
#   stop            停止单机 Redis
#   start-cluster   启动 Redis Cluster
#   stop-cluster    停止 Redis Cluster
#   start-sentinel  启动 Redis Sentinel
#   stop-sentinel   停止 Redis Sentinel
#   restart         重启单机 Redis
#   perf            运行性能测试

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/common.sh"

PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REAL_ENV_DIR="$PROJECT_ROOT/tests/real_env"

REDIS_HOST="127.0.0.1"
REDIS_PORT="6381"
REDIS_CMD="docker exec corag-redis-1 redis-cli -p ${REDIS_PORT}"

show_help() {
    cat << EOF
用法: $(basename "$0") <subcommand> [options]

Redis 服务管理 + 性能测试

子命令:
  start           启动单机 Redis
  stop            停止单机 Redis
  start-cluster   启动 Redis Cluster
  stop-cluster    停止 Redis Cluster
  start-sentinel  启动 Redis Sentinel
  stop-sentinel   停止 Redis Sentinel
  restart         重启单机 Redis
  perf            运行性能测试
  help            显示帮助信息

示例:
  ./scripts/redis.sh start              # 启动单机 Redis
  ./scripts/redis.sh stop               # 停止单机 Redis
  ./scripts/redis.sh start-cluster      # 启动 Cluster
  ./scripts/redis.sh perf               # 性能测试
  ./scripts/redis.sh restart            # 重启
EOF
}

# ==================== 服务管理 ====================

cmd_start() {
    log_info "启动单机 Redis..."
    if [ -f "$REAL_ENV_DIR/docker-compose.yml" ]; then
        (cd "$REAL_ENV_DIR" && docker-compose up -d)
        sleep 3
        if docker exec oxcache-redis-test redis-cli ping 2>/dev/null | grep -q "PONG"; then
            log_success "单机 Redis 已启动 (端口 6379)"
            export REDIS_URL="redis://127.0.0.1:6379"
            export OXCACHE_ALLOW_INSECURE_REDIS=1
        else
            log_warning "Redis 启动后未响应"
        fi
    else
        log_error "$REAL_ENV_DIR/docker-compose.yml 不存在"
        exit 1
    fi
}

cmd_stop() {
    log_info "停止单机 Redis..."
    if [ -f "$REAL_ENV_DIR/docker-compose.yml" ]; then
        (cd "$REAL_ENV_DIR" && docker-compose down)
        log_success "单机 Redis 已停止"
    fi
}

cmd_start_cluster() {
    log_info "启动 Redis Cluster..."
    if [ -f "$REAL_ENV_DIR/docker-compose.cluster.yml" ]; then
        (cd "$REAL_ENV_DIR" && docker-compose -f docker-compose.cluster.yml up -d)
        log_info "等待集群初始化..."
        sleep 20
        log_success "Redis Cluster 已启动 (端口 7000-7005)"
        export REDIS_CLUSTER_AVAILABLE=1
    else
        log_error "$REAL_ENV_DIR/docker-compose.cluster.yml 不存在"
        exit 1
    fi
}

cmd_stop_cluster() {
    log_info "停止 Redis Cluster..."
    if [ -f "$REAL_ENV_DIR/docker-compose.cluster.yml" ]; then
        (cd "$REAL_ENV_DIR" && docker-compose -f docker-compose.cluster.yml down)
        log_success "Redis Cluster 已停止"
    fi
}

cmd_start_sentinel() {
    log_info "启动 Redis Sentinel..."
    if [ -f "$REAL_ENV_DIR/docker-compose.sentinel.yml" ]; then
        (cd "$REAL_ENV_DIR" && docker-compose -f docker-compose.sentinel.yml up -d)
        log_info "等待 Sentinel 初始化..."
        sleep 20
        log_success "Redis Sentinel 已启动"
        export REDIS_SENTINEL_AVAILABLE=1
    else
        log_error "$REAL_ENV_DIR/docker-compose.sentinel.yml 不存在"
        exit 1
    fi
}

cmd_stop_sentinel() {
    log_info "停止 Redis Sentinel..."
    if [ -f "$REAL_ENV_DIR/docker-compose.sentinel.yml" ]; then
        (cd "$REAL_ENV_DIR" && docker-compose -f docker-compose.sentinel.yml down)
        log_success "Redis Sentinel 已停止"
    fi
}

cmd_restart() {
    cmd_stop
    sleep 1
    cmd_start
}

# ==================== 性能测试 ====================

cmd_perf() {
    echo "🚀 Starting Redis Performance Tests"
    echo "==================================="
    echo ""

    # Test 1: PING latency
    echo "📡 Test 1: PING Latency"
    echo "Running 1000 PING operations..."
    PING_START=$(date +%s%N)
    for i in {1..1000}; do
        ${REDIS_CMD} PING > /dev/null 2>&1
    done
    PING_END=$(date +%s%N)
    PING_DURATION=$(( (PING_END - PING_START) / 1000000 ))
    PING_AVG=$(( PING_DURATION / 1000 ))
    echo "✅ 1000 PINGs completed in ${PING_DURATION}ms"
    echo "   Average latency: ${PING_AVG}µs"
    echo ""

    # Test 2: SET latency with different sizes
    echo "💾 Test 2: SET Latency (different data sizes)"
    for SIZE in 100 1000 10000; do
        echo "Testing SET with ${SIZE} bytes..."
        VALUE=$(python3 -c "print('x' * ${SIZE})")
        SET_START=$(date +%s%N)
        for i in {1..100}; do
            ${REDIS_CMD} SET "perf:test:size:${SIZE}:${i}" "${VALUE}" EX 60 > /dev/null 2>&1
        done
        SET_END=$(date +%s%N)
        SET_DURATION=$(( (SET_END - SET_START) / 1000000 ))
        SET_AVG=$(( SET_DURATION / 100 ))
        THROUGHPUT=$(( 100000 / SET_DURATION ))
        echo "   ✅ 100 SETs completed in ${SET_DURATION}ms (avg: ${SET_AVG}ms, ~${THROUGHPUT} ops/s)"
    done
    echo ""

    # Test 3: GET latency
    echo "📖 Test 3: GET Latency"
    echo "Running 1000 GET operations..."
    GET_START=$(date +%s%N)
    for i in {1..1000}; do
        ${REDIS_CMD} GET "perf:test:size:100:1" > /dev/null 2>&1
    done
    GET_END=$(date +%s%N)
    GET_DURATION=$(( (GET_END - GET_START) / 1000000 ))
    GET_AVG=$(( GET_DURATION / 1000 ))
    echo "✅ 1000 GETs completed in ${GET_DURATION}ms"
    echo "   Average latency: ${GET_AVG}µs"
    echo ""

    # Test 4: INCR latency
    echo "🔢 Test 4: INCR Latency"
    INCR_START=$(date +%s%N)
    for i in {1..500}; do
        ${REDIS_CMD} INCR "perf:test:counter" > /dev/null 2>&1
        ${REDIS_CMD} DEL "perf:test:counter" > /dev/null 2>&1
    done
    INCR_END=$(date +%s%N)
    INCR_DURATION=$(( (INCR_END - INCR_START) / 1000000 ))
    INCR_AVG=$(( INCR_DURATION / 500 ))
    echo "✅ 500 INCR/DEL pairs completed in ${INCR_DURATION}ms"
    echo "   Average latency: ${INCR_AVG}ms"
    echo ""

    # Test 5: Pipeline performance
    echo "📦 Test 5: Pipeline Performance"
    echo "Testing 1000 commands in pipeline..."
    PIPE_START=$(date +%s%N)
    for i in {1..100}; do
        CMD=""
        for j in {1..10}; do
            CMD="${CMD}SET perf:pipe:${i}:${j} value${j} EX 60;"
        done
        echo "${CMD}" | ${REDIS_CMD} --pipe > /dev/null 2>&1 || true
    done
    PIPE_END=$(date +%s%N)
    PIPE_DURATION=$(( (PIPE_END - PIPE_START) / 1000000 ))
    PIPE_OPS=$(( 1000 * 100 / PIPE_DURATION ))
    echo "✅ 100,000 pipelined commands completed in ${PIPE_DURATION}ms"
    echo "   Throughput: ~${PIPE_OPS} ops/s"
    echo ""

    # Cleanup
    echo "🧹 Cleanup test data..."
    ${REDIS_CMD} FLUSHDB > /dev/null 2>&1 || true
    echo ""

    echo "==================================="
    echo "🎉 Redis Performance Tests Complete!"
    echo ""
    echo "📊 Summary:"
    echo "   PING latency:       ~${PING_AVG}µs"
    echo "   SET (100B) latency: ~${SET_AVG}ms"
    echo "   GET latency:        ~${GET_AVG}µs"
    echo "   Pipeline throughput: ~${PIPE_OPS} ops/s"
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
        start)           cmd_start "$@" ;;
        stop)            cmd_stop "$@" ;;
        start-cluster)   cmd_start_cluster "$@" ;;
        stop-cluster)    cmd_stop_cluster "$@" ;;
        start-sentinel)  cmd_start_sentinel "$@" ;;
        stop-sentinel)   cmd_stop_sentinel "$@" ;;
        restart)         cmd_restart "$@" ;;
        perf)            cmd_perf "$@" ;;
        help|-h|--help)  show_help ;;
        *)
            log_error "未知子命令: $cmd"
            show_help
            exit 1
            ;;
    esac
}

main "$@"
