#!/bin/bash

# Bash build script for AniDB C# bindings

set -e

# Default values
CONFIGURATION="Release"
RUN_TESTS=false
PACK=false
CLEAN=false

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --configuration)
            CONFIGURATION="$2"
            shift 2
            ;;
        --run-tests)
            RUN_TESTS=true
            shift
            ;;
        --pack)
            PACK=true
            shift
            ;;
        --clean)
            CLEAN=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Get script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
SOLUTION_FILE="$SCRIPT_DIR/AniDBClient.sln"
OUTPUT_DIR="$SCRIPT_DIR/artifacts"

echo -e "\033[36mAniDB C# Bindings Build Script\033[0m"
echo -e "\033[36m==============================\033[0m"

# Clean if requested
if [ "$CLEAN" = true ]; then
    echo -e "\n\033[33mCleaning previous builds...\033[0m"
    
    if [ -d "$OUTPUT_DIR" ]; then
        rm -rf "$OUTPUT_DIR"
    fi
    
    dotnet clean "$SOLUTION_FILE" --configuration "$CONFIGURATION"
fi

# Restore dependencies
echo -e "\n\033[33mRestoring dependencies...\033[0m"
dotnet restore "$SOLUTION_FILE"

# Build solution
echo -e "\n\033[33mBuilding solution ($CONFIGURATION)...\033[0m"
dotnet build "$SOLUTION_FILE" --configuration "$CONFIGURATION" --no-restore

# Run tests if requested
if [ "$RUN_TESTS" = true ]; then
    echo -e "\n\033[33mRunning tests...\033[0m"
    
    TEST_PROJECT="$SCRIPT_DIR/src/AniDBClient.Tests/AniDBClient.Tests.csproj"
    
    dotnet test "$TEST_PROJECT" \
        --configuration "$CONFIGURATION" \
        --no-build \
        --logger "console;verbosity=normal" \
        --collect:"XPlat Code Coverage"
fi

# Create NuGet package if requested
if [ "$PACK" = true ]; then
    echo -e "\n\033[33mCreating NuGet package...\033[0m"
    
    PROJECT_FILE="$SCRIPT_DIR/src/AniDBClient/AniDBClient.csproj"
    
    # Ensure output directory exists
    mkdir -p "$OUTPUT_DIR"
    
    dotnet pack "$PROJECT_FILE" \
        --configuration "$CONFIGURATION" \
        --no-build \
        --output "$OUTPUT_DIR"
    
    echo -e "\n\033[32mPackage created in: $OUTPUT_DIR\033[0m"
fi

# Copy native libraries
echo -e "\n\033[33mCopying native libraries...\033[0m"

NATIVE_LIBS_SOURCE="$SCRIPT_DIR/../../target/release"
RUNTIMES_DIR="$SCRIPT_DIR/src/AniDBClient/runtimes"

# Detect platform
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    PLATFORM="linux-x64"
    LIB_FILE="libanidb_client_core.so"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    if [[ $(uname -m) == "arm64" ]]; then
        PLATFORM="osx-arm64"
    else
        PLATFORM="osx-x64"
    fi
    LIB_FILE="libanidb_client_core.dylib"
else
    echo "Unsupported platform: $OSTYPE"
    exit 1
fi

# Create runtime directory
RUNTIME_DIR="$RUNTIMES_DIR/$PLATFORM/native"
mkdir -p "$RUNTIME_DIR"

# Copy native library if it exists
SOURCE_FILE="$NATIVE_LIBS_SOURCE/$LIB_FILE"
if [ -f "$SOURCE_FILE" ]; then
    DEST_FILE="$RUNTIME_DIR/$LIB_FILE"
    cp -f "$SOURCE_FILE" "$DEST_FILE"
    echo -e "  \033[90mCopied $LIB_FILE to $PLATFORM\033[0m"
else
    echo -e "  \033[33mWarning: Native library not found at $SOURCE_FILE\033[0m"
fi

echo -e "\n\033[32mBuild completed successfully!\033[0m"

# Show summary
echo -e "\n\033[36mSummary:\033[0m"
echo -e "  \033[90mConfiguration: $CONFIGURATION\033[0m"
echo -e "  \033[90mTests Run: $RUN_TESTS\033[0m"
echo -e "  \033[90mPackage Created: $PACK\033[0m"

if [ "$PACK" = true ]; then
    echo -e "  \033[90mPackages:\033[0m"
    find "$OUTPUT_DIR" -name "*.nupkg" -exec basename {} \; | while read -r package; do
        echo -e "    \033[90m$package\033[0m"
    done
fi