# AniDB Client Python Library

Python bindings for the AniDB Client Core Library, providing efficient file hashing and anime identification capabilities.

## Features

- **High-performance file hashing** with support for ED2K, CRC32, MD5, SHA1, and TTH algorithms
- **Streaming file processing** for handling very large files (100GB+) with constant memory usage
- **Context manager support** for automatic resource cleanup
- **Async/await support** for non-blocking operations
- **Type hints** for better IDE support and type checking
- **Progress callbacks** for monitoring long-running operations
- **Event system** for detailed processing insights
- **Cache management** for improved performance

## Installation

### From PyPI

```bash
pip install anidb-client
```

### From Source

```bash
git clone https://github.com/yourusername/anidb-client.git
cd anidb-client/bindings/python
pip install -e .
```

### Requirements

- Python 3.8 or higher
- AniDB Client Core Library (libanidb_client_core)
- ctypes (included with Python)

## Quick Start

### Basic Usage

```python
from anidb_client import AniDBClient

# Process a file with context manager
with AniDBClient() as client:
    result = client.process_file("anime_episode.mkv")
    
    print(f"ED2K: {result.get_hash(HashAlgorithm.ED2K)}")
    print(f"Size: {result.file_size:,} bytes")
    print(f"Time: {result.processing_time_seconds:.2f}s")
```

### Multiple Hash Algorithms

```python
from anidb_client import AniDBClient, HashAlgorithm, ProcessOptions

options = ProcessOptions(
    algorithms=[HashAlgorithm.ED2K, HashAlgorithm.CRC32, HashAlgorithm.MD5]
)

with AniDBClient() as client:
    result = client.process_file("video.mp4", options)
    
    for algo, hash_value in result.hashes.items():
        print(f"{algo.name}: {hash_value}")
```

### Async Operations

```python
import asyncio
from anidb_client import AniDBClient

async def process_files():
    with AniDBClient() as client:
        # Process multiple files concurrently
        tasks = [
            client.process_file_async("file1.mkv"),
            client.process_file_async("file2.mkv"),
            client.process_file_async("file3.mkv"),
        ]
        
        results = await asyncio.gather(*tasks)
        
        for result in results:
            print(f"{result.file_path.name}: {result.get_hash(HashAlgorithm.ED2K)}")

asyncio.run(process_files())
```

### Progress Monitoring

```python
def progress_callback(percentage, bytes_processed, total_bytes):
    bar_width = 40
    filled = int(bar_width * percentage / 100)
    bar = "█" * filled + "░" * (bar_width - filled)
    print(f"\r[{bar}] {percentage:.1f}%", end="", flush=True)

options = ProcessOptions(
    enable_progress=True,
    progress_callback=progress_callback
)

with AniDBClient() as client:
    result = client.process_file("large_file.mkv", options)
    print()  # New line after progress
```

### Event System

```python
from anidb_client import AniDBClient, EventType

def event_handler(event):
    if event.type == EventType.FILE_START:
        print(f"Started: {event.data['file_path']}")
    elif event.type == EventType.HASH_COMPLETE:
        print(f"Completed {event.data['algorithm']} hash")

with AniDBClient() as client:
    client.connect_events(event_handler)
    result = client.process_file("anime.mkv")
```

### Anime Identification

```python
with AniDBClient() as client:
    # First, get the ED2K hash
    result = client.process_file("anime_episode.mkv")
    ed2k_hash = result.get_hash(HashAlgorithm.ED2K)
    
    # Then identify the file
    anime_info = client.identify_file(ed2k_hash, result.file_size)
    
    if anime_info:
        print(f"Title: {anime_info.title}")
        print(f"Episode: {anime_info.episode_number}")
        print(f"Confidence: {anime_info.confidence:.0%}")
```

## Configuration

```python
from pathlib import Path
from anidb_client import AniDBClient, ClientConfig

config = ClientConfig(
    cache_dir=Path.home() / ".anidb_cache",
    max_concurrent_files=4,
    chunk_size=128 * 1024,  # 128KB
    enable_debug_logging=True,
    username="your_username",  # Optional
    password="your_password",  # Optional
)

with AniDBClient(config) as client:
    # Use configured client
    pass
```

## API Reference

### Classes

#### `AniDBClient`
Main client class for interacting with the AniDB library.

**Methods:**
- `process_file(file_path, options)` - Process a single file
- `process_file_async(file_path, options)` - Process file asynchronously
- `process_batch(file_paths, options)` - Process multiple files
- `calculate_hash(file_path, algorithm)` - Calculate single hash
- `identify_file(ed2k_hash, file_size)` - Identify anime by hash
- `clear_cache()` - Clear the hash cache
- `get_cache_stats()` - Get cache statistics

#### `ClientConfig`
Configuration for the AniDB client.

**Fields:**
- `cache_dir` - Directory for cache storage
- `max_concurrent_files` - Maximum concurrent operations
- `chunk_size` - Size of chunks for streaming
- `enable_debug_logging` - Enable debug output
- `username` - AniDB username (optional)
- `password` - AniDB password (optional)

#### `ProcessOptions`
Options for file processing.

**Fields:**
- `algorithms` - List of hash algorithms to calculate
- `enable_progress` - Enable progress reporting
- `verify_existing` - Verify existing cached hashes
- `progress_callback` - Callback for progress updates

### Enums

#### `HashAlgorithm`
- `ED2K` - ED2K hash (AniDB standard)
- `CRC32` - CRC32 checksum
- `MD5` - MD5 hash
- `SHA1` - SHA-1 hash
- `TTH` - Tiger Tree Hash

#### `ProcessingStatus`
- `PENDING` - Not started
- `PROCESSING` - In progress
- `COMPLETED` - Successfully completed
- `FAILED` - Failed with error
- `CANCELLED` - Cancelled by user

## Examples

See the `examples/` directory for complete examples:

- `basic_usage.py` - Simple file processing
- `advanced_usage.py` - Advanced features and event handling
- `async_example.py` - Asynchronous operations
- `hash_calculator.py` - Standalone hash calculator utility

## Error Handling

```python
from anidb_client import AniDBClient, FileNotFoundError, NetworkError

try:
    with AniDBClient() as client:
        result = client.process_file("missing.mkv")
except FileNotFoundError as e:
    print(f"File not found: {e.file_path}")
except NetworkError as e:
    print(f"Network error: {e}")
except Exception as e:
    print(f"Unexpected error: {e}")
```

## Performance Tips

1. **Use context managers** to ensure proper cleanup
2. **Enable caching** to avoid reprocessing files
3. **Use appropriate chunk sizes** - larger chunks for faster processing
4. **Process files in batches** for better throughput
5. **Use async operations** for I/O-bound workloads

## Development

### Running Tests

```bash
# Install development dependencies
pip install -e ".[dev]"

# Run tests
pytest

# Run with coverage
pytest --cov=anidb_client

# Run type checking
mypy src/anidb_client
```

### Code Style

```bash
# Format code
black src tests examples

# Lint code
ruff src tests examples
```

## License

This project is licensed under the MIT License. See the LICENSE file for details.

## Support

For issues and questions:
- GitHub Issues: https://github.com/yourusername/anidb-client/issues
- Documentation: https://anidb-client.readthedocs.io