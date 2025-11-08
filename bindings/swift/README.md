# AniDB Client Swift Bindings

Native Swift bindings for the AniDB Client Core library, providing a modern Swift interface with async/await support, type safety, and seamless macOS integration.

## Features

- **Native Swift API**: Designed specifically for Swift developers with idiomatic patterns
- **Async/Await Support**: Modern concurrency with structured concurrency support
- **Type Safety**: Strong typing with Swift enums and structs
- **Automatic Memory Management**: No manual memory management required
- **Progress Tracking**: Real-time progress updates with closures
- **Event Streaming**: AsyncStream-based event monitoring
- **Objective-C Compatibility**: Full bridging support for Objective-C projects
- **Error Handling**: Swift-native error handling with detailed error types

## Requirements

- macOS 13.0+
- Swift 5.9+
- AniDB Client Core library installed

## Installation

### Swift Package Manager

Add the following to your `Package.swift`:

```swift
dependencies: [
    .package(path: "../path/to/anidb-client/bindings/swift")
]
```

### Building from Source

```bash
cd bindings/swift
swift build
```

## Quick Start

```swift
import AniDBClient

// Create client with default configuration
let client = try AniDBClient()

// Process a file
let result = try await client.processFile(
    at: URL(fileURLWithPath: "/path/to/video.mkv"),
    algorithms: [.ed2k, .md5],
    progress: { progress in
        print("Progress: \(progress.percentage)%")
    }
)

// Access results
print("ED2K: \(result.hashes[.ed2k] ?? "N/A")")
print("Processing time: \(result.processingTime)s")
```

## API Overview

### Client Configuration

```swift
let config = AniDBClient.Configuration(
    cacheDirectory: URL(fileURLWithPath: "/custom/cache"),
    maxConcurrentFiles: 8,
    chunkSize: 128 * 1024,
    maxMemoryUsage: 1_000_000_000, // 1GB
    enableDebugLogging: true,
    username: "your_username",
    password: "your_password"
)

let client = try AniDBClient(configuration: config)
```

### File Processing

#### Single File

```swift
let result = try await client.processFile(
    at: fileURL,
    algorithms: [.ed2k, .crc32, .md5, .sha1, .tth],
    progress: { progress in
        print("\(progress.bytesProcessed) / \(progress.totalBytes)")
    }
)
```

#### Batch Processing

```swift
let results = try await client.processBatch(
    urls: [file1, file2, file3],
    algorithms: [.ed2k],
    maxConcurrent: 4,
    continueOnError: true,
    progress: { batchProgress in
        print("\(batchProgress.completedFiles) / \(batchProgress.totalFiles)")
    }
)
```

### Direct Hash Calculation

```swift
let data = "Hello, World!".data(using: .utf8)!
let hash = try client.calculateHash(for: data, algorithm: .md5)
```

### Cache Management

```swift
// Check cache statistics
let stats = try client.cacheStatistics()
print("Cache size: \(stats.formattedSize)")

// Check if file is cached
let isCached = try client.isCached(url: fileURL, algorithm: .ed2k)

// Clear cache
try client.clearCache()
```

### Event Monitoring

```swift
Task {
    for await event in client.events.stream {
        switch event {
        case .fileStart(let url, let size):
            print("Started processing: \(url.lastPathComponent)")
            
        case .hashComplete(let algorithm, let hash):
            print("\(algorithm.name): \(hash)")
            
        case .cacheHit(let url, let algorithm):
            print("Cache hit for \(url.lastPathComponent)")
            
        default:
            break
        }
    }
}
```

### Anime Identification

```swift
let animeInfo = try await client.identifyAnime(
    ed2kHash: "abc123...",
    fileSize: 1234567890
)

print("Anime: \(animeInfo.title)")
print("Episode: \(animeInfo.episodeNumber)")
```

## Objective-C Compatibility

The library includes full Objective-C bridging support:

```objc
@import AniDBClient;

ANDClient *client = [[ANDClient alloc] init];

[client processFileAt:@"/path/to/file.mkv"
           algorithms:@[@(ANDHashAlgorithmED2K)]
      progressHandler:^(ANDProgress *progress) {
          NSLog(@"Progress: %.0f%%", progress.percentage);
      }
           completion:^(ANDFileResult *result, NSError *error) {
               if (error) {
                   NSLog(@"Error: %@", error);
               } else {
                   NSLog(@"ED2K: %@", result.hashes[@"ED2K"]);
               }
           }];
```

## Error Handling

The library provides detailed error types with recovery suggestions:

```swift
do {
    let result = try await client.processFile(at: url)
} catch AniDBError.fileNotFound(let path) {
    print("File not found: \(path)")
} catch AniDBError.outOfMemory {
    print("Insufficient memory available")
} catch {
    print("Unexpected error: \(error)")
}
```

## Performance Considerations

- The library uses streaming file processing to handle large files efficiently
- Memory usage is capped at the configured limit (default: 500MB)
- Multiple hash algorithms are calculated in parallel for optimal performance
- Cache is used to avoid recalculating hashes for unchanged files

## Thread Safety

- `AniDBClient` is thread-safe and can be used from multiple threads
- All async methods can be called concurrently
- Progress callbacks are called on arbitrary threads

## Running Tests

```bash
swift test
```

## Example Application

Run the included example:

```bash
swift run anidb-example
```

## License

See the main project LICENSE file for details.