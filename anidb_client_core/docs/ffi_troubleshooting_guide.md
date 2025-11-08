# AniDB Client FFI Troubleshooting Guide

## Table of Contents

1. [Common Issues](#common-issues)
2. [Library Loading Problems](#library-loading-problems)
3. [Memory Issues](#memory-issues)
4. [Performance Problems](#performance-problems)
5. [API Usage Errors](#api-usage-errors)
6. [Platform-Specific Issues](#platform-specific-issues)
7. [Debugging Techniques](#debugging-techniques)
8. [Error Codes Reference](#error-codes-reference)
9. [FAQ](#faq)

## Common Issues

### Issue: "Library not found" or "Cannot load library"

**Symptoms:**
- Application fails to start
- Error messages about missing libraries
- Dynamic linker errors

**Solutions:**

1. **Check library path:**
   ```bash
   # Linux
   ldd your_application
   export LD_LIBRARY_PATH=/path/to/lib:$LD_LIBRARY_PATH
   
   # macOS
   otool -L your_application
   export DYLD_LIBRARY_PATH=/path/to/lib:$DYLD_LIBRARY_PATH
   
   # Windows
   echo %PATH%
   set PATH=C:\path\to\lib;%PATH%
   ```

2. **Verify library architecture:**
   ```bash
   # Linux/macOS
   file libanidb_client_core.so
   file your_application
   
   # Windows (Visual Studio Command Prompt)
   dumpbin /headers anidb_client_core.dll
   ```

3. **Install runtime dependencies:**
   ```bash
   # Linux (Debian/Ubuntu)
   sudo apt-get install libc6 libgcc1 libstdc++6
   
   # macOS
   # Usually included with system
   
   # Windows
   # Install Visual C++ Redistributables 2019
   ```

### Issue: "Version mismatch" error

**Symptoms:**
- `ANIDB_ERROR_VERSION_MISMATCH` returned from `anidb_init`
- Application refuses to initialize

**Solutions:**

1. **Check ABI version:**
   ```c
   printf("Header ABI version: %d\n", ANIDB_ABI_VERSION);
   printf("Library ABI version: %d\n", anidb_get_abi_version());
   
   if (ANIDB_ABI_VERSION != anidb_get_abi_version()) {
       fprintf(stderr, "Version mismatch! Please update library or headers.\n");
   }
   ```

2. **Update library and headers together:**
   ```bash
   # Always update both the library and header file
   cp new_version/libanidb_client_core.so /usr/local/lib/
   cp new_version/anidb.h /usr/local/include/
   ```

### Issue: "Invalid handle" errors

**Symptoms:**
- `ANIDB_ERROR_INVALID_HANDLE` returned from API calls
- Crashes when using handles

**Solutions:**

1. **Initialize handle pointers:**
   ```c
   anidb_client_handle_t client = NULL;  // Always initialize
   anidb_result_t result = anidb_client_create(&client);
   
   if (result != ANIDB_SUCCESS || client == NULL) {
       // Handle creation failed
   }
   ```

2. **Check handle validity:**
   ```c
   // Wrapper function to validate handles
   int is_valid_handle(anidb_client_handle_t handle) {
       if (handle == NULL) return 0;
       
       // Try a simple operation
       char buffer[1];
       return anidb_client_get_last_error(handle, buffer, 1) != ANIDB_ERROR_INVALID_HANDLE;
   }
   ```

3. **Avoid use-after-free:**
   ```c
   anidb_client_destroy(client);
   client = NULL;  // Always NULL after destroy
   
   // Later...
   if (client != NULL) {  // Check before use
       // Safe to use
   }
   ```

## Library Loading Problems

### Linux-Specific Loading Issues

1. **Missing GLIBC version:**
   ```bash
   # Check required GLIBC version
   objdump -T libanidb_client_core.so | grep GLIBC
   
   # Check system GLIBC version
   ldd --version
   
   # Solution: Update system or compile for older GLIBC
   ```

2. **SELinux blocking library:**
   ```bash
   # Check SELinux denials
   sudo ausearch -m avc -ts recent
   
   # Temporarily disable (for testing only)
   sudo setenforce 0
   
   # Proper solution: Set correct context
   sudo chcon -t textrel_shlib_t libanidb_client_core.so
   ```

### Windows-Specific Loading Issues

1. **Missing dependencies:**
   ```powershell
   # Use Dependency Walker or Dependencies tool
   dependencies.exe anidb_client_core.dll
   
   # Common missing DLLs:
   # - VCRUNTIME140.dll
   # - MSVCP140.dll
   # - api-ms-win-crt-*.dll
   ```

2. **32-bit vs 64-bit mismatch:**
   ```c
   #ifdef _WIN64
       #define LIBRARY_NAME "anidb_client_core_x64.dll"
   #else
       #define LIBRARY_NAME "anidb_client_core_x86.dll"
   #endif
   ```

### macOS-Specific Loading Issues

1. **Quarantine attribute:**
   ```bash
   # Check for quarantine
   xattr -l libanidb_client_core.dylib
   
   # Remove quarantine
   xattr -d com.apple.quarantine libanidb_client_core.dylib
   ```

2. **Code signing issues:**
   ```bash
   # Check signature
   codesign -dv libanidb_client_core.dylib
   
   # Sign library (requires developer certificate)
   codesign --force --sign "Developer ID" libanidb_client_core.dylib
   ```

## Memory Issues

### Issue: Out of Memory Errors

**Symptoms:**
- `ANIDB_ERROR_OUT_OF_MEMORY` errors
- Application crashes
- System becomes unresponsive

**Diagnostic Steps:**

1. **Monitor memory usage:**
   ```c
   void monitor_memory() {
       anidb_memory_stats_t stats;
       if (anidb_get_memory_stats(&stats) == ANIDB_SUCCESS) {
           printf("Memory used: %llu MB (limit: %llu MB)\n",
                  stats.total_memory_used / 1048576,
                  stats.memory_limit / 1048576);
           printf("Pressure: %s\n",
                  stats.memory_pressure == 0 ? "Low" :
                  stats.memory_pressure == 1 ? "Medium" :
                  stats.memory_pressure == 2 ? "High" : "Critical");
       }
   }
   ```

2. **Reduce memory usage:**
   ```c
   // Reduce concurrent operations
   anidb_config_t low_memory_config = {
       .max_concurrent_files = 1,
       .chunk_size = 16384,  // 16KB
       .max_memory_usage = 50 * 1024 * 1024  // 50MB
   };
   
   // Force garbage collection
   anidb_memory_gc();
   
   // Clear cache if necessary
   anidb_cache_clear(client);
   ```

### Issue: Memory Leaks

**Symptoms:**
- Gradual memory increase
- Eventually runs out of memory
- Performance degradation over time

**Detection:**

1. **Use built-in leak detection:**
   ```c
   #ifdef DEBUG
   uint64_t leaks, bytes;
   anidb_check_memory_leaks(&leaks, &bytes);
   if (leaks > 0) {
       printf("LEAK: %llu allocations (%llu bytes)\n", leaks, bytes);
   }
   #endif
   ```

2. **Common leak patterns:**
   ```c
   // WRONG: Forgetting to free results
   anidb_file_result_t* result;
   anidb_process_file(client, "file.mkv", &options, &result);
   // Missing: anidb_free_file_result(result);
   
   // CORRECT: Always free results
   anidb_file_result_t* result;
   if (anidb_process_file(client, "file.mkv", &options, &result) == ANIDB_SUCCESS) {
       // Use result...
       anidb_free_file_result(result);  // Always free!
   }
   ```

3. **Use memory debugging tools:**
   ```bash
   # Valgrind (Linux)
   valgrind --leak-check=full --show-leak-kinds=all ./your_app
   
   # AddressSanitizer (compile-time)
   gcc -fsanitize=address -g your_app.c -lanidb_client_core
   
   # macOS Instruments
   instruments -t Leaks ./your_app
   ```

## Performance Problems

### Issue: Slow Processing Speed

**Symptoms:**
- Processing takes much longer than expected
- Low CPU usage
- Poor throughput

**Diagnostic Steps:**

1. **Measure actual performance:**
   ```c
   struct timespec start, end;
   clock_gettime(CLOCK_MONOTONIC, &start);
   
   // Process file
   anidb_process_file(client, "test.mkv", &options, &result);
   
   clock_gettime(CLOCK_MONOTONIC, &end);
   double elapsed = (end.tv_sec - start.tv_sec) + 
                   (end.tv_nsec - start.tv_nsec) / 1e9;
   
   double throughput_mbps = (result->file_size / 1048576.0) / elapsed;
   printf("Throughput: %.2f MB/s\n", throughput_mbps);
   ```

2. **Check for bottlenecks:**
   ```c
   // I/O bottleneck test
   void test_io_speed(const char* path) {
       FILE* f = fopen(path, "rb");
       if (!f) return;
       
       char buffer[1048576];  // 1MB
       struct timespec start, end;
       clock_gettime(CLOCK_MONOTONIC, &start);
       
       size_t total = 0;
       while (fread(buffer, 1, sizeof(buffer), f) > 0) {
           total += sizeof(buffer);
       }
       
       clock_gettime(CLOCK_MONOTONIC, &end);
       fclose(f);
       
       double elapsed = (end.tv_sec - start.tv_sec) + 
                       (end.tv_nsec - start.tv_nsec) / 1e9;
       printf("Raw I/O speed: %.2f MB/s\n", (total / 1048576.0) / elapsed);
   }
   ```

### Issue: High CPU Usage

**Solutions:**

1. **Reduce algorithm count:**
   ```c
   // Use only necessary algorithms
   anidb_hash_algorithm_t minimal[] = { ANIDB_HASH_ED2K };
   options.algorithms = minimal;
   options.algorithm_count = 1;
   ```

2. **Adjust priority:**
   ```c
   #ifdef _WIN32
   SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
   #else
   nice(10);  // Lower priority
   #endif
   ```

## API Usage Errors

### Issue: Incorrect Parameter Usage

**Common mistakes:**

1. **Wrong string encoding:**
   ```c
   // WRONG: Using system encoding
   const char* path = "C:\\fichier_français.mkv";  // May not be UTF-8
   
   // CORRECT: Ensure UTF-8
   #ifdef _WIN32
   wchar_t wide_path[] = L"C:\\fichier_français.mkv";
   char utf8_path[MAX_PATH];
   WideCharToMultiByte(CP_UTF8, 0, wide_path, -1, 
                       utf8_path, sizeof(utf8_path), NULL, NULL);
   #endif
   ```

2. **Buffer size errors:**
   ```c
   // WRONG: Buffer too small
   char error[10];
   anidb_client_get_last_error(client, error, sizeof(error));
   
   // CORRECT: Adequate buffer
   char error[256];
   anidb_client_get_last_error(client, error, sizeof(error));
   ```

3. **Null pointer handling:**
   ```c
   // WRONG: Not checking for NULL
   anidb_process_file(client, NULL, &options, &result);
   
   // CORRECT: Validate inputs
   if (file_path != NULL && options != NULL) {
       anidb_process_file(client, file_path, options, &result);
   }
   ```

### Issue: Callback Problems

**Common issues:**

1. **Wrong calling convention:**
   ```c
   // WRONG: Missing calling convention
   void my_callback(float progress, uint64_t bytes, 
                    uint64_t total, void* data) { }
   
   // CORRECT: Match expected signature exactly
   void my_callback(float progress, uint64_t bytes, 
                    uint64_t total, void* data) {
       // Implementation
   }
   ```

2. **Callback crashes:**
   ```c
   // Safe callback implementation
   void safe_progress_callback(float percentage, uint64_t bytes_processed,
                              uint64_t total_bytes, void* user_data) {
       // Validate parameters
       if (total_bytes == 0) return;
       if (percentage < 0 || percentage > 100) return;
       
       // Safely cast user data
       if (user_data != NULL) {
           my_context_t* ctx = (my_context_t*)user_data;
           // Validate context...
       }
       
       // Safe operations only
       printf("Progress: %.1f%%\r", percentage);
       fflush(stdout);
   }
   ```

## Platform-Specific Issues

### Linux Issues

1. **File descriptor limits:**
   ```c
   // Check and increase limits
   struct rlimit rlim;
   getrlimit(RLIMIT_NOFILE, &rlim);
   printf("Current FD limit: %ld\n", rlim.rlim_cur);
   
   rlim.rlim_cur = 10240;
   rlim.rlim_max = 10240;
   if (setrlimit(RLIMIT_NOFILE, &rlim) != 0) {
       perror("setrlimit");
   }
   ```

2. **Permission issues:**
   ```bash
   # Check file permissions
   ls -la video.mkv
   
   # Fix permissions
   chmod 644 video.mkv
   
   # Check process capabilities
   getcap your_application
   ```

### Windows Issues

1. **Long path support:**
   ```c
   // Enable long paths
   #ifdef _WIN32
   // Convert to extended-length path
   char* make_long_path(const char* path) {
       static char long_path[32768];
       if (strlen(path) > MAX_PATH - 12) {
           snprintf(long_path, sizeof(long_path), "\\\\?\\%s", path);
           return long_path;
       }
       return (char*)path;
   }
   #endif
   ```

2. **Antivirus interference:**
   ```c
   // Add retry logic for antivirus scanning
   int retry_count = 3;
   anidb_result_t result;
   
   do {
       result = anidb_process_file(client, path, &options, &file_result);
       if (result == ANIDB_ERROR_IO || result == ANIDB_ERROR_BUSY) {
           Sleep(1000);  // Wait 1 second
           retry_count--;
       } else {
           break;
       }
   } while (retry_count > 0);
   ```

### macOS Issues

1. **App Translocation:**
   ```bash
   # Check if app is translocated
   ps aux | grep your_app
   # Look for paths like /private/var/folders/.../AppTranslocation/...
   
   # Fix by moving app
   mv /path/to/your_app.app /Applications/
   # Then run from /Applications/
   ```

2. **Sandbox restrictions:**
   ```c
   // Check sandbox status
   #ifdef __APPLE__
   #include <sandbox.h>
   
   if (sandbox_check(getpid(), "file-read-data", 
                     SANDBOX_CHECK_NO_REPORT, path) != 0) {
       printf("Sandbox blocks access to: %s\n", path);
   }
   #endif
   ```

## Debugging Techniques

### Enable Debug Logging

```c
// Enable debug mode
anidb_config_t debug_config = {
    .enable_debug_logging = 1,
    // Other settings...
};

// Set environment variable
setenv("RUST_LOG", "debug", 1);
setenv("RUST_BACKTRACE", "1", 1);
```

### Use Debug Builds

```bash
# Build library with debug symbols
cargo build  # Debug build
# or
cargo build --release --features debug-assertions

# Use debugger
gdb ./your_application
(gdb) break anidb_process_file
(gdb) run
(gdb) backtrace
```

### Create Minimal Test Case

```c
// Minimal reproducible example
#include <stdio.h>
#include "anidb.h"

int main() {
    printf("Testing AniDB library version %s\n", anidb_get_version());
    
    if (anidb_init(ANIDB_ABI_VERSION) != ANIDB_SUCCESS) {
        fprintf(stderr, "Init failed\n");
        return 1;
    }
    
    anidb_client_handle_t client;
    if (anidb_client_create(&client) != ANIDB_SUCCESS) {
        fprintf(stderr, "Create failed\n");
        return 1;
    }
    
    // Minimal test...
    
    anidb_client_destroy(client);
    anidb_cleanup();
    return 0;
}
```

### Logging Helper

```c
// Comprehensive logging wrapper
typedef enum {
    LOG_DEBUG,
    LOG_INFO,
    LOG_WARN,
    LOG_ERROR
} log_level_t;

void log_anidb_error(const char* operation, anidb_result_t result, 
                     anidb_client_handle_t client) {
    const char* error_str = anidb_error_string(result);
    fprintf(stderr, "[ERROR] %s failed: %s (%d)\n", 
            operation, error_str, result);
    
    if (client != NULL) {
        char detailed[512];
        if (anidb_client_get_last_error(client, detailed, 
                                       sizeof(detailed)) == ANIDB_SUCCESS) {
            fprintf(stderr, "[ERROR] Details: %s\n", detailed);
        }
    }
    
    // Log to file
    FILE* log = fopen("anidb_error.log", "a");
    if (log) {
        time_t now = time(NULL);
        fprintf(log, "[%s] %s: %s\n", ctime(&now), operation, error_str);
        fclose(log);
    }
}

// Usage
anidb_result_t result = anidb_process_file(client, path, &options, &file_result);
if (result != ANIDB_SUCCESS) {
    log_anidb_error("process_file", result, client);
}
```

## Error Codes Reference

### Quick Reference Table

| Error Code | Value | Description | Common Causes |
|------------|-------|-------------|---------------|
| `ANIDB_SUCCESS` | 0 | Operation successful | N/A |
| `ANIDB_ERROR_INVALID_HANDLE` | 1 | Invalid handle provided | Null or freed handle |
| `ANIDB_ERROR_INVALID_PARAMETER` | 2 | Invalid parameter | Null pointers, zero counts |
| `ANIDB_ERROR_FILE_NOT_FOUND` | 3 | File not found | Wrong path, missing file |
| `ANIDB_ERROR_PROCESSING` | 4 | Processing error | Internal error |
| `ANIDB_ERROR_OUT_OF_MEMORY` | 5 | Out of memory | Memory limit reached |
| `ANIDB_ERROR_IO` | 6 | I/O error | Disk error, permissions |
| `ANIDB_ERROR_NETWORK` | 7 | Network error | Connection failed |
| `ANIDB_ERROR_CANCELLED` | 8 | Operation cancelled | User cancellation |
| `ANIDB_ERROR_INVALID_UTF8` | 9 | Invalid UTF-8 | Bad string encoding |
| `ANIDB_ERROR_VERSION_MISMATCH` | 10 | Version mismatch | Wrong library version |
| `ANIDB_ERROR_TIMEOUT` | 11 | Operation timeout | Too slow, hung |
| `ANIDB_ERROR_PERMISSION_DENIED` | 12 | Permission denied | File permissions |
| `ANIDB_ERROR_CACHE` | 13 | Cache error | Corrupted cache |
| `ANIDB_ERROR_BUSY` | 14 | Resource busy | Concurrent access |
| `ANIDB_ERROR_UNKNOWN` | 99 | Unknown error | Unexpected condition |

### Error Handling Best Practices

```c
// Comprehensive error handling
anidb_result_t handle_api_call(anidb_client_handle_t client, 
                               const char* operation_name) {
    anidb_result_t result = /* API call */;
    
    switch (result) {
        case ANIDB_SUCCESS:
            return result;
            
        case ANIDB_ERROR_FILE_NOT_FOUND:
            fprintf(stderr, "%s: File not found\n", operation_name);
            // Maybe prompt for new path
            break;
            
        case ANIDB_ERROR_OUT_OF_MEMORY:
            fprintf(stderr, "%s: Out of memory\n", operation_name);
            // Try to free memory
            anidb_memory_gc();
            // Retry with reduced settings
            break;
            
        case ANIDB_ERROR_NETWORK:
            fprintf(stderr, "%s: Network error\n", operation_name);
            // Check connectivity
            // Retry with backoff
            break;
            
        default:
            fprintf(stderr, "%s: %s\n", operation_name, 
                    anidb_error_string(result));
            break;
    }
    
    return result;
}
```

## FAQ

### Q: Why does the library use so much memory?

**A:** The library is optimized for speed, which requires memory for:
- Buffer pools for I/O operations
- Hash calculation state
- Result caching
- Concurrent operations

You can reduce memory usage by:
- Lowering `max_memory_usage` in configuration
- Reducing `chunk_size`
- Limiting `max_concurrent_files`
- Calling `anidb_memory_gc()` periodically

### Q: How can I process files larger than available RAM?

**A:** The library uses streaming processing:
```c
// This works for any file size
anidb_config_t config = {
    .max_memory_usage = 100 * 1024 * 1024,  // Only 100MB RAM
    .chunk_size = 65536  // Process in 64KB chunks
};

// Can process 100GB+ files with only 100MB RAM
```

### Q: Why is processing slower than expected?

**A:** Common causes:
1. **Slow storage**: HDD vs SSD makes a big difference
2. **Small chunk size**: Increase for better throughput
3. **Too many algorithms**: Each adds overhead
4. **Antivirus scanning**: Add exclusions
5. **Other I/O**: Competing disk access

### Q: Can I use the library from multiple threads?

**A:** Yes, with these guidelines:
- Each thread should have its own client handle
- Don't share handles between threads
- Global functions like `anidb_init()` are thread-safe
- Callbacks are called from background threads

### Q: How do I handle Unicode filenames?

**A:** Always use UTF-8 encoding:
```c
#ifdef _WIN32
// Convert Windows Unicode to UTF-8
wchar_t* wide_path = L"C:\\ファイル.mkv";
char utf8_path[MAX_PATH * 3];  // UTF-8 can be up to 3x size
WideCharToMultiByte(CP_UTF8, 0, wide_path, -1, 
                    utf8_path, sizeof(utf8_path), NULL, NULL);
#else
// Linux/macOS typically use UTF-8 already
const char* utf8_path = "/home/user/ファイル.mkv";
#endif
```

### Q: What's the difference between ED2K and other hashes?

**A:** ED2K is specifically designed for:
- Large file handling (special chunking algorithm)
- Compatibility with AniDB database
- Efficient calculation for media files

Other hashes are provided for compatibility but ED2K is recommended for AniDB.

### Q: How can I cancel a long-running operation?

**A:** Currently, use these strategies:
1. Process files in smaller batches
2. Use async operations (language bindings)
3. Set reasonable timeouts
4. Monitor progress and abort if needed

### Q: Is the library safe to use in production?

**A:** Yes, with proper error handling:
- Always check return values
- Free all allocated memory
- Handle all error cases
- Test with your specific workload
- Monitor memory usage

### Q: Where can I get help?

**A:** Resources:
1. Check this troubleshooting guide
2. Review the API reference
3. Look at example code
4. Enable debug logging
5. Create minimal test case
6. File issue with details