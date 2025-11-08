/**
 * Error Handling Example in C
 * 
 * This example demonstrates:
 * - Comprehensive error handling
 * - Different error scenarios
 * - Recovery strategies
 * - Logging and debugging
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include "../include/anidb.h"

// Error context structure
typedef struct {
    int error_count;
    int warning_count;
    FILE* log_file;
} error_context_t;

// Log levels
typedef enum {
    LOG_DEBUG,
    LOG_INFO,
    LOG_WARNING,
    LOG_ERROR,
    LOG_FATAL
} log_level_t;

// Logging function
void log_message(error_context_t* ctx, log_level_t level, 
                 const char* format, ...) {
    const char* level_str[] = {
        "DEBUG", "INFO", "WARN", "ERROR", "FATAL"
    };
    
    va_list args;
    va_start(args, format);
    
    // Log to file if available
    if (ctx && ctx->log_file) {
        fprintf(ctx->log_file, "[%s] ", level_str[level]);
        vfprintf(ctx->log_file, format, args);
        fprintf(ctx->log_file, "\n");
        fflush(ctx->log_file);
    }
    
    // Also log to console for warnings and above
    if (level >= LOG_WARNING) {
        fprintf(stderr, "[%s] ", level_str[level]);
        vfprintf(stderr, format, args);
        fprintf(stderr, "\n");
        
        if (level == LOG_WARNING && ctx) {
            ctx->warning_count++;
        } else if (level >= LOG_ERROR && ctx) {
            ctx->error_count++;
        }
    }
    
    va_end(args);
}

// Error callback with context
void error_callback_with_context(anidb_result_t error_code, 
                                const char* error_message,
                                const char* file_path, 
                                void* user_data) {
    error_context_t* ctx = (error_context_t*)user_data;
    
    log_message(ctx, LOG_ERROR, "Processing error %d: %s (file: %s)",
                error_code, error_message, 
                file_path ? file_path : "N/A");
    
    // Handle specific errors
    switch (error_code) {
        case ANIDB_ERROR_FILE_NOT_FOUND:
            log_message(ctx, LOG_INFO, 
                       "Suggestion: Check if file exists and path is correct");
            break;
            
        case ANIDB_ERROR_PERMISSION_DENIED:
            log_message(ctx, LOG_INFO, 
                       "Suggestion: Check file permissions");
            break;
            
        case ANIDB_ERROR_OUT_OF_MEMORY:
            log_message(ctx, LOG_INFO, 
                       "Suggestion: Reduce concurrent operations or chunk size");
            break;
            
        case ANIDB_ERROR_NETWORK:
            log_message(ctx, LOG_INFO, 
                       "Suggestion: Check network connection and AniDB availability");
            break;
            
        default:
            break;
    }
}

// Demonstrate various error scenarios
void demonstrate_error_scenarios(anidb_client_handle_t client, 
                               error_context_t* ctx) {
    anidb_result_t result;
    
    log_message(ctx, LOG_INFO, "=== Demonstrating Error Scenarios ===");
    
    // Scenario 1: File not found
    log_message(ctx, LOG_INFO, "\n1. Testing file not found error...");
    anidb_file_result_t* result1;
    anidb_hash_algorithm_t algo = ANIDB_HASH_ED2K;
    anidb_process_options_t options = {
        .algorithms = &algo,
        .algorithm_count = 1,
        .enable_progress = 0,
        .verify_existing = 0,
        .progress_callback = NULL,
        .user_data = NULL
    };
    
    result = anidb_process_file(client, "/nonexistent/file.mkv", 
                               &options, &result1);
    if (result != ANIDB_SUCCESS) {
        log_message(ctx, LOG_INFO, "Expected error occurred: %s", 
                   anidb_error_string(result));
        
        char error_msg[256];
        if (anidb_client_get_last_error(client, error_msg, 
                                       sizeof(error_msg)) == ANIDB_SUCCESS) {
            log_message(ctx, LOG_DEBUG, "Detailed error: %s", error_msg);
        }
    }
    
    // Scenario 2: Invalid parameters
    log_message(ctx, LOG_INFO, "\n2. Testing invalid parameter errors...");
    
    // Null file path
    result = anidb_process_file(client, NULL, &options, &result1);
    if (result == ANIDB_ERROR_INVALID_PARAMETER) {
        log_message(ctx, LOG_INFO, "Correctly caught null file path");
    }
    
    // Null options
    result = anidb_process_file(client, "test.mkv", NULL, &result1);
    if (result == ANIDB_ERROR_INVALID_PARAMETER) {
        log_message(ctx, LOG_INFO, "Correctly caught null options");
    }
    
    // Invalid algorithm count
    anidb_process_options_t bad_options = options;
    bad_options.algorithm_count = 0;
    result = anidb_process_file(client, "test.mkv", &bad_options, &result1);
    if (result == ANIDB_ERROR_INVALID_PARAMETER) {
        log_message(ctx, LOG_INFO, "Correctly caught zero algorithm count");
    }
    
    // Scenario 3: Memory pressure simulation
    log_message(ctx, LOG_INFO, "\n3. Testing memory pressure handling...");
    
    // Get current memory stats
    anidb_memory_stats_t mem_stats;
    if (anidb_get_memory_stats(&mem_stats) == ANIDB_SUCCESS) {
        log_message(ctx, LOG_INFO, "Current memory usage: %llu MB",
                   (unsigned long long)(mem_stats.total_memory_used / 1048576));
        
        const char* pressure_str[] = {"Low", "Medium", "High", "Critical"};
        log_message(ctx, LOG_INFO, "Memory pressure: %s",
                   pressure_str[mem_stats.memory_pressure]);
    }
    
    // Scenario 4: Cache errors
    log_message(ctx, LOG_INFO, "\n4. Testing cache operations...");
    
    // Try to check cache for invalid file
    int is_cached;
    result = anidb_cache_check_file(client, "", ANIDB_HASH_ED2K, &is_cached);
    if (result != ANIDB_SUCCESS) {
        log_message(ctx, LOG_INFO, "Correctly caught empty file path in cache check");
    }
    
    // Scenario 5: Handle validation
    log_message(ctx, LOG_INFO, "\n5. Testing handle validation...");
    
    // Use invalid handle
    anidb_client_handle_t invalid_handle = (anidb_client_handle_t)0xDEADBEEF;
    result = anidb_client_get_last_error(invalid_handle, error_msg, 
                                        sizeof(error_msg));
    if (result == ANIDB_ERROR_INVALID_HANDLE) {
        log_message(ctx, LOG_INFO, "Correctly caught invalid handle");
    }
}

// Demonstrate recovery strategies
void demonstrate_recovery(anidb_client_handle_t client, error_context_t* ctx) {
    log_message(ctx, LOG_INFO, "\n=== Demonstrating Recovery Strategies ===");
    
    // Strategy 1: Retry with exponential backoff
    log_message(ctx, LOG_INFO, "\n1. Retry with exponential backoff");
    
    int max_retries = 3;
    int retry_delay = 1; // seconds
    
    for (int attempt = 1; attempt <= max_retries; attempt++) {
        log_message(ctx, LOG_INFO, "Attempt %d/%d", attempt, max_retries);
        
        // Simulate operation that might fail
        anidb_result_t result = ANIDB_ERROR_NETWORK; // Simulated failure
        
        if (result == ANIDB_SUCCESS) {
            log_message(ctx, LOG_INFO, "Operation succeeded on attempt %d", attempt);
            break;
        } else if (attempt < max_retries) {
            log_message(ctx, LOG_WARNING, "Operation failed, retrying in %d seconds...", 
                       retry_delay);
            sleep(retry_delay);
            retry_delay *= 2; // Exponential backoff
        } else {
            log_message(ctx, LOG_ERROR, "Operation failed after %d attempts", max_retries);
        }
    }
    
    // Strategy 2: Fallback options
    log_message(ctx, LOG_INFO, "\n2. Using fallback algorithms");
    
    anidb_hash_algorithm_t primary_algos[] = {
        ANIDB_HASH_ED2K,
        ANIDB_HASH_TTH,
        ANIDB_HASH_SHA1
    };
    
    anidb_hash_algorithm_t fallback_algos[] = {
        ANIDB_HASH_MD5,
        ANIDB_HASH_CRC32
    };
    
    log_message(ctx, LOG_INFO, "Trying primary algorithms...");
    // Simulate failure with primary algorithms
    
    log_message(ctx, LOG_INFO, "Primary failed, using fallback algorithms...");
    // Use fallback algorithms
    
    // Strategy 3: Graceful degradation
    log_message(ctx, LOG_INFO, "\n3. Graceful degradation");
    
    // Start with high performance settings
    size_t chunk_sizes[] = {1048576, 262144, 65536, 16384}; // 1MB, 256KB, 64KB, 16KB
    size_t concurrent_ops[] = {8, 4, 2, 1};
    
    for (int i = 0; i < 4; i++) {
        log_message(ctx, LOG_INFO, 
                   "Trying chunk_size=%zu, concurrent=%zu",
                   chunk_sizes[i], concurrent_ops[i]);
        
        // Simulate operation with current settings
        // If it succeeds, break
        // Otherwise, continue with reduced settings
    }
}

int main(int argc, char* argv[]) {
    // Initialize error context
    error_context_t ctx = {
        .error_count = 0,
        .warning_count = 0,
        .log_file = NULL
    };
    
    // Open log file
    ctx.log_file = fopen("anidb_errors.log", "w");
    if (!ctx.log_file) {
        fprintf(stderr, "Warning: Could not open log file\n");
    }
    
    log_message(&ctx, LOG_INFO, "=== AniDB Error Handling Example ===");
    log_message(&ctx, LOG_INFO, "Library version: %s", anidb_get_version());
    
    // Initialize with version check
    log_message(&ctx, LOG_INFO, "Checking ABI compatibility...");
    uint32_t abi_version = anidb_get_abi_version();
    if (abi_version != ANIDB_ABI_VERSION) {
        log_message(&ctx, LOG_FATAL, 
                   "ABI version mismatch! Expected %u, got %u",
                   ANIDB_ABI_VERSION, abi_version);
        if (ctx.log_file) fclose(ctx.log_file);
        return 1;
    }
    
    // Initialize library
    anidb_result_t result = anidb_init(ANIDB_ABI_VERSION);
    if (result != ANIDB_SUCCESS) {
        log_message(&ctx, LOG_FATAL, 
                   "Failed to initialize library: %s",
                   anidb_error_string(result));
        if (ctx.log_file) fclose(ctx.log_file);
        return 1;
    }
    
    // Create client with error handling
    anidb_client_handle_t client;
    result = anidb_client_create(&client);
    if (result != ANIDB_SUCCESS) {
        log_message(&ctx, LOG_FATAL, 
                   "Failed to create client: %s",
                   anidb_error_string(result));
        anidb_cleanup();
        if (ctx.log_file) fclose(ctx.log_file);
        return 1;
    }
    
    // Register error callback
    uint64_t error_cb_id = anidb_register_callback(
        client, 
        ANIDB_CALLBACK_ERROR,
        (void*)error_callback_with_context, 
        &ctx
    );
    
    if (!error_cb_id) {
        log_message(&ctx, LOG_WARNING, "Failed to register error callback");
    }
    
    // Run demonstrations
    demonstrate_error_scenarios(client, &ctx);
    demonstrate_recovery(client, &ctx);
    
    // Process a file if provided
    if (argc > 1) {
        log_message(&ctx, LOG_INFO, "\n=== Processing User File ===");
        
        anidb_hash_algorithm_t algorithms[] = {ANIDB_HASH_ED2K};
        anidb_process_options_t options = {
            .algorithms = algorithms,
            .algorithm_count = 1,
            .enable_progress = 0,
            .verify_existing = 0,
            .progress_callback = NULL,
            .user_data = NULL
        };
        
        anidb_file_result_t* file_result;
        result = anidb_process_file(client, argv[1], &options, &file_result);
        
        if (result == ANIDB_SUCCESS) {
            log_message(&ctx, LOG_INFO, "File processed successfully!");
            log_message(&ctx, LOG_INFO, "ED2K: %s", file_result->hashes[0].hash_value);
            anidb_free_file_result(file_result);
        } else {
            log_message(&ctx, LOG_ERROR, "Failed to process file: %s",
                       anidb_error_string(result));
        }
    }
    
    // Summary
    log_message(&ctx, LOG_INFO, "\n=== Error Summary ===");
    log_message(&ctx, LOG_INFO, "Total errors: %d", ctx.error_count);
    log_message(&ctx, LOG_INFO, "Total warnings: %d", ctx.warning_count);
    
    // Cleanup
    if (error_cb_id) {
        anidb_unregister_callback(client, error_cb_id);
    }
    
    anidb_client_destroy(client);
    anidb_cleanup();
    
    if (ctx.log_file) {
        log_message(&ctx, LOG_INFO, "Log file closed.");
        fclose(ctx.log_file);
    }
    
    return (ctx.error_count > 0) ? 1 : 0;
}