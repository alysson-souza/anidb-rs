#!/bin/bash

# Build script for AniDB Swift bindings

set -e

echo "Building AniDB Swift bindings..."
echo "==============================="

# Check if we're in the right directory
if [ ! -f "Package.swift" ]; then
    echo "Error: Package.swift not found. Please run this script from the bindings/swift directory."
    exit 1
fi

# Clean previous build
echo "Cleaning previous build..."
swift package clean

# Build the library
echo "Building library..."
swift build -c release

# Run tests
echo "Running tests..."
swift test

# Build the example
echo "Building example application..."
swift build -c release --product anidb-example

echo ""
echo "Build completed successfully!"
echo ""
echo "Library location: .build/release/libAniDBClient.dylib"
echo "Example binary: .build/release/anidb-example"
echo ""
echo "To use in your project, add this package as a dependency."