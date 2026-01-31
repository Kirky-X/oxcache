#!/usr/bin/env bash
# Pre-commit hook for Cargo Audit
# Checks for security vulnerabilities

set -e

echo "Running cargo audit..."
if command -v cargo-audit &> /dev/null; then
    cargo audit
else
    echo "cargo-audit not installed, skipping audit"
    exit 0
fi
