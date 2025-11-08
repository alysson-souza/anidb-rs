# AniDB Client C# Bindings

High-performance .NET bindings for the AniDB Client Core Library, providing native performance with idiomatic C# APIs.

## Features

- **Native Performance**: Direct P/Invoke to Rust library for maximum speed
- **Memory Safe**: SafeHandle pattern prevents memory leaks
- **Async/Await**: Full Task-based async API support
- **Cross-Platform**: Works on Windows, Linux, and macOS
- **.NET 6.0+**: Modern .NET support with nullable reference types
- **Type Safe**: Strongly-typed enums and data structures
- **Event System**: Real-time processing events via C# events

## Installation

### NuGet Package

```bash
dotnet add package AniDBClient
```

### Manual Build

```bash
# Build the C# library
dotnet build src/AniDBClient/AniDBClient.csproj -c Release

# Run tests
dotnet test src/AniDBClient.Tests/AniDBClient.Tests.csproj

# Build NuGet package
dotnet pack src/AniDBClient/AniDBClient.csproj -c Release
```

## Quick Start

```csharp
using AniDBClient;

// Create client with default configuration
using var client = new AniDBClient();

// Process a file
var result = await client.ProcessFileAsync("video.mkv", new ProcessingOptions
{
    Algorithms = new[] { HashAlgorithm.ED2K, HashAlgorithm.CRC32 },
    EnableProgress = true,
    ProgressCallback = info => Console.WriteLine($"Progress: {info.Percentage:F1}%")
});

Console.WriteLine($"ED2K: {result.Hashes.First(h => h.Algorithm == HashAlgorithm.ED2K).Value}");
```

## API Overview

### Client Configuration

```csharp
var config = new ClientConfiguration
{
    CacheDirectory = @"C:\AniDB\Cache",
    MaxConcurrentFiles = 8,
    ChunkSize = 128 * 1024,
    MaxMemoryUsage = 500 * 1024 * 1024,
    EnableDebugLogging = true,
    Username = "myusername",
    Password = "mypassword"
};

using var client = new AniDBClient(config);
```

### File Processing

#### Synchronous Processing

```csharp
var result = client.ProcessFile("file.mkv", new ProcessingOptions
{
    Algorithms = new[] { HashAlgorithm.ED2K },
    VerifyExisting = true
});
```

#### Asynchronous Processing

```csharp
var result = await client.ProcessFileAsync("file.mkv", cancellationToken: cts.Token);
```

### Batch Processing

```csharp
var files = Directory.GetFiles(@"C:\Anime", "*.mkv", SearchOption.AllDirectories);

var batchResult = await client.ProcessBatchAsync(files, new BatchOptions
{
    Algorithms = new[] { HashAlgorithm.ED2K },
    MaxConcurrent = 4,
    ContinueOnError = true,
    SkipExisting = true,
    ProgressCallback = info => 
    {
        Console.WriteLine($"Processing {info.CurrentFile}");
        Console.WriteLine($"Files: {info.FilesCompleted}/{info.TotalFiles}");
    }
});

Console.WriteLine($"Processed {batchResult.SuccessfulFiles} files successfully");
```

### Hash Calculation

```csharp
// Hash a file
string ed2kHash = client.CalculateHash("file.mkv", HashAlgorithm.ED2K);

// Hash memory buffer
byte[] data = File.ReadAllBytes("small_file.txt");
string md5Hash = client.CalculateHash(data, HashAlgorithm.MD5);
```

### Anime Identification

```csharp
var animeInfo = await client.IdentifyFileAsync(ed2kHash, fileSize);

if (animeInfo != null)
{
    Console.WriteLine($"Anime: {animeInfo.Title}");
    Console.WriteLine($"Episode: {animeInfo.EpisodeNumber}");
    Console.WriteLine($"Confidence: {animeInfo.Confidence:P0}");
}
```

### Event Handling

```csharp
client.EventReceived += (sender, evt) =>
{
    switch (evt.Type)
    {
        case EventType.FileStart:
            Console.WriteLine($"Started processing: {evt.FilePath}");
            break;
        
        case EventType.HashComplete:
            Console.WriteLine($"{evt.Algorithm}: {evt.HashValue}");
            break;
        
        case EventType.MemoryWarning:
            var (current, max) = evt.MemoryUsage!.Value;
            Console.WriteLine($"Memory usage: {current}/{max} bytes");
            break;
    }
};
```

### Cache Management

```csharp
// Check if file is cached
bool isCached = client.IsFileCached("file.mkv", HashAlgorithm.ED2K);

// Get cache statistics
var stats = client.GetCacheStatistics();
Console.WriteLine($"Cache entries: {stats.TotalEntries}");
Console.WriteLine($"Cache size: {stats.SizeInBytes} bytes");

// Clear cache
client.ClearCache();
```

## Error Handling

The library throws specific exceptions for different error conditions:

```csharp
try
{
    var result = await client.ProcessFileAsync("file.mkv");
}
catch (FileNotFoundException ex)
{
    Console.WriteLine($"File not found: {ex.FilePath}");
}
catch (NetworkException ex)
{
    Console.WriteLine("Network error - AniDB may be offline");
}
catch (OperationCancelledException)
{
    Console.WriteLine("Operation was cancelled");
}
catch (AniDBException ex)
{
    Console.WriteLine($"AniDB error ({ex.ErrorCode}): {ex.Message}");
}
```

## Thread Safety

- `AniDBClient` instances are thread-safe for all operations
- Progress callbacks are invoked on background threads
- Event handlers are invoked on background threads
- Use `ConfigureAwait(false)` in library/non-UI code

## Performance Tips

1. **Enable Caching**: Dramatically speeds up repeated processing
2. **Use Batch Processing**: More efficient than individual files
3. **Appropriate Chunk Size**: Larger chunks (64KB-256KB) for better throughput
4. **Limit Concurrent Operations**: 4-8 concurrent files is usually optimal
5. **Use Async Methods**: Better resource utilization

## Platform-Specific Notes

### Windows
- Native library: `anidb_client_core.dll`
- Requires Visual C++ Runtime 2019+

### Linux
- Native library: `libanidb_client_core.so`
- May require `libssl` for network operations

### macOS
- Native library: `libanidb_client_core.dylib`
- Universal binary supports both x64 and ARM64

## Example Application

See the `examples/ConsoleApp` directory for a complete example application demonstrating all features.

```bash
cd examples/ConsoleApp
dotnet run -- process "video.mkv" ED2K CRC32 MD5
```

## Troubleshooting

### DllNotFoundException
- Ensure native libraries are in the correct runtime folder
- Check platform-specific dependencies
- Verify architecture matches (x86/x64/ARM64)

### Version Mismatch
- Update both C# bindings and native library together
- Check ABI version compatibility

### Memory Issues
- Configure `MaxMemoryUsage` appropriately
- Monitor memory events
- Use streaming for large files

## License

MIT License - See LICENSE file for details