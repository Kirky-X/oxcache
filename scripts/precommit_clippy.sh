#!/usr/bin/env bash
# Pre-commit hook for Clippy checks
# Runs cargo clippy with strict warnings

set -e

echo "Running Clippy checks..."
cargo clippy --all-features -- -D warnings
