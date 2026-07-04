#!/usr/bin/env bash
# ============================================================================
# Feature Matrix Compile/Test Script
#
# Verifies that all supported feature combinations compile and that the
# narrowest supported feature set (`minimal`) passes its test suite.
#
# This script exists because `cargo test` can only target one feature
# combination per invocation — we need multiple invocations to cover the
# matrix. CI should run this as a dedicated job alongside `cargo test
# --all-features`.
#
# Supported configurations (per oxcache Cargo.toml):
#   - minimal: L1 memory cache only (memory + tracing + metrics + serialization + chrono)
#   - core:    L1 + L2 Redis (minimal + redis + futures)
#   - full:    All features enabled (default)
#
# memory-only is NOT a supported configuration for the high-level Cache API
# (cache module requires serialization+tracing). Users wanting only L1 memory
# storage should use `minimal` instead.
# ============================================================================

set -euo pipefail

cd "$(dirname "$0")/.."

echo "=== Feature matrix compile check ==="
echo ""

# 4 feature combinations covering all supported configurations + edge cases
# - minimal/core/full: tiered feature sets documented in Cargo.toml
# - memory,serialization,tracing,metrics: equivalent to minimal without chrono,
#   verifies that the cfg-gate fix works for the minimal feature's dep set
FEATURES_LIST=(
    "--no-default-features --features minimal"
    "--no-default-features --features core"
    "--no-default-features --features full"
    "--no-default-features --features memory,serialization,tracing,metrics"
)

FAILED=()

for features in "${FEATURES_LIST[@]}"; do
    echo "--- cargo check $features --lib ---"
    if cargo check $features --lib 2>&1 | tail -5; then
        echo "[PASS] $features"
    else
        echo "[FAIL] $features"
        FAILED+=("$features")
    fi
    echo ""
done

echo "=== Narrow-feature test (minimal) ==="
if cargo test --no-default-features --features minimal --lib 2>&1 | tail -20; then
    echo "[PASS] cargo test --no-default-features --features minimal --lib"
else
    echo "[FAIL] cargo test --no-default-features --features minimal --lib"
    FAILED+=("cargo test --no-default-features --features minimal --lib")
fi

echo ""
echo "=== Summary ==="
if [ ${#FAILED[@]} -eq 0 ]; then
    echo "All ${#FEATURES_LIST[@]} feature combinations + minimal test passed."
    exit 0
else
    echo "FAILED (${#FAILED[@]}):"
    for f in "${FAILED[@]}"; do
        echo "  - $f"
    done
    exit 1
fi
