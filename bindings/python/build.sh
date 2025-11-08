#!/bin/bash
# Build script for AniDB Client Python bindings

set -e

echo "Building AniDB Client Python bindings..."

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Get the directory of this script
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/../.."

# Check if we're in a virtual environment
if [[ -z "$VIRTUAL_ENV" ]]; then
    echo -e "${YELLOW}Warning: Not in a virtual environment. Consider using:${NC}"
    echo "  python -m venv venv"
    echo "  source venv/bin/activate"
    echo ""
fi

# Build the Rust library first
echo -e "${GREEN}Building Rust library...${NC}"
cd "$PROJECT_ROOT"
cargo build --release

# Copy the library to the Python package
echo -e "${GREEN}Copying native library...${NC}"
RUST_TARGET_DIR="$PROJECT_ROOT/target/release"
PYTHON_LIB_DIR="$SCRIPT_DIR/src/anidb_client"

# Create directory if it doesn't exist
mkdir -p "$PYTHON_LIB_DIR"

# Determine platform and copy appropriate library
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    cp "$RUST_TARGET_DIR/libanidb_client_core.dylib" "$PYTHON_LIB_DIR/"
    echo "Copied libanidb_client_core.dylib"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Linux
    cp "$RUST_TARGET_DIR/libanidb_client_core.so" "$PYTHON_LIB_DIR/"
    echo "Copied libanidb_client_core.so"
elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "cygwin" ]] || [[ "$OSTYPE" == "win32" ]]; then
    # Windows
    cp "$RUST_TARGET_DIR/anidb_client_core.dll" "$PYTHON_LIB_DIR/"
    echo "Copied anidb_client_core.dll"
else
    echo -e "${RED}Unknown platform: $OSTYPE${NC}"
    exit 1
fi

# Install Python package in development mode
echo -e "${GREEN}Installing Python package...${NC}"
cd "$SCRIPT_DIR"
pip install -e .

# Install development dependencies if requested
if [[ "$1" == "--dev" ]]; then
    echo -e "${GREEN}Installing development dependencies...${NC}"
    pip install -e ".[dev]"
fi

# Run tests if requested
if [[ "$1" == "--test" ]] || [[ "$2" == "--test" ]]; then
    echo -e "${GREEN}Running tests...${NC}"
    pytest
fi

echo -e "${GREEN}Build complete!${NC}"
echo ""
echo "To use the library:"
echo "  from anidb_client import AniDBClient"
echo ""
echo "Run examples with:"
echo "  python examples/basic_usage.py <file_path>"