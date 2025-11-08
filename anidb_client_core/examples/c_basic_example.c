/**
 * Basic AniDB Client Example in C
 * 
 * This example demonstrates:
 * - Library initialization
 * - Client creation and configuration
 * - Single file processing
 * - Error handling
 * - Memory cleanup
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../include/anidb.h"

int main(int argc, char* argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <file_path>\n", argv[0]);
        return 1;
    }
    
    const char* file_path = argv[1];
    
    // Initialize the library
    printf("Initializing AniDB library...\n");
    anidb_result_t result = anidb_init(ANIDB_ABI_VERSION);
    if (result != ANIDB_SUCCESS) {
        fprintf(stderr, "Failed to initialize library: %s\n", 
                anidb_error_string(result));
        return 1;
    }
    
    printf("Library version: %s\n", anidb_get_version());
    printf("ABI version: %u\n", anidb_get_abi_version());
    
    // Create client with default configuration
    printf("\nCreating AniDB client...\n");
    anidb_client_handle_t client;
    result = anidb_client_create(&client);
    if (result != ANIDB_SUCCESS) {
        fprintf(stderr, "Failed to create client: %s\n", 
                anidb_error_string(result));
        anidb_cleanup();
        return 1;
    }
    
    // Define hash algorithms to calculate
    anidb_hash_algorithm_t algorithms[] = {
        ANIDB_HASH_ED2K,
        ANIDB_HASH_CRC32,
        ANIDB_HASH_MD5,
        ANIDB_HASH_SHA1
    };
    
    // Set up processing options
    anidb_process_options_t options = {
        .algorithms = algorithms,
        .algorithm_count = sizeof(algorithms) / sizeof(algorithms[0]),
        .enable_progress = 0,  // No progress callback for basic example
        .verify_existing = 0,
        .progress_callback = NULL,
        .user_data = NULL
    };
    
    // Process the file
    printf("\nProcessing file: %s\n", file_path);
    anidb_file_result_t* file_result;
    result = anidb_process_file(client, file_path, &options, &file_result);
    
    if (result != ANIDB_SUCCESS) {
        fprintf(stderr, "Failed to process file: %s\n", 
                anidb_error_string(result));
        
        // Get detailed error message
        char error_msg[256];
        if (anidb_client_get_last_error(client, error_msg, sizeof(error_msg)) == ANIDB_SUCCESS) {
            fprintf(stderr, "Details: %s\n", error_msg);
        }
    } else {
        // Display results
        printf("\nFile processing completed successfully!\n");
        printf("File: %s\n", file_result->file_path);
        printf("Size: %llu bytes\n", (unsigned long long)file_result->file_size);
        printf("Processing time: %llu ms\n", (unsigned long long)file_result->processing_time_ms);
        printf("\nHashes:\n");
        
        for (size_t i = 0; i < file_result->hash_count; i++) {
            const char* algo_name = anidb_hash_algorithm_name(file_result->hashes[i].algorithm);
            printf("  %s: %s\n", algo_name, file_result->hashes[i].hash_value);
        }
        
        // Free the result
        anidb_free_file_result(file_result);
    }
    
    // Get memory statistics
    printf("\nMemory Statistics:\n");
    anidb_memory_stats_t mem_stats;
    if (anidb_get_memory_stats(&mem_stats) == ANIDB_SUCCESS) {
        printf("  Total memory used: %llu bytes\n", 
               (unsigned long long)mem_stats.total_memory_used);
        printf("  FFI allocated: %llu bytes\n", 
               (unsigned long long)mem_stats.ffi_allocated);
        printf("  Buffer pool memory: %llu bytes\n", 
               (unsigned long long)mem_stats.pool_memory);
        printf("  Memory pressure: %s\n",
               mem_stats.memory_pressure == 0 ? "Low" :
               mem_stats.memory_pressure == 1 ? "Medium" :
               mem_stats.memory_pressure == 2 ? "High" : "Critical");
    }
    
    // Cleanup
    printf("\nCleaning up...\n");
    anidb_client_destroy(client);
    anidb_cleanup();
    
    printf("Done!\n");
    return 0;
}