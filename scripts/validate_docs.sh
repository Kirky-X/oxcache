#!/bin/bash

# Documentation Validation Script
# This script helps maintain consistency between documentation and code

set -e

echo "🔍 Validating Oxcache Documentation Consistency..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Track issues
issues=0

# Function to print colored output
print_issue() {
    echo -e "${RED}❌ $1${NC}"
    ((issues++))
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

echo "📋 Checking documentation files..."

# Check if all documented files exist
echo "1. Checking file existence..."

if [ ! -f "README.md" ]; then
    print_issue "README.md not found"
else
    print_success "README.md exists"
fi

if [ ! -f "README_zh.md" ]; then
    print_issue "README_zh.md not found"
else
    print_success "README_zh.md exists"
fi

if [ ! -f "docs/API_REFERENCE.md" ]; then
    print_issue "docs/API_REFERENCE.md not found"
else
    print_success "docs/API_REFERENCE.md exists"
fi

if [ ! -f "docs/ARCHITECTURE.md" ]; then
    print_issue "docs/ARCHITECTURE.md not found"
else
    print_success "docs/ARCHITECTURE.md exists"
fi

if [ ! -f "docs/USER_GUIDE.md" ]; then
    print_issue "docs/USER_GUIDE.md not found"
else
    print_success "docs/USER_GUIDE.md exists"
fi

if [ ! -f "docs/CHANGELOG.md" ]; then
    print_issue "docs/CHANGELOG.md not found"
else
    print_success "docs/CHANGELOG.md exists"
fi

# Check version consistency
echo "2. Checking version consistency..."

cargo_version=$(grep "^version = " Cargo.toml | head -1 | sed 's/version = "//' | sed 's/"//')
readme_version=$(grep "oxcache = \"" README.md | head -1 | sed 's/.*oxcache = "//' | sed 's/".*//')
readme_zh_version=$(grep "oxcache = \"" README_zh.md | head -1 | sed 's/.*oxcache = "//' | sed 's/".*//')

if [ "$cargo_version" = "$readme_version" ]; then
    print_success "README.md version matches Cargo.toml ($cargo_version)"
else
    print_issue "README.md version ($readme_version) doesn't match Cargo.toml ($cargo_version)"
fi

if [ "$cargo_version" = "$readme_zh_version" ]; then
    print_success "README_zh.md version matches Cargo.toml ($cargo_version)"
else
    print_issue "README_zh.md version ($readme_zh_version) doesn't match Cargo.toml ($cargo_version)"
fi

# Check for broken links
echo "3. Checking for broken documentation links..."

# Check for non-existent /docs/zh/ links
if grep -r "docs/zh/" README*.md docs/; then
    print_warning "Found references to non-existent /docs/zh/ paths"
    echo "   These should be updated to point to existing documentation"
fi

# Check configuration examples
echo "4. Checking configuration examples..."

# Verify that cache_type values are documented correctly
if grep -q "cache_type.*\"l1\"" README.md && grep -q "cache_type.*\"l2\"" README.md; then
    print_success "README.md includes L1 and L2 cache type examples"
else
    print_warning "README.md should include examples for all cache types (l1, l2, two-level)"
fi

if grep -q "cache_type.*\"l1\"" README_zh.md && grep -q "cache_type.*\"l2\"" README_zh.md; then
    print_success "README_zh.md includes L1 and L2 cache type examples"
else
    print_warning "README_zh.md should include examples for all cache types (l1, l2, two-level)"
fi

# Check feature documentation
echo "5. Checking feature documentation..."

# Verify that features are documented in README files
if grep -q "features.*full" README.md && grep -q "features.*core" README.md && grep -q "features.*minimal" README.md; then
    print_success "README.md documents feature tiers (full, core, minimal)"
else
    print_warning "README.md should document all feature tiers"
fi

# Check for performance claims with environment info
echo "6. Checking performance documentation..."

if grep -q "Test environment.*Redis" README.md; then
    print_success "README.md includes test environment details for performance data"
else
    print_warning "README.md should include test environment details for performance claims"
fi

if grep -q "测试环境.*Redis" README_zh.md; then
    print_success "README_zh.md includes test environment details for performance data"
else
    print_warning "README_zh.md should include test environment details for performance claims"
fi

# Check API reference for feature requirements
echo "7. Checking API reference feature requirements..."

if grep -q "Required Features.*config-toml.*confers" docs/API_REFERENCE.md; then
    print_success "API_REFERENCE.md documents feature requirements"
else
    print_warning "API_REFERENCE.md should clearly document feature requirements"
fi

# Summary
echo ""
echo "📊 Validation Summary:"
if [ $issues -eq 0 ]; then
    print_success "All checks passed! Documentation is consistent."
    exit 0
else
    echo -e "${RED}Found $issues issues that need attention.${NC}"
    echo ""
    echo "🔧 Recommended actions:"
    echo "1. Fix version inconsistencies"
    echo "2. Update broken links"
    echo "3. Add missing configuration examples"
    echo "4. Document feature requirements clearly"
    echo "5. Add test environment details for performance claims"
    exit 1
fi
