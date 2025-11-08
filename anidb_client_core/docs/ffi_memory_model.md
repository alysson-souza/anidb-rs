# FFI Memory Management Model

This document describes the memory ownership and management model for the AniDB client FFI interface.

## Core Principles

1. **Allocation by library, freed by caller**: The library allocates memory for return values, and the caller is responsible for freeing it.
2. **Explicit ownership transfer**: Functions that return allocated memory clearly document ownership transfer.
3. **Tracked allocations**: All FFI allocations are tracked for leak detection and debugging.
4. **Buffer pooling**: Frequently allocated buffers use a pool to reduce allocation overhead.
5. **Memory limits**: The library enforces a 500MB memory limit across all operations.

## String Management

### Allocation
```c
// Strings returned by the library are allocated and must be freed
const char* error_msg = anidb_client_get_last_error(handle);
// Use the string...
anidb_free_string((char*)error_msg);
```

### UTF-8 Handling
- All strings crossing FFI boundary must be valid UTF-8
- Invalid UTF-8 returns `AniDBResult::ErrorInvalidUtf8`
- Null termination is guaranteed for all returned strings

## Buffer Management

### Buffer Pool
The library uses an internal buffer pool for frequently allocated buffers:
- Hash results
- File results
- Batch results
- Event data

Pool characteristics:
- Size classes: 1KB, 4KB, 16KB, 64KB, 256KB, 1MB
- Maximum 10 buffers per size class
- Automatic shrinking under memory pressure
- Thread-safe allocation and deallocation

### Direct Allocation
Large or infrequent allocations bypass the pool and allocate directly.

## Result Structures

### File Results
```c
AniDBFileResult* result;
anidb_process_file(handle, path, options, &result);

// Use result...

// Free the result and all contained allocations
anidb_free_file_result(result);
```

Ownership details:
- `file_path`: Owned by result, freed by `anidb_free_file_result`
- `error_message`: Owned by result, freed by `anidb_free_file_result`
- `hashes`: Array owned by result, including all hash strings

### Batch Results
```c
AniDBBatchResult* batch;
anidb_process_batch(handle, files, count, options, &batch);

// Use batch...

// Free the batch and all contained results
anidb_free_batch_result(batch);
```

Ownership details:
- `results`: Array of file results, all freed recursively
- Each file result follows the same ownership as individual results

## Memory Tracking

### Allocation Types
The library tracks different allocation types:
- `String`: String allocations
- `HashResult`: Hash result arrays
- `FileResult`: File result structures
- `BatchResult`: Batch result structures
- `Buffer`: Generic buffers
- `Event`: Event data

### Statistics
```c
AniDBMemoryStats stats;
anidb_get_memory_stats(&stats);

printf("Total memory: %llu\n", stats.total_memory_used);
printf("FFI allocated: %llu\n", stats.ffi_allocated);
printf("Pool memory: %llu\n", stats.pool_memory);
printf("Memory pressure: %u\n", stats.memory_pressure);
```

### Leak Detection (Debug Builds)
```c
uint64_t leak_count, leaked_bytes;
anidb_check_memory_leaks(&leak_count, &leaked_bytes);

if (leak_count > 0) {
    printf("Found %llu leaks totaling %llu bytes\n", 
           leak_count, leaked_bytes);
}
```

## Memory Pressure

The library monitors memory pressure and sends events:
- **Low**: < 50% of limit
- **Medium**: 50-75% of limit
- **High**: 75-90% of limit
- **Critical**: > 90% of limit

Critical pressure triggers:
- Memory warning events
- Buffer pool shrinking
- Potential operation failures

## Best Practices

1. **Always free returned memory**: Use the appropriate free function for each type
2. **Check for null**: Always check returned pointers before use
3. **Handle errors early**: Free any partial allocations on error paths
4. **Monitor memory**: Use statistics API to track memory usage
5. **Respond to events**: Handle memory warning events appropriately

## Error Handling

Memory allocation failures return:
- `AniDBResult::ErrorOutOfMemory`: Allocation failed or would exceed limit
- Null pointers for string allocations
- Partial results are cleaned up internally

## Thread Safety

- All allocation/deallocation functions are thread-safe
- The buffer pool supports concurrent access
- Memory tracking is atomic and lock-free where possible

## Example: Complete Memory Management

```c
// Initialize
anidb_init(1);

// Create client
void* handle;
anidb_client_create(&handle);

// Process file
AniDBProcessOptions options = {
    .algorithms = algorithms,
    .algorithm_count = 2,
    .enable_progress = 1,
};

AniDBFileResult* result;
AniDBResult status = anidb_process_file(handle, "/path/to/file", &options, &result);

if (status == AniDBResult::Success) {
    // Use result
    printf("File: %s\n", result->file_path);
    printf("Size: %llu\n", result->file_size);
    
    for (size_t i = 0; i < result->hash_count; i++) {
        printf("%s: %s\n", 
               anidb_hash_algorithm_name(result->hashes[i].algorithm),
               result->hashes[i].hash_value);
    }
    
    // Free result
    anidb_free_file_result(result);
} else {
    // Handle error
    char error_buf[256];
    anidb_client_get_last_error(handle, error_buf, sizeof(error_buf));
    printf("Error: %s\n", error_buf);
}

// Check for leaks before cleanup
uint64_t leaks, leaked;
anidb_check_memory_leaks(&leaks, &leaked);

// Cleanup
anidb_client_destroy(handle);
anidb_cleanup();
```