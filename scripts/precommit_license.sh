#!/usr/bin/env bash
# Pre-commit hook for license checks

set -e

echo "Checking license compliance..."
if command -v cargo-license &> /dev/null; then
    cargo license
else
    echo "cargo-license not installed, skipping license check"
    exit 0
fi
