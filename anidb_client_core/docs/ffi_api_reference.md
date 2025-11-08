# AniDB Client FFI API Reference

> **Note**: For the internal architecture and module structure, see [FFI Architecture Overview](ffi_architecture.md).

## Table of Contents

1. [Overview](#overview)
2. [Library Initialization](#library-initialization)
3. [Client Management](#client-management)
4. [File Processing](#file-processing)
5. [Batch Processing](#batch-processing)
6. [Hash Calculation](#hash-calculation)
7. [Cache Management](#cache-management)
8. [Anime Identification](#anime-identification)
9. [Memory Management](#memory-management)
10. [Callback Management](#callback-management)
11. [Event System](#event-system)
12. [Memory Statistics](#memory-statistics)
13. [Error Handling](#error-handling)
14. [Constants and Enumerations](#constants-and-enumerations)

## Overview

The AniDB Client FFI provides a C-compatible interface for integrating the AniDB client functionality into applications written in various programming languages. The API follows these design principles:

- **Opaque Handle Pattern**: All objects are represented by opaque handles for safety
- **Consistent Error Handling**: All functions return `anidb_result_t` status codes
- **Memory Safety**: Clear ownership rules and explicit memory management functions
- **Thread Safety**: All functions are thread-safe when used with different handles
- **UTF-8 Strings**: All strings are expected to be UTF-8 encoded

### Version Compatibility

```c
#define ANIDB_VERSION_MAJOR 1
#define ANIDB_VERSION_MINOR 0
#define ANIDB_VERSION_PATCH 0
#define ANIDB_ABI_VERSION 1
```

Always check ABI compatibility when initializing the library:

```c
anidb_result_t result = anidb_init(ANIDB_ABI_VERSION);
```

## Library Initialization

### anidb_init

Initialize the AniDB client library. Must be called before any other library functions.

```c
anidb_result_t anidb_init(uint32_t abi_version);
```

**Parameters:**
- `abi_version`: Expected ABI version (use `ANIDB_ABI_VERSION`)

**Returns:**
- `ANIDB_SUCCESS`: Initialization successful
- `ANIDB_ERROR_VERSION_MISMATCH`: ABI version mismatch

**Example:**
```c
if (anidb_init(ANIDB_ABI_VERSION) != ANIDB_SUCCESS) {
    fprintf(stderr, "Failed to initialize AniDB library\n");
    return 1;
}
```

### anidb_cleanup

Clean up the AniDB client library. Should be called when the library is no longer needed.

```c
void anidb_cleanup(void);
```

**Example:**
```c
// At program exit
anidb_cleanup();
```

### anidb_get_version

Get the library version string.

```c
const char* anidb_get_version(void);
```

**Returns:**
- Version string (e.g., "0.1.0-alpha"). Do not free this string.

### anidb_get_abi_version

Get the library ABI version number.

```c
uint32_t anidb_get_abi_version(void);
```

**Returns:**
- ABI version number

## Client Management

### anidb_client_create

Create a new AniDB client instance with default configuration.

```c
anidb_result_t anidb_client_create(anidb_client_handle_t* handle);
```

**Parameters:**
- `handle`: Output parameter for the client handle

**Returns:**
- `ANIDB_SUCCESS`: Client created successfully
- `ANIDB_ERROR_INVALID_PARAMETER`: Invalid handle pointer
- `ANIDB_ERROR_OUT_OF_MEMORY`: Memory allocation failed

**Example:**
```c
anidb_client_handle_t client;
anidb_result_t result = anidb_client_create(&client);
if (result != ANIDB_SUCCESS) {
    fprintf(stderr, "Failed to create client: %s\n", 
            anidb_error_string(result));
    return 1;
}
```

### anidb_client_create_with_config

Create a new AniDB client instance with custom configuration.

```c
anidb_result_t anidb_client_create_with_config(
    const anidb_config_t* config,
    anidb_client_handle_t* handle
);
```

**Parameters:**
- `config`: Client configuration structure
- `handle`: Output parameter for the client handle

**Configuration Structure:**
```c
typedef struct {
    const char* cache_dir;          // Cache directory path (UTF-8)
    size_t max_concurrent_files;    // Max concurrent operations (1-100)
    size_t chunk_size;              // Chunk size in bytes (1KB-10MB)
    size_t max_memory_usage;        // Max memory in bytes (0=default)
    int enable_debug_logging;       // Enable debug logs (0/1)
    const char* username;           // AniDB username (optional)
    const char* password;           // AniDB password (optional)
} anidb_config_t;
```

**Example:**
```c
anidb_config_t config = {
    .cache_dir = "/home/user/.anidb_cache",
    .max_concurrent_files = 4,
    .chunk_size = 65536,  // 64KB
    .max_memory_usage = 0,  // Use default
    .enable_debug_logging = 0,
    .username = NULL,
    .password = NULL
};

anidb_client_handle_t client;
anidb_result_t result = anidb_client_create_with_config(&config, &client);
```

### anidb_client_destroy

Destroy an AniDB client instance and release all associated resources.

```c
anidb_result_t anidb_client_destroy(anidb_client_handle_t handle);
```

**Parameters:**
- `handle`: Client handle to destroy

**Returns:**
- `ANIDB_SUCCESS`: Client destroyed successfully
- `ANIDB_ERROR_INVALID_HANDLE`: Invalid handle

**Example:**
```c
// Clean up client when done
anidb_client_destroy(client);
```

### anidb_client_get_last_error

Get the last error message for a client.

```c
anidb_result_t anidb_client_get_last_error(
    anidb_client_handle_t handle,
    char* buffer,
    size_t buffer_size
);
```

**Parameters:**
- `handle`: Client handle
- `buffer`: Buffer to store the error message
- `buffer_size`: Size of the buffer

**Returns:**
- `ANIDB_SUCCESS`: Error message retrieved
- `ANIDB_ERROR_INVALID_HANDLE`: Invalid handle
- `ANIDB_ERROR_INVALID_PARAMETER`: Invalid buffer

**Example:**
```c
char error_msg[256];
if (anidb_client_get_last_error(client, error_msg, sizeof(error_msg)) == ANIDB_SUCCESS) {
    printf("Last error: %s\n", error_msg);
}
```

## File Processing

### anidb_process_file

Process a single file synchronously, calculating specified hashes.

```c
anidb_result_t anidb_process_file(
    anidb_client_handle_t handle,
    const char* file_path,
    const anidb_process_options_t* options,
    anidb_file_result_t** result
);
```

**Parameters:**
- `handle`: Client handle
- `file_path`: Path to the file (UTF-8 encoded)
- `options`: Processing options
- `result`: Output parameter for the result (caller must free)

**Processing Options Structure:**
```c
typedef struct {
    const anidb_hash_algorithm_t* algorithms;  // Array of algorithms
    size_t algorithm_count;                    // Number of algorithms
    int enable_progress;                       // Enable progress (0/1)
    int verify_existing;                       // Verify cached hashes (0/1)
    anidb_progress_callback_t progress_callback; // Progress callback
    void* user_data;                          // User data for callback
} anidb_process_options_t;
```

**File Result Structure:**
```c
typedef struct {
    char* file_path;                // File path
    uint64_t file_size;             // File size in bytes
    anidb_status_t status;          // Processing status
    anidb_hash_result_t* hashes;    // Array of hash results
    size_t hash_count;              // Number of hashes
    uint64_t processing_time_ms;    // Processing time
    char* error_message;            // Error message (NULL if success)
} anidb_file_result_t;
```

**Example:**
```c
// Define algorithms to calculate
anidb_hash_algorithm_t algorithms[] = {
    ANIDB_HASH_ED2K,
    ANIDB_HASH_CRC32,
    ANIDB_HASH_MD5
};

// Set up processing options
anidb_process_options_t options = {
    .algorithms = algorithms,
    .algorithm_count = 3,
    .enable_progress = 1,
    .verify_existing = 0,
    .progress_callback = my_progress_callback,
    .user_data = NULL
};

// Process file
anidb_file_result_t* result;
anidb_result_t status = anidb_process_file(
    client, 
    "/path/to/video.mkv", 
    &options, 
    &result
);

if (status == ANIDB_SUCCESS) {
    printf("File: %s (%" PRIu64 " bytes)\n", result->file_path, result->file_size);
    for (size_t i = 0; i < result->hash_count; i++) {
        printf("%s: %s\n", 
            anidb_hash_algorithm_name(result->hashes[i].algorithm),
            result->hashes[i].hash_value);
    }
    anidb_free_file_result(result);
}
```

### Progress Callback

```c
void my_progress_callback(
    float percentage,
    uint64_t bytes_processed,
    uint64_t total_bytes,
    void* user_data
) {
    printf("\rProgress: %.1f%% (%" PRIu64 "/%" PRIu64 " bytes)", 
           percentage, bytes_processed, total_bytes);
    fflush(stdout);
}
```

## Batch Processing

### anidb_process_batch

Process multiple files in a batch synchronously.

```c
anidb_result_t anidb_process_batch(
    anidb_client_handle_t handle,
    const char** file_paths,
    size_t file_count,
    const anidb_batch_options_t* options,
    anidb_batch_result_t** result
);
```

**Parameters:**
- `handle`: Client handle
- `file_paths`: Array of file paths (UTF-8 encoded)
- `file_count`: Number of files
- `options`: Batch processing options
- `result`: Output parameter for the result (caller must free)

**Batch Options Structure:**
```c
typedef struct {
    const anidb_hash_algorithm_t* algorithms;  // Array of algorithms
    size_t algorithm_count;                    // Number of algorithms
    size_t max_concurrent;                     // Max concurrent operations
    int continue_on_error;                     // Continue on error (0/1)
    int skip_existing;                         // Skip cached files (0/1)
    anidb_progress_callback_t progress_callback;     // Progress callback
    anidb_completion_callback_t completion_callback; // Completion callback
    void* user_data;                          // User data for callbacks
} anidb_batch_options_t;
```

**Batch Result Structure:**
```c
typedef struct {
    size_t total_files;             // Total number of files
    size_t successful_files;        // Successfully processed files
    size_t failed_files;            // Failed files
    anidb_file_result_t* results;   // Array of individual results
    uint64_t total_time_ms;         // Total processing time
} anidb_batch_result_t;
```

**Example:**
```c
const char* files[] = {
    "/videos/episode01.mkv",
    "/videos/episode02.mkv",
    "/videos/episode03.mkv"
};

anidb_hash_algorithm_t algorithms[] = { ANIDB_HASH_ED2K };

anidb_batch_options_t options = {
    .algorithms = algorithms,
    .algorithm_count = 1,
    .max_concurrent = 2,
    .continue_on_error = 1,
    .skip_existing = 0,
    .progress_callback = batch_progress_callback,
    .completion_callback = batch_completion_callback,
    .user_data = NULL
};

anidb_batch_result_t* result;
anidb_result_t status = anidb_process_batch(
    client, files, 3, &options, &result
);

if (status == ANIDB_SUCCESS) {
    printf("Processed %zu/%zu files successfully\n", 
           result->successful_files, result->total_files);
    anidb_free_batch_result(result);
}
```

## Hash Calculation

### anidb_calculate_hash

Calculate a hash for a file without full processing.

```c
anidb_result_t anidb_calculate_hash(
    const char* file_path,
    anidb_hash_algorithm_t algorithm,
    char* hash_buffer,
    size_t buffer_size
);
```

**Parameters:**
- `file_path`: Path to the file (UTF-8 encoded)
- `algorithm`: Hash algorithm to use
- `hash_buffer`: Buffer to store the hash
- `buffer_size`: Size of the hash buffer

**Returns:**
- `ANIDB_SUCCESS`: Hash calculated successfully
- `ANIDB_ERROR_FILE_NOT_FOUND`: File not found
- `ANIDB_ERROR_INVALID_PARAMETER`: Invalid parameters

**Example:**
```c
char hash[65];  // Max hash size + null terminator
anidb_result_t result = anidb_calculate_hash(
    "/path/to/file.mkv",
    ANIDB_HASH_ED2K,
    hash,
    sizeof(hash)
);

if (result == ANIDB_SUCCESS) {
    printf("ED2K hash: %s\n", hash);
}
```

### anidb_calculate_hash_buffer

Calculate a hash for a memory buffer.

```c
anidb_result_t anidb_calculate_hash_buffer(
    const uint8_t* data,
    size_t data_size,
    anidb_hash_algorithm_t algorithm,
    char* hash_buffer,
    size_t buffer_size
);
```

**Parameters:**
- `data`: Data buffer
- `data_size`: Size of the data
- `algorithm`: Hash algorithm to use
- `hash_buffer`: Buffer to store the hash
- `buffer_size`: Size of the hash buffer

### anidb_hash_buffer_size

Get the required buffer size for a hash algorithm.

```c
size_t anidb_hash_buffer_size(anidb_hash_algorithm_t algorithm);
```

**Returns:**
- Required buffer size in bytes (includes null terminator)

**Example:**
```c
size_t required_size = anidb_hash_buffer_size(ANIDB_HASH_SHA1);
char* buffer = malloc(required_size);
```

## Cache Management

### anidb_cache_clear

Clear the hash cache.

```c
anidb_result_t anidb_cache_clear(anidb_client_handle_t handle);
```

### anidb_cache_get_stats

Get cache statistics.

```c
anidb_result_t anidb_cache_get_stats(
    anidb_client_handle_t handle,
    size_t* total_entries,
    uint64_t* cache_size_bytes
);
```

**Example:**
```c
size_t entries;
uint64_t size;
if (anidb_cache_get_stats(client, &entries, &size) == ANIDB_SUCCESS) {
    printf("Cache: %zu entries, %" PRIu64 " bytes\n", entries, size);
}
```

### anidb_cache_check_file

Check if a file hash is in the cache.

```c
anidb_result_t anidb_cache_check_file(
    anidb_client_handle_t handle,
    const char* file_path,
    anidb_hash_algorithm_t algorithm,
    int* is_cached
);
```

**Example:**
```c
int is_cached;
if (anidb_cache_check_file(client, "/path/to/file.mkv", 
                           ANIDB_HASH_ED2K, &is_cached) == ANIDB_SUCCESS) {
    printf("File is %s\n", is_cached ? "cached" : "not cached");
}
```

## Anime Identification

### anidb_identify_file

Identify an anime file by its ED2K hash and size.

```c
anidb_result_t anidb_identify_file(
    anidb_client_handle_t handle,
    const char* ed2k_hash,
    uint64_t file_size,
    anidb_anime_info_t** info
);
```

**Parameters:**
- `handle`: Client handle
- `ed2k_hash`: ED2K hash of the file
- `file_size`: File size in bytes
- `info`: Output parameter for anime info (caller must free)

**Anime Info Structure:**
```c
typedef struct {
    uint64_t anime_id;      // AniDB anime ID
    uint64_t episode_id;    // AniDB episode ID
    char* title;            // Anime title
    uint32_t episode_number; // Episode number
    double confidence;      // Confidence score (0.0-1.0)
    int source;            // Source: 0=AniDB, 1=Cache, 2=Filename
} anidb_anime_info_t;
```

**Example:**
```c
anidb_anime_info_t* info;
anidb_result_t result = anidb_identify_file(
    client,
    "a1b2c3d4e5f6...",  // ED2K hash
    1234567890,         // File size
    &info
);

if (result == ANIDB_SUCCESS) {
    printf("Anime: %s (Episode %u)\n", info->title, info->episode_number);
    printf("Confidence: %.2f\n", info->confidence);
    anidb_free_anime_info(info);
}
```

## Memory Management

All dynamically allocated memory returned by the library must be freed using the appropriate free function.

### anidb_free_string

Free a string allocated by the library.

```c
void anidb_free_string(char* str);
```

### anidb_free_file_result

Free a file result structure.

```c
void anidb_free_file_result(anidb_file_result_t* result);
```

### anidb_free_batch_result

Free a batch result structure.

```c
void anidb_free_batch_result(anidb_batch_result_t* result);
```

### anidb_free_anime_info

Free an anime info structure.

```c
void anidb_free_anime_info(anidb_anime_info_t* info);
```

## Callback Management

### anidb_register_callback

Register a callback with the client.

```c
uint64_t anidb_register_callback(
    anidb_client_handle_t handle,
    anidb_callback_type_t type,
    void* callback,
    void* user_data
);
```

**Parameters:**
- `handle`: Client handle
- `type`: Type of callback to register
- `callback`: Callback function pointer
- `user_data`: User data to pass to callback

**Returns:**
- Callback ID for unregistration, or 0 on error

**Callback Types:**
- `ANIDB_CALLBACK_PROGRESS`: Progress updates
- `ANIDB_CALLBACK_ERROR`: Error notifications
- `ANIDB_CALLBACK_COMPLETION`: Operation completion
- `ANIDB_CALLBACK_EVENT`: General events

**Example:**
```c
// Register error callback
uint64_t error_cb_id = anidb_register_callback(
    client,
    ANIDB_CALLBACK_ERROR,
    (void*)my_error_callback,
    NULL
);

// Error callback function
void my_error_callback(
    anidb_result_t error_code,
    const char* error_message,
    const char* file_path,
    void* user_data
) {
    fprintf(stderr, "Error %d: %s (file: %s)\n", 
            error_code, error_message, 
            file_path ? file_path : "N/A");
}
```

### anidb_unregister_callback

Unregister a callback.

```c
anidb_result_t anidb_unregister_callback(
    anidb_client_handle_t handle,
    uint64_t callback_id
);
```

## Event System

### anidb_event_connect

Connect to the event system for receiving detailed events.

```c
anidb_result_t anidb_event_connect(
    anidb_client_handle_t handle,
    anidb_event_callback_t callback,
    void* user_data
);
```

**Event Callback Function:**
```c
void my_event_callback(
    const anidb_event_t* event,
    void* user_data
) {
    switch (event->type) {
        case ANIDB_EVENT_FILE_START:
            printf("Processing file: %s\n", event->data.file.file_path);
            break;
        case ANIDB_EVENT_HASH_COMPLETE:
            printf("Hash complete: %s = %s\n",
                   anidb_hash_algorithm_name(event->data.hash.algorithm),
                   event->data.hash.hash_value);
            break;
        // Handle other events...
    }
}
```

### anidb_event_disconnect

Disconnect from the event system.

```c
anidb_result_t anidb_event_disconnect(anidb_client_handle_t handle);
```

### anidb_event_poll

Poll for events without a callback.

```c
anidb_result_t anidb_event_poll(
    anidb_client_handle_t handle,
    anidb_event_t* events,
    size_t max_events,
    size_t* event_count
);
```

**Example:**
```c
anidb_event_t events[10];
size_t count;

if (anidb_event_poll(client, events, 10, &count) == ANIDB_SUCCESS) {
    for (size_t i = 0; i < count; i++) {
        // Process event
        process_event(&events[i]);
    }
}
```

## Memory Statistics

### anidb_get_memory_stats

Get detailed memory usage statistics.

```c
typedef struct {
    uint64_t total_memory_used;   // Total memory used
    uint64_t ffi_allocated;       // FFI allocated memory
    uint64_t ffi_peak;           // Peak FFI memory
    uint64_t pool_memory;        // Buffer pool memory
    uint64_t pool_hits;          // Buffer pool hits
    uint64_t pool_misses;        // Buffer pool misses
    uint64_t active_allocations;  // Active allocations
    uint64_t memory_limit;       // Memory limit
    uint32_t memory_pressure;    // 0=Low, 1=Medium, 2=High, 3=Critical
} anidb_memory_stats_t;

anidb_result_t anidb_get_memory_stats(anidb_memory_stats_t* stats);
```

**Example:**
```c
anidb_memory_stats_t stats;
if (anidb_get_memory_stats(&stats) == ANIDB_SUCCESS) {
    printf("Memory used: %" PRIu64 " bytes\n", stats.total_memory_used);
    printf("Memory pressure: %s\n", 
           stats.memory_pressure == 0 ? "Low" :
           stats.memory_pressure == 1 ? "Medium" :
           stats.memory_pressure == 2 ? "High" : "Critical");
}
```

### anidb_memory_gc

Force garbage collection of unused buffers.

```c
anidb_result_t anidb_memory_gc(void);
```

### anidb_check_memory_leaks

Check for memory leaks (debug builds only).

```c
anidb_result_t anidb_check_memory_leaks(
    uint64_t* leak_count,
    uint64_t* total_leaked_bytes
);
```

## Error Handling

### Error Codes

```c
typedef enum {
    ANIDB_SUCCESS = 0,
    ANIDB_ERROR_INVALID_HANDLE = 1,
    ANIDB_ERROR_INVALID_PARAMETER = 2,
    ANIDB_ERROR_FILE_NOT_FOUND = 3,
    ANIDB_ERROR_PROCESSING = 4,
    ANIDB_ERROR_OUT_OF_MEMORY = 5,
    ANIDB_ERROR_IO = 6,
    ANIDB_ERROR_NETWORK = 7,
    ANIDB_ERROR_CANCELLED = 8,
    ANIDB_ERROR_INVALID_UTF8 = 9,
    ANIDB_ERROR_VERSION_MISMATCH = 10,
    ANIDB_ERROR_TIMEOUT = 11,
    ANIDB_ERROR_PERMISSION_DENIED = 12,
    ANIDB_ERROR_CACHE = 13,
    ANIDB_ERROR_BUSY = 14,
    ANIDB_ERROR_UNKNOWN = 99
} anidb_result_t;
```

### anidb_error_string

Get a human-readable error description.

```c
const char* anidb_error_string(anidb_result_t error);
```

**Example:**
```c
anidb_result_t result = anidb_process_file(client, path, &options, &file_result);
if (result != ANIDB_SUCCESS) {
    fprintf(stderr, "Error: %s\n", anidb_error_string(result));
}
```

## Constants and Enumerations

### Hash Algorithms

```c
typedef enum {
    ANIDB_HASH_ED2K = 1,   // ED2K hash (default for AniDB)
    ANIDB_HASH_CRC32 = 2,  // CRC32 checksum
    ANIDB_HASH_MD5 = 3,    // MD5 hash
    ANIDB_HASH_SHA1 = 4,   // SHA1 hash
    ANIDB_HASH_TTH = 5     // Tiger Tree Hash
} anidb_hash_algorithm_t;
```

### Processing Status

```c
typedef enum {
    ANIDB_STATUS_PENDING = 0,     // Processing pending
    ANIDB_STATUS_PROCESSING = 1,  // Currently processing
    ANIDB_STATUS_COMPLETED = 2,   // Processing completed
    ANIDB_STATUS_FAILED = 3,      // Processing failed
    ANIDB_STATUS_CANCELLED = 4    // Processing cancelled
} anidb_status_t;
```

### Event Types

```c
typedef enum {
    ANIDB_EVENT_FILE_START = 1,        // File processing started
    ANIDB_EVENT_FILE_COMPLETE = 2,     // File processing completed
    ANIDB_EVENT_HASH_START = 3,        // Hash calculation started
    ANIDB_EVENT_HASH_COMPLETE = 4,     // Hash calculation completed
    ANIDB_EVENT_CACHE_HIT = 5,         // Cache hit occurred
    ANIDB_EVENT_CACHE_MISS = 6,        // Cache miss occurred
    ANIDB_EVENT_NETWORK_START = 7,     // Network request started
    ANIDB_EVENT_NETWORK_COMPLETE = 8,  // Network request completed
    ANIDB_EVENT_MEMORY_WARNING = 9     // Memory threshold reached
} anidb_event_type_t;
```

### anidb_hash_algorithm_name

Get the name of a hash algorithm.

```c
const char* anidb_hash_algorithm_name(anidb_hash_algorithm_t algorithm);
```

**Example:**
```c
printf("Using algorithm: %s\n", anidb_hash_algorithm_name(ANIDB_HASH_ED2K));
// Output: "Using algorithm: ED2K"
```