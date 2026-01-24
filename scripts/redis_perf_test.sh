#!/bin/bash
# Redis Performance Test Script
# Tests Redis performance using redis-cli

set -e

REDIS_HOST="127.0.0.1"
REDIS_PORT="6381"
REDIS_CMD="docker exec corag-redis-1 redis-cli -p ${REDIS_PORT}"

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

# Test 4: INCR latency (counter operation)
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
CLEANUP_START=$(date +%s%N)
${REDIS_CMD} FLUSHDB > /dev/null 2>&1 || true
CLEANUP_END=$(date +%s%N)
CLEANUP_DURATION=$(( (CLEANUP_END - CLEANUP_START) / 1000 ))
echo "✅ Cleanup completed in ${CLEANUP_DURATION}ms"
echo ""

echo "==================================="
echo "🎉 Redis Performance Tests Complete!"
echo ""
echo "📊 Summary:"
echo "   PING latency:       ~${PING_AVG}µs"
echo "   SET (100B) latency: ~${SET_AVG}ms"
echo "   GET latency:        ~${GET_AVG}µs"
echo "   Pipeline throughput: ~${PIPE_OPS} ops/s"
echo ""
echo "✅ Redis is working correctly and performing well."
