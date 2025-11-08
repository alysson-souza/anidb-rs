# FFI Integration Tests

This directory contains comprehensive integration tests for the Foreign Function Interface (FFI) bindings of the AniDB client library. These tests ensure that the library can be safely used from other programming languages via C-compatible bindings.

## Test Coverage

### 1. Multi-threaded Access (`ffi_integration_tests.rs`)

#### Thread Safety Tests
- **test_ffi_multi_threaded_access**: Verifies multiple threads can safely create and use separate client instances
- **test_ffi_thread_safety_shared_client**: Tests concurrent operations on a single shared client
- **test_ffi_race_condition_detection**: Aggressive concurrent access to detect potential race conditions

#### Large File Processing
- **test_ffi_large_file_processing**: Tests processing files >1GB while maintaining memory limits
  - Verifies memory usage stays under 500MB
  - Ensures throughput remains >50MB/s
  - Validates progress reporting for large files

#### Error Handling
- **test_ffi_error_conditions**: Comprehensive error scenario testing
  - Invalid file paths
  - Permission denied scenarios
  - Invalid parameters
  - Resource exhaustion simulation

#### Memory Management
- **test_ffi_memory_stress**: Stress tests memory allocation and deallocation
  - Processes 50 files rapidly
  - Monitors memory pressure
  - Triggers garbage collection when needed
  - Verifies no memory leaks (in debug builds)

- **test_ffi_buffer_pool_effectiveness**: Tests buffer pool reuse efficiency
  - Measures hit rate for buffer allocations
  - Verifies >50% hit rate for repeated operations

#### Event System
- **test_ffi_event_system**: Tests event callbacks and notifications
- **test_ffi_callback_management**: Tests callback registration/unregistration

### 2. Batch Processing (`ffi_batch_integration_tests.rs`)

- **test_ffi_batch_processing_basic**: Basic batch file processing
- **test_ffi_batch_error_handling**: Batch processing with mixed valid/invalid files
- **test_ffi_concurrent_batch_processing**: Multiple concurrent batch operations
- **test_ffi_batch_memory_constraints**: Batch processing under memory pressure
- **test_ffi_batch_repeat_processing_without_cache**: Ensures repeated runs are stable now that caching lives outside the core library

### 3. Cross-Platform Compatibility (`ffi_cross_platform_tests.rs`)

#### Path Handling
- **test_ffi_platform_path_handling**: Tests various path formats
  - Unicode filenames (Chinese, Russian, Japanese)
  - Special characters (spaces, dashes, dots)
  - Platform-specific characters

- **test_ffi_long_path_support**: Tests deeply nested directory structures
  - Windows: Up to 30 levels deep
  - Unix: Up to 50 levels deep

#### Platform-Specific Features
- **test_ffi_platform_permissions**: Tests file permission handling
  - Unix: Read-only files, no-permission files
  - Windows: Read-only attribute

- **test_ffi_platform_performance**: Tests platform-specific optimizations
  - Linux: Larger chunk sizes (128KB)
  - Windows/macOS: Standard chunks (64KB)

- **test_ffi_platform_cache_directories**: Tests platform-specific cache paths

## Running the Tests

### Run All FFI Integration Tests
```bash
cargo test --package anidb_client_core --test 'ffi_*' -- --nocapture
```

### Run Specific Test Categories
```bash
# Multi-threaded tests
cargo test --package anidb_client_core --test ffi_integration_tests -- --nocapture

# Batch processing tests
cargo test --package anidb_client_core --test ffi_batch_integration_tests -- --nocapture

# Cross-platform tests
cargo test --package anidb_client_core --test ffi_cross_platform_tests -- --nocapture
```

### Run Large File Tests (Disabled by Default)
```bash
cargo test --package anidb_client_core --test ffi_integration_tests test_ffi_large_file_processing -- --ignored --nocapture
```

## Test Requirements

### Memory Requirements
- Most tests: ~100MB
- Memory stress tests: ~200MB
- Large file tests: ~1.5GB disk space, ~500MB RAM

### Platform Requirements
- **All platforms**: Basic POSIX file operations
- **Unix**: File permission manipulation (chmod)
- **Windows**: Long path support (\\\\?\\ prefix)

## Key Testing Patterns

### Thread Safety
```rust
// Convert raw pointer to usize for thread safety
let client_handle_ptr = client_handle as usize;
let handle = thread::spawn(move || {
    let client_handle = client_handle_ptr as *mut std::ffi::c_void;
    // Use client_handle...
});
```

### Memory Monitoring
```rust
let mut stats = AniDBMemoryStats { /* ... */ };
anidb_get_memory_stats(&mut stats);
if stats.memory_pressure >= 2 {
    anidb_memory_gc(); // Trigger garbage collection
}
```

### Progress Tracking
```rust
extern "C" fn progress_callback(percentage: f32, bytes: u64, total: u64, user_data: *mut c_void) {
    // Track progress...
}
```

## Known Limitations

1. **Platform Differences**:
   - Windows has stricter path length limits
   - Unix file permissions are more granular
   - Performance characteristics vary by OS

2. **Test Environment**:
   - Large file tests require significant disk space
   - Some tests may fail in constrained environments
   - Network tests are not included (offline operation)

## Future Enhancements

1. **Language Binding Tests**:
   - Python FFI integration tests
   - Node.js FFI integration tests
   - C++ wrapper tests

2. **Performance Benchmarks**:
   - Comparative benchmarks across platforms
   - Memory allocation profiling
   - Thread contention analysis

3. **Stress Testing**:
   - 24-hour continuous operation tests
   - Thousands of concurrent files
   - Network failure simulation
