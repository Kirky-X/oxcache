#!/bin/bash
# Install pre-commit hooks for Oxcache project

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo ""
echo "🔧 Installing pre-commit hooks for Oxcache..."
echo ""

# Check if we're in a git repository
if [ ! -d "$PROJECT_ROOT/.git" ]; then
    print_error "Not a git repository. Please run this script from the project root."
    exit 1
fi

# Check if pre-commit is installed
if ! command -v pre-commit &> /dev/null; then
    print_warning "pre-commit not found. Installing..."

    # Try pip first
    if command -v pip &> /dev/null; then
        pip install pre-commit
    elif command -v pip3 &> /dev/null; then
        pip3 install pre-commit
    elif command -v python -m pip &> /dev/null; then
        python -m pip install pre-commit
    else
        print_error "pip not found. Please install pre-commit manually:"
        print_error "  pip install pre-commit"
        exit 1
    fi
fi

print_success "pre-commit is installed: $(pre-commit --version)"

# Install the pre-commit hooks
print_info "Installing git hooks..."
cd "$PROJECT_ROOT"
pre-commit install

# Install commit-msg hook if needed
if [ -f ".pre-commit-config.yaml" ] && grep -q "commit-msg" .pre-commit-config.yaml; then
    pre-commit install --hook-type commit-msg
fi

# Update hooks to latest versions
print_info "Updating hooks to latest versions..."
pre-commit autoupdate 2>/dev/null || true

# Make the custom hook script executable
if [ -f "$SCRIPT_DIR/pre-commit" ]; then
    chmod +x "$SCRIPT_DIR/pre-commit"
    print_success "Made custom pre-commit script executable"
fi

echo ""
print_success "Pre-commit hooks installed successfully!"
echo ""
echo "📋 Available hooks:"
pre-commit hooks 2>/dev/null || echo "  Run 'pre-commit hooks' to see available hooks"
echo ""
echo "📝 Usage:"
echo "  - Hooks will run automatically on 'git commit'"
echo "  - Run manually: pre-commit run --all-files"
echo "  - Skip hooks: git commit --no-verify"
echo ""
echo "📚 Documentation: scripts/pre-commit/README.md"
echo ""
