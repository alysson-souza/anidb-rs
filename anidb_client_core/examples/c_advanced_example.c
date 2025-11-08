/**
 * Advanced AniDB Client Example in C
 * 
 * This example demonstrates:
 * - Custom client configuration
 * - Progress callbacks
 * - Event system
 * - Batch processing
 * - Error handling with callbacks
 * - Cache management
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#ifdef _WIN32
    #include <windows.h>
    #define sleep(x) Sleep((x) * 1000)
#else
    #include <unistd.h>
#endif
#include "../include/anidb.h"

// Global variables for demonstration
static int g_processing_count = 0;
static int g_total_files = 0;

// Progress callback
void progress_callback(float percentage, uint64_t bytes_processed, 
                      uint64_t total_bytes, void* user_data) {
    static int last_percentage = -1;
    int current_percentage = (int)percentage;
    
    // Only print when percentage changes
    if (current_percentage != last_percentage) {
        printf("\rProgress: %3d%% [", current_percentage);
        
        // Draw progress bar
        int bar_width = 50;
        int filled = (bar_width * current_percentage) / 100;
        for (int i = 0; i < bar_width; i++) {
            if (i < filled) printf("=");
            else printf(" ");
        }
        
        printf("] %llu/%llu bytes", 
               (unsigned long long)bytes_processed,
               (unsigned long long)total_bytes);
        fflush(stdout);
        
        last_percentage = current_percentage;
    }
}

// Error callback
void error_callback(anidb_result_t error_code, const char* error_message,
                   const char* file_path, void* user_data) {
    fprintf(stderr, "\n[ERROR] Code %d: %s\n", error_code, error_message);
    if (file_path) {
        fprintf(stderr, "        File: %s\n", file_path);
    }
}

// Completion callback
void completion_callback(anidb_result_t result, void* user_data) {
    g_processing_count++;
    if (result == ANIDB_SUCCESS) {
        printf("\n[COMPLETE] File %d/%d processed successfully\n", 
               g_processing_count, g_total_files);
    } else {
        printf("\n[COMPLETE] File %d/%d failed with error: %s\n", 
               g_processing_count, g_total_files,
               anidb_error_string(result));
    }
}

// Event callback
void event_callback(const anidb_event_t* event, void* user_data) {
    switch (event->type) {
        case ANIDB_EVENT_FILE_START:
            printf("\n[EVENT] Starting file: %s (%llu bytes)\n",
                   event->data.file.file_path,
                   (unsigned long long)event->data.file.file_size);
            break;
            
        case ANIDB_EVENT_FILE_COMPLETE:
            printf("[EVENT] File complete: %s\n", event->data.file.file_path);
            if (event->context) {
                printf("        Context: %s\n", event->context);
            }
            break;
            
        case ANIDB_EVENT_HASH_START:
            printf("[EVENT] Starting %s hash calculation\n",
                   anidb_hash_algorithm_name(event->data.hash.algorithm));
            break;
            
        case ANIDB_EVENT_HASH_COMPLETE:
            printf("[EVENT] %s hash: %s\n",
                   anidb_hash_algorithm_name(event->data.hash.algorithm),
                   event->data.hash.hash_value);
            break;
            
        case ANIDB_EVENT_CACHE_HIT:
            printf("[EVENT] Cache hit for %s (%s)\n",
                   event->data.cache.file_path,
                   anidb_hash_algorithm_name(event->data.cache.algorithm));
            break;
            
        case ANIDB_EVENT_CACHE_MISS:
            printf("[EVENT] Cache miss for %s (%s)\n",
                   event->data.cache.file_path,
                   anidb_hash_algorithm_name(event->data.cache.algorithm));
            break;
            
        case ANIDB_EVENT_MEMORY_WARNING:
            printf("[EVENT] Memory warning! Current: %llu MB, Max: %llu MB\n",
                   (unsigned long long)(event->data.memory.current_usage / 1048576),
                   (unsigned long long)(event->data.memory.max_usage / 1048576));
            if (event->context) {
                printf("        Context: %s\n", event->context);
            }
            break;
            
        default:
            printf("[EVENT] Unknown event type: %d\n", event->type);
            break;
    }
}

// Process multiple files
int process_batch(anidb_client_handle_t client, const char** files, size_t file_count) {
    printf("\n=== Batch Processing %zu Files ===\n", file_count);
    
    g_processing_count = 0;
    g_total_files = (int)file_count;
    
    // Define algorithms for batch
    anidb_hash_algorithm_t algorithms[] = {
        ANIDB_HASH_ED2K,
        ANIDB_HASH_CRC32
    };
    
    // Batch options
    anidb_batch_options_t options = {
        .algorithms = algorithms,
        .algorithm_count = 2,
        .max_concurrent = 2,
        .continue_on_error = 1,
        .skip_existing = 0,
        .progress_callback = progress_callback,
        .completion_callback = completion_callback,
        .user_data = NULL
    };
    
    // Process batch
    anidb_batch_result_t* batch_result;
    anidb_result_t result = anidb_process_batch(client, files, file_count, 
                                                &options, &batch_result);
    
    if (result != ANIDB_SUCCESS) {
        fprintf(stderr, "Batch processing failed: %s\n", 
                anidb_error_string(result));
        return 1;
    }
    
    // Display batch results
    printf("\n\n=== Batch Results ===\n");
    printf("Total files: %zu\n", batch_result->total_files);
    printf("Successful: %zu\n", batch_result->successful_files);
    printf("Failed: %zu\n", batch_result->failed_files);
    printf("Total time: %llu ms\n", (unsigned long long)batch_result->total_time_ms);
    
    // Display individual results
    printf("\nIndividual Results:\n");
    for (size_t i = 0; i < batch_result->total_files; i++) {
        anidb_file_result_t* file = &batch_result->results[i];
        printf("\n[%zu] %s\n", i + 1, file->file_path);
        
        if (file->status == ANIDB_STATUS_COMPLETED) {
            printf("    Size: %llu bytes\n", (unsigned long long)file->file_size);
            printf("    Time: %llu ms\n", (unsigned long long)file->processing_time_ms);
            printf("    Hashes:\n");
            
            for (size_t j = 0; j < file->hash_count; j++) {
                printf("      %s: %s\n",
                       anidb_hash_algorithm_name(file->hashes[j].algorithm),
                       file->hashes[j].hash_value);
            }
        } else {
            printf("    Status: %s\n", 
                   file->status == ANIDB_STATUS_FAILED ? "Failed" : "Unknown");
            if (file->error_message) {
                printf("    Error: %s\n", file->error_message);
            }
        }
    }
    
    // Free batch result
    anidb_free_batch_result(batch_result);
    
    return 0;
}

int main(int argc, char* argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <file1> [file2] [file3] ...\n", argv[0]);
        return 1;
    }
    
    // Initialize library
    if (anidb_init(ANIDB_ABI_VERSION) != ANIDB_SUCCESS) {
        fprintf(stderr, "Failed to initialize library\n");
        return 1;
    }
    
    // Create client with custom configuration
    anidb_config_t config = {
        .cache_dir = ".anidb_cache",
        .max_concurrent_files = 4,
        .chunk_size = 65536,  // 64KB chunks
        .max_memory_usage = 100 * 1024 * 1024,  // 100MB limit
        .enable_debug_logging = 0,
        .username = NULL,
        .password = NULL
    };
    
    anidb_client_handle_t client;
    anidb_result_t result = anidb_client_create_with_config(&config, &client);
    if (result != ANIDB_SUCCESS) {
        fprintf(stderr, "Failed to create client: %s\n", 
                anidb_error_string(result));
        anidb_cleanup();
        return 1;
    }
    
    // Register callbacks
    printf("Registering callbacks...\n");
    uint64_t error_cb_id = anidb_register_callback(client, ANIDB_CALLBACK_ERROR,
                                                   (void*)error_callback, NULL);
    uint64_t complete_cb_id = anidb_register_callback(client, ANIDB_CALLBACK_COMPLETION,
                                                      (void*)completion_callback, NULL);
    
    if (!error_cb_id || !complete_cb_id) {
        fprintf(stderr, "Failed to register callbacks\n");
    }
    
    // Connect to event system
    printf("Connecting to event system...\n");
    result = anidb_event_connect(client, event_callback, NULL);
    if (result != ANIDB_SUCCESS) {
        fprintf(stderr, "Failed to connect to event system: %s\n",
                anidb_error_string(result));
    }
    
    // Check cache statistics before processing
    size_t cache_entries;
    uint64_t cache_size;
    if (anidb_cache_get_stats(client, &cache_entries, &cache_size) == ANIDB_SUCCESS) {
        printf("\nCache statistics before processing:\n");
        printf("  Entries: %zu\n", cache_entries);
        printf("  Size: %llu bytes\n", (unsigned long long)cache_size);
    }
    
    if (argc == 2) {
        // Single file processing with all features
        printf("\n=== Single File Processing ===\n");
        
        anidb_hash_algorithm_t algorithms[] = {
            ANIDB_HASH_ED2K,
            ANIDB_HASH_CRC32,
            ANIDB_HASH_MD5,
            ANIDB_HASH_SHA1,
            ANIDB_HASH_TTH
        };
        
        anidb_process_options_t options = {
            .algorithms = algorithms,
            .algorithm_count = 5,
            .enable_progress = 1,
            .verify_existing = 0,
            .progress_callback = progress_callback,
            .user_data = NULL
        };
        
        anidb_file_result_t* file_result;
        result = anidb_process_file(client, argv[1], &options, &file_result);
        
        if (result == ANIDB_SUCCESS) {
            printf("\n\nProcessing completed!\n");
            anidb_free_file_result(file_result);
        } else {
            fprintf(stderr, "\n\nProcessing failed: %s\n", 
                    anidb_error_string(result));
        }
    } else {
        // Batch processing
        const char** files = (const char**)&argv[1];
        size_t file_count = argc - 1;
        process_batch(client, files, file_count);
    }
    
    // Check cache statistics after processing
    if (anidb_cache_get_stats(client, &cache_entries, &cache_size) == ANIDB_SUCCESS) {
        printf("\nCache statistics after processing:\n");
        printf("  Entries: %zu\n", cache_entries);
        printf("  Size: %llu bytes\n", (unsigned long long)cache_size);
    }
    
    // Check for memory leaks (debug builds only)
#ifdef DEBUG
    uint64_t leak_count, leaked_bytes;
    if (anidb_check_memory_leaks(&leak_count, &leaked_bytes) == ANIDB_SUCCESS) {
        if (leak_count > 0) {
            printf("\nWarning: %llu memory leaks detected (%llu bytes)\n",
                   (unsigned long long)leak_count,
                   (unsigned long long)leaked_bytes);
        } else {
            printf("\nNo memory leaks detected!\n");
        }
    }
#endif
    
    // Cleanup
    printf("\nCleaning up...\n");
    
    // Disconnect from event system
    anidb_event_disconnect(client);
    
    // Unregister callbacks
    if (error_cb_id) anidb_unregister_callback(client, error_cb_id);
    if (complete_cb_id) anidb_unregister_callback(client, complete_cb_id);
    
    // Destroy client and cleanup library
    anidb_client_destroy(client);
    anidb_cleanup();
    
    printf("Done!\n");
    return 0;
}