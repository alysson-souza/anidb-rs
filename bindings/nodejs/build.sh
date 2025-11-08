#!/bin/bash
set -e

echo "Building AniDB Client for Node.js..."
echo "===================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if we're in the right directory
if [ ! -f "package.json" ]; then
    echo -e "${RED}Error: package.json not found. Please run from the nodejs bindings directory.${NC}"
    exit 1
fi

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check prerequisites
echo "Checking prerequisites..."

if ! command_exists node; then
    echo -e "${RED}Error: Node.js is not installed${NC}"
    exit 1
fi

if ! command_exists npm; then
    echo -e "${RED}Error: npm is not installed${NC}"
    exit 1
fi

if ! command_exists python3 && ! command_exists python; then
    echo -e "${YELLOW}Warning: Python not found. It may be required for node-gyp${NC}"
fi

NODE_VERSION=$(node -v)
echo "Node.js version: $NODE_VERSION"

# Check Node.js version (14.0.0 minimum)
NODE_MAJOR=$(echo $NODE_VERSION | cut -d. -f1 | sed 's/v//')
if [ "$NODE_MAJOR" -lt 14 ]; then
    echo -e "${RED}Error: Node.js 14.0.0 or higher is required${NC}"
    exit 1
fi

# Build the Rust library first if needed
RUST_LIB_PATH="../../target/release/libanidb_client_core.a"
if [ ! -f "$RUST_LIB_PATH" ]; then
    echo -e "${YELLOW}Rust library not found. Building...${NC}"
    cd ../..
    cargo build --release
    cd bindings/nodejs
    
    if [ ! -f "$RUST_LIB_PATH" ]; then
        echo -e "${RED}Error: Failed to build Rust library${NC}"
        exit 1
    fi
fi

echo -e "${GREEN}✓ Rust library found${NC}"

# Clean previous builds
echo ""
echo "Cleaning previous builds..."
rm -rf build dist node_modules

# Install dependencies
echo ""
echo "Installing dependencies..."
npm install

# Build native module
echo ""
echo "Building native module..."
npm run build:native

if [ $? -ne 0 ]; then
    echo -e "${RED}Error: Native module build failed${NC}"
    exit 1
fi

# Build TypeScript
echo ""
echo "Building TypeScript..."
npm run build:ts

if [ $? -ne 0 ]; then
    echo -e "${RED}Error: TypeScript build failed${NC}"
    exit 1
fi

# Run tests
echo ""
echo "Running tests..."
npm test

if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✓ Build completed successfully!${NC}"
    echo ""
    echo "You can now:"
    echo "  - Run examples: npm run example:basic"
    echo "  - Use in your project: const { AniDBClient } = require('./dist')"
else
    echo -e "${YELLOW}Warning: Tests failed but build completed${NC}"
fi

# Display package info
echo ""
echo "Package information:"
echo "==================="
npm list --depth=0

echo ""
echo "Native module location: build/Release/anidb_client.node"
echo "TypeScript output: dist/"