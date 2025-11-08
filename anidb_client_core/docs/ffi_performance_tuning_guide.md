# AniDB Client FFI Performance Tuning Guide

## Table of Contents

1. [Overview](#overview)
2. [Memory Management](#memory-management)
3. [Concurrent Operations](#concurrent-operations)
4. [Chunk Size Optimization](#chunk-size-optimization)
5. [Cache Optimization](#cache-optimization)
6. [Platform-Specific Optimizations](#platform-specific-optimizations)
7. [Benchmarking](#benchmarking)
8. [Troubleshooting Performance Issues](#troubleshooting-performance-issues)
9. [Best Practices](#best-practices)

## Overview

The AniDB Client library is designed for high-performance file processing with strict memory constraints. This guide provides detailed information on optimizing performance for different use cases and platforms.

### Key Performance Factors

1. **I/O Performance**: Disk read speed and access patterns
2. **CPU Utilization**: Hash calculation and parallel processing
3. **Memory Usage**: Buffer management and allocation patterns
4. **Concurrency**: Optimal thread count and work distribution

## Memory Management

### Memory Configuration

```c
anidb_config_t config = {
    .max_memory_usage = 200 * 1024 * 1024,  // 200MB limit
    .chunk_size = 65536,                    // 64KB chunks
    .max_concurrent_files = 4               // Limit concurrent operations
};
```

### Memory Usage Patterns

#### Small Files (<10MB)
- Lower memory limit (50-100MB)
- Smaller chunk size (16-32KB)
- More concurrent operations (8-16)

```c
// Configuration for small files
anidb_config_t small_file_config = {
    .max_memory_usage = 50 * 1024 * 1024,   // 50MB
    .chunk_size = 16384,                    // 16KB
    .max_concurrent_files = 16
};
```

#### Large Files (>1GB)
- Higher memory limit (200-500MB)
- Larger chunk size (256KB-1MB)
- Fewer concurrent operations (1-4)

```c
// Configuration for large files
anidb_config_t large_file_config = {
    .max_memory_usage = 500 * 1024 * 1024,  // 500MB
    .chunk_size = 1048576,                  // 1MB
    .max_concurrent_files = 2
};
```

### Buffer Pool Optimization

The library uses a buffer pool to reduce allocation overhead:

```c
// Monitor buffer pool efficiency
anidb_memory_stats_t stats;
anidb_get_memory_stats(&stats);

double hit_rate = (double)stats.pool_hits / 
                  (stats.pool_hits + stats.pool_misses) * 100.0;
printf("Buffer pool hit rate: %.2f%%\n", hit_rate);

// Force garbage collection if needed
if (stats.memory_pressure >= 2) {  // High or Critical
    anidb_memory_gc();
}
```

### Memory Pressure Handling

```c
void handle_memory_pressure(const anidb_memory_stats_t* stats) {
    switch (stats->memory_pressure) {
        case 0:  // Low
            // Normal operation
            break;
            
        case 1:  // Medium
            // Reduce concurrent operations
            printf("Medium memory pressure, reducing concurrency\n");
            break;
            
        case 2:  // High
            // Aggressive memory reduction
            printf("High memory pressure, forcing GC\n");
            anidb_memory_gc();
            break;
            
        case 3:  // Critical
            // Emergency measures
            printf("Critical memory pressure!\n");
            // Cancel non-essential operations
            break;
    }
}
```

## Concurrent Operations

### Optimal Concurrency Settings

The optimal number of concurrent operations depends on:
- CPU cores
- Available memory
- I/O subsystem performance
- File sizes

```c
#include <stdlib.h>
#ifdef _WIN32
    #include <windows.h>
    int get_cpu_count() {
        SYSTEM_INFO sysinfo;
        GetSystemInfo(&sysinfo);
        return sysinfo.dwNumberOfProcessors;
    }
#else
    #include <unistd.h>
    int get_cpu_count() {
        return sysconf(_SC_NPROCESSORS_ONLN);
    }
#endif

// Calculate optimal concurrency
int calculate_optimal_concurrency(size_t avg_file_size, size_t available_memory) {
    int cpu_count = get_cpu_count();
    
    // Memory-based limit
    size_t memory_per_file = avg_file_size / 10;  // Rough estimate
    int memory_limit = available_memory / memory_per_file;
    
    // CPU-based limit (leave some cores for system)
    int cpu_limit = (cpu_count > 2) ? cpu_count - 1 : 1;
    
    // I/O-based limit (SSDs can handle more)
    int io_limit = is_ssd() ? 8 : 4;
    
    // Return the minimum of all limits
    int optimal = cpu_limit;
    if (memory_limit < optimal) optimal = memory_limit;
    if (io_limit < optimal) optimal = io_limit;
    
    return (optimal > 0) ? optimal : 1;
}
```

### Batch Processing Optimization

```c
// Adaptive batch processing
typedef struct {
    size_t total_size;
    size_t file_count;
    double avg_size;
    double std_dev;
} batch_stats_t;

batch_stats_t analyze_batch(const char** files, size_t count) {
    batch_stats_t stats = {0};
    
    for (size_t i = 0; i < count; i++) {
        struct stat st;
        if (stat(files[i], &st) == 0) {
            stats.total_size += st.st_size;
            stats.file_count++;
        }
    }
    
    stats.avg_size = (double)stats.total_size / stats.file_count;
    return stats;
}

anidb_batch_options_t optimize_batch_options(const char** files, size_t count) {
    batch_stats_t stats = analyze_batch(files, count);
    
    anidb_batch_options_t options = {0};
    
    // Adjust concurrency based on file sizes
    if (stats.avg_size < 100 * 1024 * 1024) {  // <100MB average
        options.max_concurrent = 8;
    } else if (stats.avg_size < 1024 * 1024 * 1024) {  // <1GB average
        options.max_concurrent = 4;
    } else {  // Very large files
        options.max_concurrent = 2;
    }
    
    // Enable skipping for large batches
    options.skip_existing = (count > 100) ? 1 : 0;
    
    // Always continue on error for large batches
    options.continue_on_error = (count > 10) ? 1 : 0;
    
    return options;
}
```

## Chunk Size Optimization

### Finding Optimal Chunk Size

```c
// Benchmark different chunk sizes
typedef struct {
    size_t chunk_size;
    double throughput_mbps;
    double cpu_usage;
} chunk_benchmark_t;

chunk_benchmark_t benchmark_chunk_size(anidb_client_handle_t client,
                                      const char* test_file,
                                      size_t chunk_size) {
    chunk_benchmark_t result = {
        .chunk_size = chunk_size,
        .throughput_mbps = 0,
        .cpu_usage = 0
    };
    
    // Create client with specific chunk size
    anidb_config_t config = {
        .chunk_size = chunk_size,
        .max_concurrent_files = 1
    };
    
    // Time the operation
    clock_t start = clock();
    
    anidb_hash_algorithm_t algo = ANIDB_HASH_ED2K;
    anidb_process_options_t options = {
        .algorithms = &algo,
        .algorithm_count = 1
    };
    
    anidb_file_result_t* file_result;
    if (anidb_process_file(client, test_file, &options, &file_result) == ANIDB_SUCCESS) {
        clock_t end = clock();
        double elapsed = (double)(end - start) / CLOCKS_PER_SEC;
        
        result.throughput_mbps = (file_result->file_size / 1048576.0) / elapsed;
        
        anidb_free_file_result(file_result);
    }
    
    return result;
}

// Find optimal chunk size for system
size_t find_optimal_chunk_size(anidb_client_handle_t client, const char* test_file) {
    size_t chunk_sizes[] = {
        16384,    // 16KB
        32768,    // 32KB
        65536,    // 64KB
        131072,   // 128KB
        262144,   // 256KB
        524288,   // 512KB
        1048576   // 1MB
    };
    
    size_t optimal_size = 65536;  // Default
    double best_throughput = 0;
    
    for (int i = 0; i < 7; i++) {
        chunk_benchmark_t result = benchmark_chunk_size(client, test_file, chunk_sizes[i]);
        
        printf("Chunk size %zu: %.2f MB/s\n", 
               chunk_sizes[i], result.throughput_mbps);
        
        if (result.throughput_mbps > best_throughput) {
            best_throughput = result.throughput_mbps;
            optimal_size = chunk_sizes[i];
        }
    }
    
    return optimal_size;
}
```

### Adaptive Chunk Sizing

```c
// Adjust chunk size based on file size
size_t calculate_chunk_size(uint64_t file_size) {
    if (file_size < 1 * 1024 * 1024) {          // <1MB
        return 16384;                            // 16KB
    } else if (file_size < 10 * 1024 * 1024) {  // <10MB
        return 32768;                            // 32KB
    } else if (file_size < 100 * 1024 * 1024) { // <100MB
        return 65536;                            // 64KB
    } else if (file_size < 1024 * 1024 * 1024) {// <1GB
        return 262144;                           // 256KB
    } else {                                     // >1GB
        return 1048576;                          // 1MB
    }
}
```

## Cache Optimization

### Cache Configuration

```c
// Optimal cache settings for different scenarios
typedef struct {
    const char* name;
    size_t max_entries;
    uint64_t max_size_bytes;
    int ttl_seconds;
} cache_profile_t;

cache_profile_t cache_profiles[] = {
    // Desktop application
    {
        .name = "desktop",
        .max_entries = 100000,
        .max_size_bytes = 1024 * 1024 * 1024,  // 1GB
        .ttl_seconds = 86400 * 30              // 30 days
    },
    // Server/batch processing
    {
        .name = "server",
        .max_entries = 1000000,
        .max_size_bytes = 10 * 1024 * 1024 * 1024,  // 10GB
        .ttl_seconds = 86400 * 90                    // 90 days
    },
    // Mobile/embedded
    {
        .name = "mobile",
        .max_entries = 10000,
        .max_size_bytes = 100 * 1024 * 1024,   // 100MB
        .ttl_seconds = 86400 * 7                // 7 days
    }
};
```

### Cache Preloading

```c
// Preload cache for better performance
void preload_cache(anidb_client_handle_t client, const char** files, size_t count) {
    printf("Preloading cache for %zu files...\n", count);
    
    int cached_count = 0;
    for (size_t i = 0; i < count; i++) {
        int is_cached;
        if (anidb_cache_check_file(client, files[i], ANIDB_HASH_ED2K, &is_cached) == ANIDB_SUCCESS) {
            if (is_cached) cached_count++;
        }
    }
    
    printf("Cache hit rate: %.2f%% (%d/%zu files)\n",
           (double)cached_count / count * 100.0, cached_count, count);
}
```

### Cache Maintenance

```c
// Periodic cache maintenance
void maintain_cache(anidb_client_handle_t client) {
    size_t entries;
    uint64_t size;
    
    if (anidb_cache_get_stats(client, &entries, &size) == ANIDB_SUCCESS) {
        printf("Cache stats: %zu entries, %.2f MB\n", 
               entries, size / 1048576.0);
        
        // Clear cache if too large
        if (size > 5 * 1024 * 1024 * 1024) {  // 5GB
            printf("Cache too large, clearing...\n");
            anidb_cache_clear(client);
        }
    }
}
```

## Platform-Specific Optimizations

### Linux Optimizations

```c
#ifdef __linux__
// Use O_DIRECT for large files to bypass page cache
#define _GNU_SOURCE
#include <fcntl.h>

void optimize_linux_io() {
    // Set I/O scheduler for better throughput
    system("echo deadline > /sys/block/sda/queue/scheduler");
    
    // Increase read-ahead
    system("echo 2048 > /sys/block/sda/queue/read_ahead_kb");
    
    // Drop caches before large operations
    system("echo 3 > /proc/sys/vm/drop_caches");
}

// CPU affinity for hash calculations
#include <sched.h>
void set_cpu_affinity(int cpu_id) {
    cpu_set_t cpuset;
    CPU_ZERO(&cpuset);
    CPU_SET(cpu_id, &cpuset);
    sched_setaffinity(0, sizeof(cpuset), &cpuset);
}
#endif
```

### Windows Optimizations

```c
#ifdef _WIN32
#include <windows.h>

void optimize_windows_io() {
    // Set process priority
    SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS);
    
    // Enable large pages
    HANDLE token;
    TOKEN_PRIVILEGES tp;
    
    if (OpenProcessToken(GetCurrentProcess(), 
                        TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &token)) {
        LookupPrivilegeValue(NULL, SE_LOCK_MEMORY_NAME, &tp.Privileges[0].Luid);
        tp.PrivilegeCount = 1;
        tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;
        AdjustTokenPrivileges(token, FALSE, &tp, 0, NULL, 0);
        CloseHandle(token);
    }
}

// NUMA optimization
void optimize_numa() {
    ULONG highestNodeNumber;
    if (GetNumaHighestNodeNumber(&highestNodeNumber)) {
        // Distribute work across NUMA nodes
        for (ULONG node = 0; node <= highestNodeNumber; node++) {
            // Set processor affinity for NUMA node
            GROUP_AFFINITY affinity;
            if (GetNumaNodeProcessorMaskEx(node, &affinity)) {
                SetThreadGroupAffinity(GetCurrentThread(), &affinity, NULL);
            }
        }
    }
}
#endif
```

### macOS Optimizations

```c
#ifdef __APPLE__
#include <sys/sysctl.h>

void optimize_macos_io() {
    // Disable Spotlight for processing directory
    system("mdutil -i off /path/to/processing/dir");
    
    // Increase file descriptor limits
    struct rlimit rlim;
    rlim.rlim_cur = 10240;
    rlim.rlim_max = 10240;
    setrlimit(RLIMIT_NOFILE, &rlim);
}

// Get cache line size for optimal alignment
size_t get_cache_line_size() {
    size_t line_size = 0;
    size_t size = sizeof(line_size);
    sysctlbyname("hw.cachelinesize", &line_size, &size, NULL, 0);
    return line_size;
}
#endif
```

## Benchmarking

### Performance Measurement Framework

```c
typedef struct {
    const char* name;
    struct timespec start;
    struct timespec end;
    uint64_t bytes_processed;
    int operations;
} perf_timer_t;

void perf_timer_start(perf_timer_t* timer, const char* name) {
    timer->name = name;
    timer->bytes_processed = 0;
    timer->operations = 0;
    clock_gettime(CLOCK_MONOTONIC, &timer->start);
}

void perf_timer_stop(perf_timer_t* timer) {
    clock_gettime(CLOCK_MONOTONIC, &timer->end);
}

void perf_timer_report(const perf_timer_t* timer) {
    double elapsed = (timer->end.tv_sec - timer->start.tv_sec) +
                    (timer->end.tv_nsec - timer->start.tv_nsec) / 1e9;
    
    printf("=== Performance Report: %s ===\n", timer->name);
    printf("Total time: %.3f seconds\n", elapsed);
    printf("Operations: %d\n", timer->operations);
    
    if (timer->operations > 0) {
        printf("Ops/second: %.2f\n", timer->operations / elapsed);
    }
    
    if (timer->bytes_processed > 0) {
        double throughput_mbps = (timer->bytes_processed / 1048576.0) / elapsed;
        printf("Throughput: %.2f MB/s\n", throughput_mbps);
    }
}
```

### Comprehensive Benchmark Suite

```c
void run_benchmark_suite(anidb_client_handle_t client) {
    printf("=== AniDB Client Benchmark Suite ===\n\n");
    
    // Test files of different sizes
    typedef struct {
        const char* path;
        size_t size;
    } test_file_t;
    
    test_file_t test_files[] = {
        {"small.bin", 1 * 1024 * 1024},      // 1MB
        {"medium.bin", 100 * 1024 * 1024},   // 100MB
        {"large.bin", 1024 * 1024 * 1024},   // 1GB
    };
    
    // Create test files
    for (int i = 0; i < 3; i++) {
        create_test_file(test_files[i].path, test_files[i].size);
    }
    
    // Benchmark 1: Single file, different algorithms
    printf("1. Algorithm Performance\n");
    anidb_hash_algorithm_t algorithms[] = {
        ANIDB_HASH_ED2K,
        ANIDB_HASH_CRC32,
        ANIDB_HASH_MD5,
        ANIDB_HASH_SHA1,
        ANIDB_HASH_TTH
    };
    
    for (int i = 0; i < 5; i++) {
        perf_timer_t timer;
        perf_timer_start(&timer, anidb_hash_algorithm_name(algorithms[i]));
        
        anidb_process_options_t options = {
            .algorithms = &algorithms[i],
            .algorithm_count = 1
        };
        
        anidb_file_result_t* result;
        if (anidb_process_file(client, test_files[1].path, &options, &result) == ANIDB_SUCCESS) {
            timer.bytes_processed = result->file_size;
            timer.operations = 1;
            anidb_free_file_result(result);
        }
        
        perf_timer_stop(&timer);
        perf_timer_report(&timer);
        printf("\n");
    }
    
    // Benchmark 2: Concurrent operations
    printf("2. Concurrent Operations\n");
    for (int concurrent = 1; concurrent <= 8; concurrent *= 2) {
        perf_timer_t timer;
        char name[64];
        snprintf(name, sizeof(name), "%d concurrent", concurrent);
        perf_timer_start(&timer, name);
        
        // Process multiple files concurrently
        const char* files[] = {
            test_files[0].path,
            test_files[0].path,
            test_files[0].path,
            test_files[0].path,
            test_files[0].path,
            test_files[0].path,
            test_files[0].path,
            test_files[0].path
        };
        
        anidb_batch_options_t batch_options = {
            .algorithms = algorithms,
            .algorithm_count = 1,
            .max_concurrent = concurrent
        };
        
        anidb_batch_result_t* batch_result;
        if (anidb_process_batch(client, files, 8, &batch_options, &batch_result) == ANIDB_SUCCESS) {
            timer.bytes_processed = test_files[0].size * 8;
            timer.operations = 8;
            anidb_free_batch_result(batch_result);
        }
        
        perf_timer_stop(&timer);
        perf_timer_report(&timer);
        printf("\n");
    }
    
    // Cleanup test files
    for (int i = 0; i < 3; i++) {
        remove(test_files[i].path);
    }
}
```

## Troubleshooting Performance Issues

### Performance Diagnostic Tool

```c
typedef struct {
    double io_wait_time;
    double cpu_time;
    double memory_wait_time;
    int cache_misses;
    int buffer_pool_misses;
} perf_diagnostics_t;

void diagnose_performance(anidb_client_handle_t client, const char* file_path) {
    printf("=== Performance Diagnostics ===\n");
    
    // Get initial state
    anidb_memory_stats_t mem_before;
    anidb_get_memory_stats(&mem_before);
    
    // Process file with detailed timing
    struct timespec start, io_start, io_end, cpu_start, cpu_end;
    clock_gettime(CLOCK_MONOTONIC, &start);
    
    // Check cache
    int is_cached;
    anidb_cache_check_file(client, file_path, ANIDB_HASH_ED2K, &is_cached);
    
    if (!is_cached) {
        printf("Cache miss - will read from disk\n");
    }
    
    // Process file
    anidb_hash_algorithm_t algo = ANIDB_HASH_ED2K;
    anidb_process_options_t options = {
        .algorithms = &algo,
        .algorithm_count = 1
    };
    
    anidb_file_result_t* result;
    anidb_result_t status = anidb_process_file(client, file_path, &options, &result);
    
    struct timespec end;
    clock_gettime(CLOCK_MONOTONIC, &end);
    
    if (status == ANIDB_SUCCESS) {
        double total_time = (end.tv_sec - start.tv_sec) +
                           (end.tv_nsec - start.tv_nsec) / 1e9;
        
        printf("\nPerformance Analysis:\n");
        printf("Total time: %.3f seconds\n", total_time);
        printf("File size: %.2f MB\n", result->file_size / 1048576.0);
        printf("Throughput: %.2f MB/s\n", 
               (result->file_size / 1048576.0) / total_time);
        
        anidb_free_file_result(result);
    }
    
    // Get memory stats
    anidb_memory_stats_t mem_after;
    anidb_get_memory_stats(&mem_after);
    
    printf("\nMemory Analysis:\n");
    printf("Memory allocated: %lld bytes\n",
           (long long)(mem_after.ffi_allocated - mem_before.ffi_allocated));
    printf("Buffer pool hits: %llu\n", 
           (unsigned long long)(mem_after.pool_hits - mem_before.pool_hits));
    printf("Buffer pool misses: %llu\n",
           (unsigned long long)(mem_after.pool_misses - mem_before.pool_misses));
    
    // Recommendations
    printf("\nRecommendations:\n");
    
    if (mem_after.memory_pressure > 1) {
        printf("- High memory pressure detected. Consider:\n");
        printf("  * Reducing chunk size\n");
        printf("  * Limiting concurrent operations\n");
        printf("  * Increasing memory limit\n");
    }
    
    double hit_rate = (double)(mem_after.pool_hits - mem_before.pool_hits) /
                     ((mem_after.pool_hits - mem_before.pool_hits) + 
                      (mem_after.pool_misses - mem_before.pool_misses)) * 100.0;
    
    if (hit_rate < 80.0) {
        printf("- Low buffer pool hit rate (%.1f%%). Consider:\n", hit_rate);
        printf("  * Increasing buffer pool size\n");
        printf("  * Processing files in size-order\n");
    }
}
```

### Common Performance Issues

#### 1. Slow I/O Performance

**Symptoms:**
- Low throughput
- High I/O wait times
- Process mostly idle

**Solutions:**
```c
// Check if file is on slow storage
int is_slow_storage(const char* path) {
    struct statfs fs_stat;
    if (statfs(path, &fs_stat) == 0) {
        // Check filesystem type
        if (fs_stat.f_type == 0x01021994) {  // TMPFS
            return 0;  // RAM disk is fast
        } else if (fs_stat.f_type == 0x858458f6) {  // RAMFS
            return 0;
        }
        // Add more filesystem checks as needed
    }
    return 1;  // Assume slow by default
}

// Optimize for slow storage
void optimize_for_slow_storage(anidb_config_t* config) {
    config->chunk_size = 1048576;        // Large chunks (1MB)
    config->max_concurrent_files = 1;    // Sequential processing
}
```

#### 2. High CPU Usage

**Symptoms:**
- 100% CPU usage
- Low throughput relative to CPU
- System unresponsive

**Solutions:**
```c
// Reduce CPU usage
void reduce_cpu_usage(anidb_client_handle_t client) {
    // Use fewer algorithms
    anidb_hash_algorithm_t minimal_algos[] = { ANIDB_HASH_ED2K };
    
    // Add small delays between operations
    usleep(1000);  // 1ms delay
    
    // Lower process priority
#ifdef _WIN32
    SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
#else
    nice(10);
#endif
}
```

#### 3. Memory Exhaustion

**Symptoms:**
- Out of memory errors
- System swapping
- Extreme slowdown

**Solutions:**
```c
// Emergency memory reduction
void emergency_memory_reduction(anidb_client_handle_t client) {
    // Force garbage collection
    anidb_memory_gc();
    
    // Reduce all limits to minimum
    anidb_config_t emergency_config = {
        .max_memory_usage = 50 * 1024 * 1024,  // 50MB only
        .chunk_size = 4096,                    // 4KB chunks
        .max_concurrent_files = 1              // Single file only
    };
    
    // Clear cache to free memory
    anidb_cache_clear(client);
}
```

## Best Practices

### 1. Profile Before Optimizing

Always measure performance before making changes:

```c
// Simple profiling macro
#define PROFILE(name, code) do { \
    struct timespec _start, _end; \
    clock_gettime(CLOCK_MONOTONIC, &_start); \
    code \
    clock_gettime(CLOCK_MONOTONIC, &_end); \
    double _elapsed = (_end.tv_sec - _start.tv_sec) + \
                     (_end.tv_nsec - _start.tv_nsec) / 1e9; \
    printf("[PROFILE] %s: %.3f seconds\n", name, _elapsed); \
} while(0)

// Usage
PROFILE("File processing", {
    anidb_process_file(client, "test.mkv", &options, &result);
});
```

### 2. Adaptive Configuration

Adjust settings based on runtime conditions:

```c
typedef struct {
    size_t available_memory;
    int cpu_count;
    int is_ssd;
    double avg_file_size;
} system_profile_t;

anidb_config_t adaptive_config(const system_profile_t* profile) {
    anidb_config_t config = {0};
    
    // Memory settings
    config.max_memory_usage = profile->available_memory / 4;  // Use 25% of available
    
    // Chunk size based on storage type
    if (profile->is_ssd) {
        config.chunk_size = 65536;   // 64KB for SSD
    } else {
        config.chunk_size = 262144;  // 256KB for HDD
    }
    
    // Concurrency based on CPU and file size
    if (profile->avg_file_size < 100 * 1024 * 1024) {  // Small files
        config.max_concurrent_files = profile->cpu_count;
    } else {  // Large files
        config.max_concurrent_files = profile->cpu_count / 2;
    }
    
    return config;
}
```

### 3. Monitor and Adjust

Continuously monitor performance and adjust:

```c
// Performance monitoring loop
void* performance_monitor(void* arg) {
    anidb_client_handle_t client = (anidb_client_handle_t)arg;
    
    while (1) {
        sleep(60);  // Check every minute
        
        anidb_memory_stats_t stats;
        if (anidb_get_memory_stats(&stats) == ANIDB_SUCCESS) {
            // Log performance metrics
            log_performance_metrics(&stats);
            
            // Auto-adjust if needed
            if (stats.memory_pressure >= 2) {
                printf("High memory pressure detected, forcing GC\n");
                anidb_memory_gc();
            }
        }
    }
    
    return NULL;
}
```

### 4. Batch Operations Efficiently

Process files in optimal order:

```c
// Sort files by size for better cache utilization
int compare_file_size(const void* a, const void* b) {
    const char* file_a = *(const char**)a;
    const char* file_b = *(const char**)b;
    
    struct stat stat_a, stat_b;
    stat(file_a, &stat_a);
    stat(file_b, &stat_b);
    
    return (stat_a.st_size > stat_b.st_size) - (stat_a.st_size < stat_b.st_size);
}

void process_files_optimally(anidb_client_handle_t client, 
                           char** files, size_t count) {
    // Sort files by size
    qsort(files, count, sizeof(char*), compare_file_size);
    
    // Process in batches of similar sizes
    size_t batch_start = 0;
    while (batch_start < count) {
        size_t batch_end = batch_start + 1;
        
        // Group files of similar size
        struct stat first_stat;
        stat(files[batch_start], &first_stat);
        
        while (batch_end < count) {
            struct stat stat;
            stat(files[batch_end], &stat);
            
            // If size difference > 2x, start new batch
            if (stat.st_size > first_stat.st_size * 2 ||
                stat.st_size < first_stat.st_size / 2) {
                break;
            }
            batch_end++;
        }
        
        // Process this batch with optimized settings
        size_t batch_size = batch_end - batch_start;
        process_batch_with_size_optimization(client, &files[batch_start], 
                                           batch_size, first_stat.st_size);
        
        batch_start = batch_end;
    }
}
```

### 5. Platform-Specific Tuning

Always consider platform differences:

```c
void apply_platform_optimizations() {
#ifdef __linux__
    // Linux: Use huge pages if available
    system("echo always > /sys/kernel/mm/transparent_hugepage/enabled");
#elif defined(_WIN32)
    // Windows: Enable large page support
    enable_large_pages();
#elif defined(__APPLE__)
    // macOS: Disable App Nap
    system("defaults write NSGlobalDomain NSAppSleepDisabled -bool YES");
#endif
}
```

## Conclusion

Performance tuning is an iterative process that requires:
1. Understanding your workload characteristics
2. Measuring actual performance
3. Identifying bottlenecks
4. Applying targeted optimizations
5. Validating improvements

Always benchmark changes and be prepared to adjust settings based on real-world usage patterns. The library is designed to be flexible enough to handle a wide range of scenarios efficiently with proper tuning.
