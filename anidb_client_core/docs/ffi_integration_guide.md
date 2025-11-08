# AniDB Client FFI Integration Guide

## Table of Contents

1. [Overview](#overview)
2. [C/C++ Integration](#cc-integration)
3. [Python Integration](#python-integration)
4. [Node.js Integration](#nodejs-integration)
5. [C# Integration](#c-integration)
6. [Swift Integration](#swift-integration)
7. [Build Instructions](#build-instructions)
8. [Platform-Specific Notes](#platform-specific-notes)
9. [Deployment Guide](#deployment-guide)

## Overview

This guide provides detailed instructions for integrating the AniDB Client library into applications written in various programming languages. The library provides a C-compatible FFI (Foreign Function Interface) that can be used from any language that supports C bindings.

### Prerequisites

- Rust toolchain (for building from source)
- Target language development environment
- Platform-specific build tools

### Library Files

After building, you'll have:
- **Dynamic Library**: `libanidb_client_core.so` (Linux), `libanidb_client_core.dylib` (macOS), `anidb_client_core.dll` (Windows)
- **Header File**: `anidb.h` (C/C++ only)
- **Import Library**: `anidb_client_core.lib` (Windows only)

## C/C++ Integration

### Building the Library

```bash
# Build release version
cargo build --release

# The library will be in target/release/
```

### CMake Integration

Create a `CMakeLists.txt`:

```cmake
cmake_minimum_required(VERSION 3.10)
project(anidb_example)

set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)

# Find the AniDB library
find_library(ANIDB_LIB 
    NAMES anidb_client_core
    PATHS ${CMAKE_CURRENT_SOURCE_DIR}/lib
)

# Include directory
include_directories(${CMAKE_CURRENT_SOURCE_DIR}/include)

# Your executable
add_executable(example main.c)

# Link the library
target_link_libraries(example ${ANIDB_LIB})

# Platform-specific settings
if(UNIX AND NOT APPLE)
    target_link_libraries(example pthread dl m)
elseif(APPLE)
    target_link_libraries(example pthread)
endif()
```

### Manual Compilation

```bash
# Linux/macOS
gcc -o example example.c -L./lib -lanidb_client_core -lpthread -ldl -lm

# Windows (MinGW)
gcc -o example.exe example.c -L./lib -lanidb_client_core -lws2_32 -lbcrypt -lntdll

# Windows (MSVC)
cl example.c /I.\include /link .\lib\anidb_client_core.lib ws2_32.lib bcrypt.lib ntdll.lib
```

### Example Program

```c
#include <stdio.h>
#include <stdlib.h>
#include "anidb.h"

int main() {
    // Initialize library
    if (anidb_init(ANIDB_ABI_VERSION) != ANIDB_SUCCESS) {
        fprintf(stderr, "Failed to initialize library\n");
        return 1;
    }
    
    // Create client
    anidb_client_handle_t client;
    anidb_result_t result = anidb_client_create(&client);
    if (result != ANIDB_SUCCESS) {
        fprintf(stderr, "Failed to create client: %s\n", 
                anidb_error_string(result));
        anidb_cleanup();
        return 1;
    }
    
    // Process a file
    anidb_hash_algorithm_t algorithms[] = { ANIDB_HASH_ED2K };
    anidb_process_options_t options = {
        .algorithms = algorithms,
        .algorithm_count = 1,
        .enable_progress = 0,
        .verify_existing = 0,
        .progress_callback = NULL,
        .user_data = NULL
    };
    
    anidb_file_result_t* file_result;
    result = anidb_process_file(client, "video.mkv", &options, &file_result);
    
    if (result == ANIDB_SUCCESS) {
        printf("File processed successfully\n");
        for (size_t i = 0; i < file_result->hash_count; i++) {
            printf("%s: %s\n", 
                   anidb_hash_algorithm_name(file_result->hashes[i].algorithm),
                   file_result->hashes[i].hash_value);
        }
        anidb_free_file_result(file_result);
    }
    
    // Cleanup
    anidb_client_destroy(client);
    anidb_cleanup();
    
    return 0;
}
```

## Python Integration

### Installation

The Python bindings are available in the `bindings/python` directory.

```bash
cd bindings/python

# Install in development mode
pip install -e .

# Or build and install
python setup.py build
pip install .
```

### Requirements

```python
# requirements.txt
cffi>=1.15.0
typing-extensions>=4.0.0
```

### Usage Example

```python
import anidb_client
from anidb_client import HashAlgorithm, ProcessOptions

# Initialize the library (done automatically on import)
# Create a client
client = anidb_client.Client()

# Configure with custom settings
config_client = anidb_client.Client(
    cache_dir="/home/user/.anidb_cache",
    max_concurrent_files=4,
    chunk_size=65536
)

# Process a single file
result = client.process_file(
    "video.mkv",
    algorithms=[HashAlgorithm.ED2K, HashAlgorithm.CRC32],
    progress_callback=lambda p, b, t: print(f"Progress: {p:.1f}%")
)

print(f"File: {result.file_path}")
print(f"Size: {result.file_size} bytes")
for algo, hash_value in result.hashes.items():
    print(f"{algo}: {hash_value}")

# Process multiple files
files = ["ep01.mkv", "ep02.mkv", "ep03.mkv"]
batch_result = client.process_batch(
    files,
    algorithms=[HashAlgorithm.ED2K],
    max_concurrent=2,
    continue_on_error=True
)

print(f"Processed {batch_result.successful_files}/{batch_result.total_files} files")

# Async support
import asyncio

async def process_async():
    async with anidb_client.AsyncClient() as client:
        result = await client.process_file_async("video.mkv")
        print(f"ED2K: {result.hashes[HashAlgorithm.ED2K]}")

asyncio.run(process_async())
```

### Error Handling

```python
from anidb_client import AniDBError, FileNotFoundError, ProcessingError

try:
    result = client.process_file("nonexistent.mkv")
except FileNotFoundError as e:
    print(f"File not found: {e}")
except ProcessingError as e:
    print(f"Processing failed: {e}")
except AniDBError as e:
    print(f"General error: {e}")
```

## Node.js Integration

### Installation

```bash
cd bindings/nodejs

# Install dependencies and build
npm install
npm run build

# Or use the build scripts
./build.sh  # Linux/macOS
./build.ps1 # Windows
```

### Package.json Setup

```json
{
  "dependencies": {
    "anidb-client": "file:./bindings/nodejs"
  }
}
```

### JavaScript Usage

```javascript
const { AniDBClient, HashAlgorithm } = require('anidb-client');

// Create client
const client = new AniDBClient({
    cacheDir: '/home/user/.anidb_cache',
    maxConcurrentFiles: 4
});

// Process file with callbacks
client.processFile('video.mkv', {
    algorithms: [HashAlgorithm.ED2K, HashAlgorithm.CRC32],
    enableProgress: true,
    onProgress: (percentage, bytesProcessed, totalBytes) => {
        console.log(`Progress: ${percentage.toFixed(1)}%`);
    }
}, (err, result) => {
    if (err) {
        console.error('Error:', err.message);
        return;
    }
    
    console.log(`File: ${result.filePath}`);
    console.log(`Size: ${result.fileSize}`);
    result.hashes.forEach(hash => {
        console.log(`${hash.algorithm}: ${hash.value}`);
    });
});

// Promise-based API
client.processFileAsync('video.mkv', {
    algorithms: [HashAlgorithm.ED2K]
})
.then(result => {
    console.log('Success:', result);
})
.catch(err => {
    console.error('Error:', err);
});

// Cleanup when done
client.destroy();
```

### TypeScript Usage

```typescript
import { AniDBClient, HashAlgorithm, ProcessOptions, FileResult } from 'anidb-client';

async function processVideo() {
    const client = new AniDBClient({
        cacheDir: process.env.ANIDB_CACHE_DIR,
        enableDebugLogging: false
    });
    
    try {
        const options: ProcessOptions = {
            algorithms: [HashAlgorithm.ED2K, HashAlgorithm.SHA1],
            enableProgress: true,
            onProgress: (percentage: number) => {
                process.stdout.write(`\rProgress: ${percentage.toFixed(1)}%`);
            }
        };
        
        const result: FileResult = await client.processFileAsync('video.mkv', options);
        console.log('\nProcessing complete:', result);
        
        // Batch processing
        const files = ['ep01.mkv', 'ep02.mkv', 'ep03.mkv'];
        const batchResult = await client.processBatchAsync(files, {
            algorithms: [HashAlgorithm.ED2K],
            maxConcurrent: 2,
            continueOnError: true
        });
        
        console.log(`Batch complete: ${batchResult.successfulFiles}/${batchResult.totalFiles}`);
    } finally {
        client.destroy();
    }
}

processVideo().catch(console.error);
```

### Streaming API

```javascript
const { createReadStream } = require('fs');
const { AniDBStream } = require('anidb-client');

// Create a streaming processor
const stream = new AniDBStream({
    algorithms: [HashAlgorithm.ED2K, HashAlgorithm.CRC32]
});

// Process file stream
createReadStream('large-video.mkv')
    .pipe(stream)
    .on('progress', (percentage) => {
        console.log(`Progress: ${percentage.toFixed(1)}%`);
    })
    .on('hash', (algorithm, value) => {
        console.log(`${algorithm}: ${value}`);
    })
    .on('finish', () => {
        console.log('Streaming complete');
    })
    .on('error', (err) => {
        console.error('Stream error:', err);
    });
```

## C# Integration

### NuGet Package

```xml
<PackageReference Include="AniDBClient" Version="0.1.0-alpha" />
```

### Manual Setup

1. Copy the native library to your output directory
2. Add reference to `AniDBClient.dll`

### Basic Usage

```csharp
using System;
using System.Threading.Tasks;
using AniDB.Client;

class Program
{
    static async Task Main(string[] args)
    {
        // Initialize library (done automatically)
        using var client = new AniDBClient(new ClientConfig
        {
            CacheDir = @"C:\Users\User\.anidb_cache",
            MaxConcurrentFiles = 4,
            ChunkSize = 65536
        });
        
        // Process file synchronously
        var result = client.ProcessFile(@"C:\Videos\anime.mkv", new ProcessOptions
        {
            Algorithms = new[] { HashAlgorithm.ED2K, HashAlgorithm.CRC32 },
            EnableProgress = true,
            ProgressCallback = (percentage, bytes, total) =>
            {
                Console.Write($"\rProgress: {percentage:F1}%");
            }
        });
        
        Console.WriteLine($"\nFile: {result.FilePath}");
        Console.WriteLine($"Size: {result.FileSize:N0} bytes");
        foreach (var hash in result.Hashes)
        {
            Console.WriteLine($"{hash.Algorithm}: {hash.Value}");
        }
        
        // Process file asynchronously
        var asyncResult = await client.ProcessFileAsync(@"C:\Videos\anime2.mkv", 
            new ProcessOptions { Algorithms = new[] { HashAlgorithm.ED2K } });
        
        Console.WriteLine($"ED2K: {asyncResult.Hashes[0].Value}");
        
        // Batch processing
        var files = new[] { "ep01.mkv", "ep02.mkv", "ep03.mkv" };
        var batchResult = await client.ProcessBatchAsync(files, new BatchOptions
        {
            Algorithms = new[] { HashAlgorithm.ED2K },
            MaxConcurrent = 2,
            ContinueOnError = true,
            ProgressCallback = (percentage, current, total) =>
            {
                Console.WriteLine($"Batch progress: {current}/{total} files ({percentage:F1}%)");
            }
        });
        
        Console.WriteLine($"Processed {batchResult.SuccessfulFiles}/{batchResult.TotalFiles} files");
    }
}
```

### Event-Based API

```csharp
using AniDB.Client.Events;

// Subscribe to events
client.FileStarted += (sender, e) =>
{
    Console.WriteLine($"Started processing: {e.FilePath}");
};

client.HashCompleted += (sender, e) =>
{
    Console.WriteLine($"{e.Algorithm} hash calculated: {e.HashValue}");
};

client.FileCompleted += (sender, e) =>
{
    Console.WriteLine($"Completed: {e.FilePath} in {e.ProcessingTime.TotalSeconds:F2}s");
};

client.Error += (sender, e) =>
{
    Console.WriteLine($"Error: {e.Message} (File: {e.FilePath})");
};

// Process with events
await client.ProcessFileAsync("video.mkv");
```

### Memory Management

```csharp
// Check memory usage
var stats = AniDBClient.GetMemoryStatistics();
Console.WriteLine($"Memory used: {stats.TotalMemoryUsed:N0} bytes");
Console.WriteLine($"Memory pressure: {stats.MemoryPressure}");

// Force garbage collection
AniDBClient.CollectGarbage();

// Check for leaks (debug only)
#if DEBUG
var (leakCount, leakBytes) = AniDBClient.CheckMemoryLeaks();
if (leakCount > 0)
{
    Console.WriteLine($"Warning: {leakCount} memory leaks detected ({leakBytes:N0} bytes)");
}
#endif
```

## Swift Integration

### Swift Package Manager

Add to `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/yourrepo/anidb-swift.git", from: "0.1.0-alpha")
]
```

### Manual Integration

1. Add the Swift package from `bindings/swift`
2. Ensure the native library is in your app bundle

### Usage Example

```swift
import AniDBClient

// Create client
let client = try AniDBClient(config: ClientConfig(
    cacheDir: "~/.anidb_cache",
    maxConcurrentFiles: 4
))

// Process file
do {
    let result = try client.processFile(
        path: "/path/to/video.mkv",
        options: ProcessOptions(
            algorithms: [.ed2k, .crc32],
            enableProgress: true,
            progressHandler: { percentage, bytesProcessed, totalBytes in
                print("Progress: \(String(format: "%.1f", percentage))%")
            }
        )
    )
    
    print("File: \(result.filePath)")
    print("Size: \(result.fileSize) bytes")
    for hash in result.hashes {
        print("\(hash.algorithm): \(hash.value)")
    }
} catch {
    print("Error: \(error)")
}

// Async/await support
Task {
    do {
        let result = try await client.processFileAsync(
            path: "/path/to/video.mkv",
            algorithms: [.ed2k]
        )
        print("ED2K: \(result.ed2kHash ?? "N/A")")
    } catch {
        print("Async error: \(error)")
    }
}

// Batch processing with Combine
import Combine

let files = ["ep01.mkv", "ep02.mkv", "ep03.mkv"]
client.processBatchPublisher(
    files: files,
    options: BatchOptions(algorithms: [.ed2k])
)
.sink(
    receiveCompletion: { completion in
        if case .failure(let error) = completion {
            print("Batch failed: \(error)")
        }
    },
    receiveValue: { result in
        print("Processed \(result.successfulFiles)/\(result.totalFiles) files")
    }
)
.store(in: &cancellables)
```

### SwiftUI Integration

```swift
import SwiftUI
import AniDBClient

struct FileHashView: View {
    @StateObject private var viewModel = FileHashViewModel()
    
    var body: some View {
        VStack {
            if viewModel.isProcessing {
                ProgressView(value: viewModel.progress)
                    .progressViewStyle(LinearProgressViewStyle())
                Text("Processing: \(viewModel.progress * 100, specifier: "%.1f")%")
            } else if let result = viewModel.result {
                VStack(alignment: .leading) {
                    Text("File: \(result.filePath)")
                    Text("Size: \(result.fileSize) bytes")
                    ForEach(result.hashes, id: \.algorithm) { hash in
                        Text("\(hash.algorithm): \(hash.value)")
                            .font(.system(.body, design: .monospaced))
                    }
                }
            }
            
            Button("Select File") {
                viewModel.selectAndProcessFile()
            }
            .disabled(viewModel.isProcessing)
        }
        .padding()
    }
}

class FileHashViewModel: ObservableObject {
    @Published var isProcessing = false
    @Published var progress: Double = 0
    @Published var result: FileResult?
    
    private let client = try! AniDBClient()
    
    func selectAndProcessFile() {
        // File selection logic here
        let path = "selected_file.mkv"
        
        isProcessing = true
        progress = 0
        
        Task {
            do {
                result = try await client.processFileAsync(
                    path: path,
                    options: ProcessOptions(
                        algorithms: [.ed2k, .crc32],
                        progressHandler: { [weak self] percentage, _, _ in
                            DispatchQueue.main.async {
                                self?.progress = Double(percentage) / 100.0
                            }
                        }
                    )
                )
            } catch {
                print("Error: \(error)")
            }
            
            await MainActor.run {
                isProcessing = false
            }
        }
    }
}
```

## Build Instructions

### From Source

```bash
# Clone repository
git clone https://github.com/yourrepo/anidb-client.git
cd anidb-client/anidb_client_core

# Build release version
cargo build --release

# Run tests
cargo test

# Build with specific features
cargo build --release --features "full"

# Cross-compilation examples
# For Windows from Linux/macOS
cargo build --release --target x86_64-pc-windows-gnu

# For Linux from macOS
cargo build --release --target x86_64-unknown-linux-gnu

# For macOS from Linux
cargo build --release --target x86_64-apple-darwin
```

### Platform-Specific Build Scripts

#### Linux/macOS Build Script

```bash
#!/bin/bash
# build.sh

set -e

echo "Building AniDB Client Library..."

# Build the Rust library
cargo build --release

# Copy library and header
mkdir -p dist/lib dist/include
cp target/release/libanidb_client_core.* dist/lib/
cp include/anidb.h dist/include/

# Generate pkg-config file
cat > dist/lib/pkgconfig/anidb_client.pc << EOF
prefix=/usr/local
exec_prefix=\${prefix}
libdir=\${exec_prefix}/lib
includedir=\${prefix}/include

Name: AniDB Client
Description: AniDB Client Core Library
Version: 0.1.0-alpha
Libs: -L\${libdir} -lanidb_client_core
Cflags: -I\${includedir}
EOF

echo "Build complete! Library in dist/"
```

#### Windows Build Script

```powershell
# build.ps1

Write-Host "Building AniDB Client Library..." -ForegroundColor Green

# Build the Rust library
cargo build --release

# Create distribution directory
New-Item -ItemType Directory -Force -Path dist\lib, dist\include

# Copy files
Copy-Item target\release\anidb_client_core.dll dist\lib\
Copy-Item target\release\anidb_client_core.dll.lib dist\lib\
Copy-Item include\anidb.h dist\include\

Write-Host "Build complete! Library in dist\" -ForegroundColor Green
```

## Platform-Specific Notes

### Linux

- Requires `glibc` 2.17 or later
- Dependencies: `libpthread`, `libdl`, `libm`
- Use `LD_LIBRARY_PATH` or install to system paths

```bash
# Install to system
sudo cp libanidb_client_core.so /usr/local/lib/
sudo ldconfig

# Or use LD_LIBRARY_PATH
export LD_LIBRARY_PATH=/path/to/lib:$LD_LIBRARY_PATH
```

### macOS

- Requires macOS 10.12 or later
- May need to sign the library for distribution
- Handle quarantine attributes

```bash
# Remove quarantine
xattr -d com.apple.quarantine libanidb_client_core.dylib

# Sign the library
codesign --sign "Developer ID" libanidb_client_core.dylib

# Install name adjustment
install_name_tool -id @rpath/libanidb_client_core.dylib libanidb_client_core.dylib
```

### Windows

- Requires Visual C++ Redistributables 2019 or later
- Dependencies: `ws2_32.dll`, `bcrypt.dll`, `ntdll.dll`
- Place DLL in application directory or PATH

```powershell
# Check dependencies
dumpbin /dependents anidb_client_core.dll

# Register in GAC (optional)
gacutil /i anidb_client_core.dll
```

### iOS

- Build with `cargo-lipo` for universal binary
- Static linking recommended

```bash
# Install cargo-lipo
cargo install cargo-lipo

# Build for iOS
cargo lipo --release --targets aarch64-apple-ios,x86_64-apple-ios

# Create xcframework
xcodebuild -create-xcframework \
    -library target/universal/release/libanidb_client_core.a \
    -headers include/anidb.h \
    -output AniDBClient.xcframework
```

### Android

- Use Android NDK for cross-compilation
- Build for multiple architectures

```bash
# Set up NDK environment
export NDK_HOME=/path/to/android-ndk
export PATH=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH

# Build for different architectures
cargo build --target aarch64-linux-android --release
cargo build --target armv7-linux-androideabi --release
cargo build --target i686-linux-android --release
cargo build --target x86_64-linux-android --release

# Create AAR package
# See bindings/android for complete example
```

## Deployment Guide

### Directory Structure

```
your-app/
├── bin/
│   └── your-app
├── lib/
│   ├── libanidb_client_core.so    # Linux
│   ├── libanidb_client_core.dylib # macOS
│   └── anidb_client_core.dll      # Windows
└── data/
    └── .anidb_cache/
```

### Docker Deployment

```dockerfile
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy application
COPY --from=builder /app/target/release/your-app /usr/local/bin/
COPY --from=builder /app/target/release/libanidb_client_core.so /usr/local/lib/

# Update library cache
RUN ldconfig

# Run application
CMD ["your-app"]
```

### Snap Package

```yaml
# snapcraft.yaml
name: your-app
version: '0.1.0-alpha'
summary: Your application using AniDB Client
description: |
  Detailed description of your application

grade: stable
confinement: strict

parts:
  anidb-client:
    plugin: rust
    source: .
    build-packages:
      - cargo
      - rustc
    stage-packages:
      - libssl1.1

apps:
  your-app:
    command: bin/your-app
    plugs:
      - network
      - home
```

### macOS App Bundle

```bash
# Create app bundle structure
mkdir -p YourApp.app/Contents/{MacOS,Frameworks,Resources}

# Copy files
cp your-app YourApp.app/Contents/MacOS/
cp libanidb_client_core.dylib YourApp.app/Contents/Frameworks/
cp Info.plist YourApp.app/Contents/

# Adjust library paths
install_name_tool -change libanidb_client_core.dylib \
    @executable_path/../Frameworks/libanidb_client_core.dylib \
    YourApp.app/Contents/MacOS/your-app

# Sign the app
codesign --deep --sign "Developer ID" YourApp.app

# Create DMG
hdiutil create -volname YourApp -srcfolder YourApp.app -ov YourApp.dmg
```

### Windows Installer

```nsis
; installer.nsi
!include "MUI2.nsh"

Name "Your App"
OutFile "YourApp-Setup.exe"
InstallDir "$PROGRAMFILES\YourApp"

Section "Main"
    SetOutPath "$INSTDIR"
    File "your-app.exe"
    File "anidb_client_core.dll"
    
    ; Visual C++ Redistributables
    File "vcredist_x64.exe"
    ExecWait '"$INSTDIR\vcredist_x64.exe" /quiet'
    
    ; Create shortcuts
    CreateDirectory "$SMPROGRAMS\YourApp"
    CreateShortcut "$SMPROGRAMS\YourApp\YourApp.lnk" "$INSTDIR\your-app.exe"
SectionEnd
```

### Package Managers

#### Homebrew (macOS/Linux)

```ruby
# Formula/your-app.rb
class YourApp < Formula
  desc "Your application using AniDB Client"
  homepage "https://github.com/yourrepo/your-app"
  url "https://github.com/yourrepo/your-app/archive/v0.1.0-alpha.tar.gz"
  sha256 "..."
  
  depends_on "rust" => :build
  
  def install
    system "cargo", "build", "--release"
    bin.install "target/release/your-app"
    lib.install "target/release/libanidb_client_core.dylib"
  end
end
```

#### APT Repository (Debian/Ubuntu)

```bash
# Create .deb package
mkdir -p debian/usr/bin debian/usr/lib
cp your-app debian/usr/bin/
cp libanidb_client_core.so debian/usr/lib/

# Create control file
cat > debian/DEBIAN/control << EOF
Package: your-app
Version: 0.1.0-alpha
Architecture: amd64
Maintainer: Your Name <email@example.com>
Description: Your application using AniDB Client
Depends: libc6 (>= 2.17)
EOF

# Build package
dpkg-deb --build debian your-app_0.1.0-alpha_amd64.deb
```

### Performance Optimization

1. **Library Loading**: Place the library in optimal locations
   - Same directory as executable (fastest)
   - System library paths
   - Use static linking for best performance

2. **Memory Settings**: Configure based on use case
   - Desktop: Higher memory limits (500MB+)
   - Server: Moderate limits (200-500MB)
   - Embedded: Lower limits (<100MB)

3. **Concurrent Operations**: Tune based on system
   - Desktop: 4-8 concurrent files
   - Server: 8-16 concurrent files
   - Limited systems: 1-2 concurrent files

### Troubleshooting

#### Library Not Found

```bash
# Linux
ldd your-app
export LD_LIBRARY_PATH=/path/to/lib:$LD_LIBRARY_PATH

# macOS
otool -L your-app
export DYLD_LIBRARY_PATH=/path/to/lib:$DYLD_LIBRARY_PATH

# Windows
where anidb_client_core.dll
set PATH=C:\path\to\lib;%PATH%
```

#### Version Mismatch

Always use matching header and library versions:

```c
if (anidb_get_abi_version() != ANIDB_ABI_VERSION) {
    fprintf(stderr, "ABI version mismatch!\n");
    return 1;
}
```

#### Debug Symbols

Build with debug symbols for troubleshooting:

```bash
# Debug build
cargo build
export RUST_BACKTRACE=1

# Generate debug symbols separately
cargo build --release
objcopy --only-keep-debug target/release/libanidb_client_core.so libanidb_client_core.debug
strip --strip-debug target/release/libanidb_client_core.so
objcopy --add-gnu-debuglink=libanidb_client_core.debug target/release/libanidb_client_core.so
```