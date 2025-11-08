# AniDB Client for Node.js

High-performance Node.js bindings for the AniDB Client Core Library, providing fast anime file hashing and identification.

## Features

- **High Performance**: Native C++ implementation with N-API for maximum speed
- **Multiple Hash Algorithms**: ED2K, CRC32, MD5, SHA1, and TTH
- **Streaming Support**: Process large files (100GB+) with constant memory usage
- **Async/Promise API**: Modern JavaScript interface with full TypeScript support
- **Batch Processing**: Efficiently process multiple files concurrently
- **Smart Caching**: Automatic caching of hash results
- **Cross-Platform**: Works on Windows, macOS, and Linux
- **Memory Efficient**: Streaming architecture keeps memory usage under 500MB

## Installation

```bash
npm install anidb-client
```

### Requirements

- Node.js 14.0.0 or higher
- Python (for node-gyp compilation)
- C++ build tools:
  - **Windows**: Visual Studio Build Tools or Visual Studio
  - **macOS**: Xcode Command Line Tools
  - **Linux**: GCC/G++ and make

## Quick Start

```javascript
const { AniDBClient } = require('anidb-client');

// Create client
const client = new AniDBClient();

// Process a file
const result = await client.processFile('anime.mkv');
console.log('ED2K Hash:', result.hashes.ed2k);
console.log('File Size:', result.fileSize);

// Clean up
client.destroy();
```

## API Documentation

### Creating a Client

```javascript
const client = new AniDBClient({
  cacheDir: '.anidb_cache',      // Cache directory
  maxConcurrentFiles: 4,          // Max parallel operations
  chunkSize: 65536,               // Chunk size in bytes
  maxMemoryUsage: 500000000,      // Max memory usage
  enableDebugLogging: false,      // Debug logging
  username: 'your_username',      // AniDB username (optional)
  password: 'your_password'       // AniDB password (optional)
});
```

### Processing Files

#### Single File (Async)

```javascript
const result = await client.processFile('file.mkv', {
  algorithms: ['ed2k', 'crc32', 'md5'],  // Hash algorithms
  enableProgress: true,                    // Enable progress events
  verifyExisting: false                    // Verify cached hashes
});

console.log(result);
// {
//   filePath: 'file.mkv',
//   fileSize: 1073741824,
//   status: 2, // COMPLETED
//   hashes: {
//     ed2k: 'a1b2c3d4...',
//     crc32: '12345678',
//     md5: 'e5f6g7h8...'
//   },
//   processingTimeMs: 2341
// }
```

#### Single File (Sync)

```javascript
const result = client.processFileSync('file.mkv');
```

#### Batch Processing

```javascript
const files = ['file1.mkv', 'file2.mkv', 'file3.mkv'];

const result = await client.processBatch(files, {
  algorithms: ['ed2k'],
  maxConcurrent: 3,
  continueOnError: true,
  skipExisting: true
});

console.log(`Processed ${result.successfulFiles}/${result.totalFiles} files`);
```

### Hash Calculation

```javascript
// Calculate hash for a file
const hash = await client.calculateHash('file.mkv', 'ed2k');

// Calculate hash for a buffer
const buffer = Buffer.from('Hello, World!');
const hash = client.calculateHashBuffer(buffer, 'md5');
```

### Streaming API

```javascript
// Create a hash stream
const stream = client.createHashStream(['ed2k', 'crc32']);

// Process file with progress
stream.on('data', (chunk) => {
  if (chunk.type === 'progress') {
    console.log(`Progress: ${chunk.percentage}%`);
  } else if (chunk.type === 'complete') {
    console.log('Hashes:', chunk.result.hashes);
  }
});

await stream.processFile('large-file.mkv');
```

### Cache Management

```javascript
// Check if file is cached
const isCached = client.isCached('file.mkv', 'ed2k');

// Get cache statistics
const stats = client.getCacheStats();
console.log(`Cache entries: ${stats.totalEntries}`);
console.log(`Cache size: ${stats.sizeBytes} bytes`);

// Clear cache
client.clearCache();
```

### Anime Identification

```javascript
// Identify anime by ED2K hash and file size
const info = await client.identifyFile(ed2kHash, fileSize);

if (info) {
  console.log(`Anime: ${info.title}`);
  console.log(`Episode: ${info.episodeNumber}`);
  console.log(`Confidence: ${info.confidence * 100}%`);
}
```

### Event Handling

```javascript
// File events
client.on('file:start', ({ filePath, fileSize }) => {
  console.log(`Starting: ${filePath} (${fileSize} bytes)`);
});

client.on('file:complete', ({ filePath }) => {
  console.log(`Completed: ${filePath}`);
});

// Hash events
client.on('hash:complete', ({ algorithm, hash }) => {
  console.log(`${algorithm}: ${hash}`);
});

// Generic events
client.on('event', (event) => {
  console.log(`Event: ${event.type}`);
});
```

## Hash Algorithms

| Algorithm | Description | Hash Length |
|-----------|-------------|-------------|
| ED2K | eDonkey2000 hash (AniDB standard) | 32 hex chars |
| CRC32 | Cyclic redundancy check | 8 hex chars |
| MD5 | Message Digest 5 | 32 hex chars |
| SHA1 | Secure Hash Algorithm 1 | 40 hex chars |
| TTH | Tiger Tree Hash | 39 base32 chars |

## Examples

See the `examples/` directory for more detailed examples:

- `basic.js` - Basic usage and features
- `async.js` - Async/Promise patterns
- `stream.js` - Streaming API for large files
- `batch.js` - Batch processing multiple files

## Performance

The library is optimized for high performance:

- **Streaming Architecture**: Process files larger than available RAM
- **Multi-threaded Hashing**: Utilize multiple CPU cores
- **Zero-copy Buffers**: Minimal memory allocation
- **Native Implementation**: C++ core for maximum speed

Typical performance on modern hardware:
- ED2K hashing: 200-400 MB/s
- Memory usage: < 100MB for any file size
- Can process 100GB+ files

## Error Handling

```javascript
try {
  const result = await client.processFile('file.mkv');
} catch (error) {
  console.error('Error:', error.message);
  console.error('Code:', error.code);
  
  // Error codes
  // ErrorCode.FILE_NOT_FOUND - File doesn't exist
  // ErrorCode.PERMISSION_DENIED - No read permission
  // ErrorCode.OUT_OF_MEMORY - Memory limit exceeded
  // etc.
}
```

## TypeScript Support

Full TypeScript definitions are included:

```typescript
import { AniDBClient, FileResult, HashAlgorithm } from 'anidb-client';

const client = new AniDBClient();

const result: FileResult = await client.processFile('file.mkv', {
  algorithms: [HashAlgorithm.ED2K, HashAlgorithm.CRC32]
});
```

## Building from Source

```bash
# Clone repository
git clone https://github.com/yourusername/anidb-client.git
cd anidb-client/bindings/nodejs

# Install dependencies
npm install

# Build native module
npm run build

# Run tests
npm test
```

## License

MIT License - see LICENSE file for details.

## Contributing

Contributions are welcome! Please read CONTRIBUTING.md for guidelines.

## Support

- GitHub Issues: https://github.com/yourusername/anidb-client/issues
- Documentation: https://docs.anidb-client.org