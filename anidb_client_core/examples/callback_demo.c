/**
 * Example demonstrating the AniDB callback system
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../include/anidb.h"

// Progress callback
void on_progress(float percentage, uint64_t bytes_processed, uint64_t total_bytes, void* user_data) {
    const char* file_name = (const char*)user_data;
    printf("[Progress] %s: %.1f%% (%llu / %llu bytes)\n", 
           file_name, percentage, bytes_processed, total_bytes);
}

// Error callback
void on_error(anidb_result_t error_code, const char* error_message, 
              const char* file_path, void* user_data) {
    printf("[Error] Code %d: %s (file: %s)\n", error_code, error_message, 
           file_path ? file_path : "unknown");
}

// Completion callback
void on_completion(anidb_result_t result, void* user_data) {
    const char* file_name = (const char*)user_data;
    printf("[Completion] %s: %s\n", file_name, 
           result == ANIDB_SUCCESS ? "Success" : "Failed");
}

// Event callback
void on_event(const anidb_event_t* event, void* user_data) {
    printf("[Event] Type %d at timestamp %llu", event->type, event->timestamp);
    
    switch (event->type) {
        case ANIDB_EVENT_FILE_START:
            printf(" - File start: %s (%llu bytes)\n", 
                   event->data.file.file_path, event->data.file.file_size);
            break;
        case ANIDB_EVENT_FILE_COMPLETE:
            printf(" - File complete: %s\n", event->data.file.file_path);
            break;
        case ANIDB_EVENT_HASH_START:
            printf(" - Hash start: %s\n", 
                   anidb_hash_algorithm_name(event->data.hash.algorithm));
            break;
        case ANIDB_EVENT_HASH_COMPLETE:
            printf(" - Hash complete: %s = %s\n",
                   anidb_hash_algorithm_name(event->data.hash.algorithm),
                   event->data.hash.hash_value);
            break;
        default:
            printf(" - Other event\n");
            break;
    }
}

int main(int argc, char* argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <file_path>\n", argv[0]);
        return 1;
    }
    
    const char* file_path = argv[1];
    
    // Initialize library
    if (anidb_init(ANIDB_ABI_VERSION) != ANIDB_SUCCESS) {
        fprintf(stderr, "Failed to initialize AniDB library\n");
        return 1;
    }
    
    // Create client
    anidb_client_handle_t client = NULL;
    if (anidb_client_create(&client) != ANIDB_SUCCESS) {
        fprintf(stderr, "Failed to create client\n");
        anidb_cleanup();
        return 1;
    }
    
    // Register callbacks
    uint64_t progress_id = anidb_register_callback(client, ANIDB_CALLBACK_PROGRESS, 
                                                   (void*)on_progress, (void*)file_path);
    uint64_t error_id = anidb_register_callback(client, ANIDB_CALLBACK_ERROR,
                                               (void*)on_error, NULL);
    uint64_t completion_id = anidb_register_callback(client, ANIDB_CALLBACK_COMPLETION,
                                                    (void*)on_completion, (void*)file_path);
    
    printf("Registered callbacks: progress=%llu, error=%llu, completion=%llu\n",
           progress_id, error_id, completion_id);
    
    // Connect to event system
    if (anidb_event_connect(client, on_event, NULL) == ANIDB_SUCCESS) {
        printf("Connected to event system\n");
    }
    
    // Process file
    anidb_hash_algorithm_t algorithms[] = { ANIDB_HASH_ED2K, ANIDB_HASH_CRC32 };
    anidb_process_options_t options = {
        .algorithms = algorithms,
        .algorithm_count = 2,
        .enable_progress = 1,
        .verify_existing = 0,
        .progress_callback = NULL,  // Using registered callback instead
        .user_data = NULL
    };
    
    anidb_file_result_t* result = NULL;
    anidb_result_t status = anidb_process_file(client, file_path, &options, &result);
    
    if (status == ANIDB_SUCCESS && result != NULL) {
        printf("\nFile processed successfully:\n");
        printf("  Path: %s\n", result->file_path);
        printf("  Size: %llu bytes\n", result->file_size);
        printf("  Time: %llu ms\n", result->processing_time_ms);
        
        for (size_t i = 0; i < result->hash_count; i++) {
            printf("  %s: %s\n", 
                   anidb_hash_algorithm_name(result->hashes[i].algorithm),
                   result->hashes[i].hash_value);
        }
        
        anidb_free_file_result(result);
    } else {
        printf("\nFailed to process file: %s\n", anidb_error_string(status));
    }
    
    // Poll for any remaining events
    anidb_event_t events[10];
    size_t event_count = 0;
    if (anidb_event_poll(client, events, 10, &event_count) == ANIDB_SUCCESS) {
        printf("\nPolled %zu events from queue\n", event_count);
    }
    
    // Cleanup
    anidb_event_disconnect(client);
    anidb_unregister_callback(client, progress_id);
    anidb_unregister_callback(client, error_id);
    anidb_unregister_callback(client, completion_id);
    anidb_client_destroy(client);
    anidb_cleanup();
    
    return 0;
}