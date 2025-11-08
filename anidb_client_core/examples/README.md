# AniDB Client Examples

This directory contains example programs demonstrating how to use the AniDB Client library in various programming languages.

## C Examples

### Building

```bash
# Build all C examples
make

# Build specific example
make c_basic_example

# Build with debug symbols
make debug

# Clean build artifacts
make clean
```

### Running

```bash
# Run specific example
make run-basic
make run-advanced
make run-error
make run-callback

# Run all examples
make run-all

# Run with custom file
./c_basic_example /path/to/your/video.mkv
```

### Available C Examples

1. **`c_basic_example.c`** - Basic usage demonstration
   - Library initialization
   - Client creation
   - Single file processing
   - Error handling
   - Memory cleanup

2. **`c_advanced_example.c`** - Advanced features
   - Custom configuration
   - Progress callbacks
   - Event system
   - Batch processing
   - Cache management

3. **`c_error_handling.c`** - Error handling patterns
   - Comprehensive error checking
   - Recovery strategies
   - Logging and debugging
   - Memory leak detection

4. **`callback_demo.c`** - Callback system demonstration
   - Progress tracking
   - Event handling
   - Asynchronous notifications

## Rust Examples

The Rust examples demonstrate native library usage:

- **`adaptive_buffer_demo.rs`** - Dynamic buffer management
- **`benchmark_parallel_performance.rs`** - Performance testing
- **`database_example.rs`** - Database integration
- **`sqlite_cache_example.rs`** - Cache system usage
- **`true_parallel_hashing.rs`** - Parallel hash calculations

Run Rust examples with:
```bash
cargo run --example adaptive_buffer_demo
cargo run --example database_example
# etc.
```

## Language Bindings Examples

### Python
See `bindings/python/examples/`:
- `basic_usage.py` - Simple file hashing
- `async_example.py` - Asynchronous processing
- `advanced_usage.py` - Advanced features
- `hash_calculator.py` - CLI tool example

### Node.js
See `bindings/nodejs/examples/`:
- `basic.js` - Basic usage
- `async.js` - Promise-based API
- `batch.js` - Batch processing
- `stream.js` - Streaming API

### C#
See `bindings/csharp/examples/`:
- `ConsoleApp/Program.cs` - Complete console application

### Swift
See `bindings/swift/Sources/AniDBExample/`:
- `main.swift` - Swift usage example

## Common Patterns

### Error Handling
```c
anidb_result_t result = anidb_process_file(client, path, &options, &file_result);
if (result != ANIDB_SUCCESS) {
    fprintf(stderr, "Error: %s\n", anidb_error_string(result));
    // Handle error appropriately
}
```

### Memory Management
```c
// Always free results
anidb_file_result_t* result;
if (anidb_process_file(client, path, &options, &result) == ANIDB_SUCCESS) {
    // Use result...
    anidb_free_file_result(result);  // Don't forget!
}
```

### Progress Tracking
```c
void progress_callback(float percentage, uint64_t bytes_processed, 
                      uint64_t total_bytes, void* user_data) {
    printf("\rProgress: %.1f%%", percentage);
    fflush(stdout);
}
```

## Performance Tips

1. **Chunk Size**: Larger chunks (256KB-1MB) for large files
2. **Concurrency**: Match CPU cores for small files, reduce for large files
3. **Memory**: Monitor usage with `anidb_get_memory_stats()`
4. **Cache**: Use `skip_existing` for large batches

## Troubleshooting

If examples fail to run:

1. **Library not found**: 
   - Linux: `export LD_LIBRARY_PATH=../target/release:$LD_LIBRARY_PATH`
   - macOS: `export DYLD_LIBRARY_PATH=../target/release:$DYLD_LIBRARY_PATH`
   - Windows: Add library directory to PATH

2. **Build errors**: Ensure Rust library is built first:
   ```bash
   cd ..
   cargo build --release
   ```

3. **Missing test files**: Create test files:
   ```bash
   dd if=/dev/zero of=test_file.bin bs=1M count=10
   ```

## Documentation

For complete API documentation, see:
- [API Reference](../docs/ffi_api_reference.md)
- [Integration Guide](../docs/ffi_integration_guide.md)
- [Performance Tuning](../docs/ffi_performance_tuning_guide.md)
- [Troubleshooting](../docs/ffi_troubleshooting_guide.md)