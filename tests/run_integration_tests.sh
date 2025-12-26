#!/bin/bash

# Database Partitioning Integration Test Script
# This script sets up the Docker environment and runs tests for database partitioning

set -e

echo "=== Database Partitioning Integration Tests ==="
echo

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    local status=$1
    local message=$2
    if [ "$status" = "success" ]; then
        echo -e "${GREEN}✓ $message${NC}"
    elif [ "$status" = "error" ]; then
        echo -e "${RED}✗ $message${NC}"
    else
        echo -e "${YELLOW}→ $message${NC}"
    fi
}

# Check if Docker is running
print_status "info" "Checking Docker status..."
if ! docker info > /dev/null 2>&1; then
    print_status "error" "Docker is not running. Please start Docker first."
    exit 1
fi
print_status "success" "Docker is running"

# Navigate to the oxcache directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

# Start Docker services
print_status "info" "Starting Docker database services..."
docker-compose up -d postgres mysql sqlite

# Wait for services to be ready
print_status "info" "Waiting for database services to be ready..."
sleep 10

# Check service health
print_status "info" "Checking database service health..."

# Check PostgreSQL
if docker-compose exec -T postgres pg_isready -U postgres > /dev/null 2>&1; then
    print_status "success" "PostgreSQL is ready"
else
    print_status "error" "PostgreSQL is not ready"
fi

# Check MySQL
if docker-compose exec -T mysql mysqladmin ping -h localhost -u root -proot > /dev/null 2>&1; then
    print_status "success" "MySQL is ready"
else
    print_status "error" "MySQL is not ready"
fi

# Check SQLite (file-based)
if docker-compose exec -T sqlite test -f /data/oxcache_test.db > /dev/null 2>&1; then
    print_status "success" "SQLite database file exists"
else
    print_status "info" "SQLite database will be created during tests"
fi

# Run the tests
print_status "info" "Running database partitioning tests..."
if cargo test --test database_partitioning_tests -- --nocapture; then
    print_status "success" "All database partitioning tests passed"
    TEST_RESULT="success"
else
    print_status "error" "Some tests failed"
    TEST_RESULT="failed"
fi

# Optional: Run with more verbose output
if [ "$1" = "--verbose" ]; then
    print_status "info" "Running tests with verbose output..."
    RUST_LOG=debug cargo test --test database_partitioning_tests -- --nocapture
fi

# Cleanup
print_status "info" "Cleaning up Docker services..."
docker-compose down

# Summary
echo
echo "=== Test Summary ==="
if [ "$TEST_RESULT" = "success" ]; then
    print_status "success" "Database partitioning integration tests completed successfully!"
    exit 0
else
    print_status "error" "Database partitioning integration tests failed!"
    exit 1
fi