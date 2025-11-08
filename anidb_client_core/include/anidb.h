/**
 * @file anidb.h
 * @brief AniDB Client Core Library C API
 * 
 * This header provides a C-compatible interface to the AniDB Client Core Library.
 * All functions use an opaque handle pattern for safety and follow consistent
 * error handling conventions.
 * 
 * @version 0.1.0-alpha
 * @date 2025-01-31
 */

#ifndef ANIDB_CLIENT_H
#define ANIDB_CLIENT_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>
#include <stdint.h>

/* ========================================================================== */
/*                           Version and Compatibility                         */
/* ========================================================================== */

/** Major version number */
#define ANIDB_VERSION_MAJOR 0

/** Minor version number */
#define ANIDB_VERSION_MINOR 1

/** Patch version number */
#define ANIDB_VERSION_PATCH 0

/** Full version string */
#define ANIDB_VERSION_STRING "0.1.0-alpha"

/** ABI version for compatibility checking */
#define ANIDB_ABI_VERSION 1

/* ========================================================================== */
/*                              Type Definitions                               */
/* ========================================================================== */

/**
 * @brief Opaque handle to an AniDB client instance
 * 
 * This handle represents a client instance and must be created with
 * anidb_client_create() and destroyed with anidb_client_destroy().
 */
typedef struct anidb_client_t* anidb_client_handle_t;

/**
 * @brief Opaque handle to a file processing operation
 * 
 * This handle represents an ongoing file processing operation and can be
 * used to query progress or cancel the operation.
 */
typedef struct anidb_operation_t* anidb_operation_handle_t;

/**
 * @brief Opaque handle to a batch processing operation
 * 
 * This handle represents a batch of file processing operations.
 */
typedef struct anidb_batch_t* anidb_batch_handle_t;

/**
 * @brief Result codes for API operations
 */
typedef enum {
    /** Operation completed successfully */
    ANIDB_SUCCESS = 0,
    
    /** Invalid handle provided */
    ANIDB_ERROR_INVALID_HANDLE = 1,
    
    /** Invalid parameter provided */
    ANIDB_ERROR_INVALID_PARAMETER = 2,
    
    /** File not found */
    ANIDB_ERROR_FILE_NOT_FOUND = 3,
    
    /** Error during processing */
    ANIDB_ERROR_PROCESSING = 4,
    
    /** Out of memory */
    ANIDB_ERROR_OUT_OF_MEMORY = 5,
    
    /** I/O error */
    ANIDB_ERROR_IO = 6,
    
    /** Network error */
    ANIDB_ERROR_NETWORK = 7,
    
    /** Operation cancelled */
    ANIDB_ERROR_CANCELLED = 8,
    
    /** Invalid UTF-8 string */
    ANIDB_ERROR_INVALID_UTF8 = 9,
    
    /** Version mismatch */
    ANIDB_ERROR_VERSION_MISMATCH = 10,
    
    /** Operation timeout */
    ANIDB_ERROR_TIMEOUT = 11,
    
    /** Permission denied */
    ANIDB_ERROR_PERMISSION_DENIED = 12,
    
    /** Cache error */
    ANIDB_ERROR_CACHE = 13,
    
    /** Resource busy */
    ANIDB_ERROR_BUSY = 14,
    
    /** Unknown error */
    ANIDB_ERROR_UNKNOWN = 99
} anidb_result_t;

/**
 * @brief Hash algorithm identifiers
 */
typedef enum {
    /** ED2K hash algorithm (default for AniDB) */
    ANIDB_HASH_ED2K = 1,
    
    /** CRC32 checksum */
    ANIDB_HASH_CRC32 = 2,
    
    /** MD5 hash */
    ANIDB_HASH_MD5 = 3,
    
    /** SHA1 hash */
    ANIDB_HASH_SHA1 = 4,
    
    /** Tiger Tree Hash */
    ANIDB_HASH_TTH = 5
} anidb_hash_algorithm_t;

/**
 * @brief Processing status codes
 */
typedef enum {
    /** Processing pending */
    ANIDB_STATUS_PENDING = 0,
    
    /** Currently processing */
    ANIDB_STATUS_PROCESSING = 1,
    
    /** Processing completed */
    ANIDB_STATUS_COMPLETED = 2,
    
    /** Processing failed */
    ANIDB_STATUS_FAILED = 3,
    
    /** Processing cancelled */
    ANIDB_STATUS_CANCELLED = 4
} anidb_status_t;

/* ========================================================================== */
/*                            Callback Definitions                             */
/* ========================================================================== */

/**
 * @brief Callback types that can be registered
 */
typedef enum {
    /** Progress update callback */
    ANIDB_CALLBACK_PROGRESS = 1,
    
    /** Error notification callback */
    ANIDB_CALLBACK_ERROR = 2,
    
    /** Operation completion callback */
    ANIDB_CALLBACK_COMPLETION = 3,
    
    /** General event callback */
    ANIDB_CALLBACK_EVENT = 4
} anidb_callback_type_t;

/**
 * @brief Event types for the event callback system
 */
typedef enum {
    /** File processing started */
    ANIDB_EVENT_FILE_START = 1,
    
    /** File processing completed */
    ANIDB_EVENT_FILE_COMPLETE = 2,
    
    /** Hash calculation started */
    ANIDB_EVENT_HASH_START = 3,
    
    /** Hash calculation completed */
    ANIDB_EVENT_HASH_COMPLETE = 4,
    
    /** Cache hit occurred */
    ANIDB_EVENT_CACHE_HIT = 5,
    
    /** Cache miss occurred */
    ANIDB_EVENT_CACHE_MISS = 6,
    
    /** Network request started */
    ANIDB_EVENT_NETWORK_START = 7,
    
    /** Network request completed */
    ANIDB_EVENT_NETWORK_COMPLETE = 8,
    
    /** Memory threshold reached */
    ANIDB_EVENT_MEMORY_WARNING = 9
} anidb_event_type_t;

/**
 * @brief Progress callback function type
 * 
 * @param percentage Progress percentage (0.0 to 100.0)
 * @param bytes_processed Number of bytes processed so far
 * @param total_bytes Total number of bytes to process
 * @param user_data User-provided data pointer
 */
typedef void (*anidb_progress_callback_t)(
    float percentage,
    uint64_t bytes_processed,
    uint64_t total_bytes,
    void* user_data
);

/**
 * @brief Error callback function type
 * 
 * @param error_code Error code that occurred
 * @param error_message Human-readable error message
 * @param file_path File path related to the error (may be NULL)
 * @param user_data User-provided data pointer
 */
typedef void (*anidb_error_callback_t)(
    anidb_result_t error_code,
    const char* error_message,
    const char* file_path,
    void* user_data
);

/**
 * @brief Completion callback function type
 * 
 * @param result Result code of the operation
 * @param user_data User-provided data pointer
 */
typedef void (*anidb_completion_callback_t)(
    anidb_result_t result,
    void* user_data
);

/**
 * @brief Event data union for different event types
 */
typedef union {
    /** Data for file events */
    struct {
        const char* file_path;
        uint64_t file_size;
    } file;
    
    /** Data for hash events */
    struct {
        anidb_hash_algorithm_t algorithm;
        const char* hash_value;
    } hash;
    
    /** Data for cache events */
    struct {
        const char* file_path;
        anidb_hash_algorithm_t algorithm;
    } cache;
    
    /** Data for network events */
    struct {
        const char* endpoint;
        int status_code;
    } network;
    
    /** Data for memory events */
    struct {
        uint64_t current_usage;
        uint64_t max_usage;
    } memory;
} anidb_event_data_t;

/**
 * @brief Event structure for event callbacks
 */
typedef struct {
    /** Type of event */
    anidb_event_type_t type;
    
    /** Timestamp when event occurred (milliseconds since epoch) */
    uint64_t timestamp;
    
    /** Event-specific data */
    anidb_event_data_t data;
    
    /** Additional context string (may be NULL) */
    const char* context;
} anidb_event_t;

/**
 * @brief Event callback function type
 * 
 * @param event Event information
 * @param user_data User-provided data pointer
 */
typedef void (*anidb_event_callback_t)(
    const anidb_event_t* event,
    void* user_data
);

/* ========================================================================== */
/*                            Structure Definitions                            */
/* ========================================================================== */

/**
 * @brief Client configuration structure
 */
typedef struct {
    /** Cache directory path (UTF-8 encoded) */
    const char* cache_dir;
    
    /** Maximum concurrent file operations */
    size_t max_concurrent_files;
    
    /** Chunk size for file processing in bytes */
    size_t chunk_size;
    
    /** Maximum memory usage in bytes (0 for default) */
    size_t max_memory_usage;
    
    /** Enable debug logging */
    int enable_debug_logging;
    
    /** AniDB username (optional) */
    const char* username;
    
    /** AniDB password (optional) */
    const char* password;
    
    /** AniDB client name (optional) */
    const char* client_name;
    
    /** AniDB client version (optional) */
    const char* client_version;
} anidb_config_t;

/**
 * @brief File processing options
 */
typedef struct {
    /** Array of hash algorithms to calculate */
    const anidb_hash_algorithm_t* algorithms;
    
    /** Number of algorithms in the array */
    size_t algorithm_count;
    
    /** Enable progress reporting */
    int enable_progress;
    
    /** Verify existing hashes in cache */
    int verify_existing;
    
    /** Progress callback (optional) */
    anidb_progress_callback_t progress_callback;
    
    /** User data for callbacks */
    void* user_data;
} anidb_process_options_t;

/**
 * @brief Hash result structure
 */
typedef struct {
    /** Hash algorithm used */
    anidb_hash_algorithm_t algorithm;
    
    /** Hash value as hexadecimal string */
    char* hash_value;
    
    /** Length of the hash string */
    size_t hash_length;
} anidb_hash_result_t;

/**
 * @brief File processing result
 */
typedef struct {
    /** File path */
    char* file_path;
    
    /** File size in bytes */
    uint64_t file_size;
    
    /** Processing status */
    anidb_status_t status;
    
    /** Array of hash results */
    anidb_hash_result_t* hashes;
    
    /** Number of hash results */
    size_t hash_count;
    
    /** Processing time in milliseconds */
    uint64_t processing_time_ms;
    
    /** Error message (NULL if no error) */
    char* error_message;
} anidb_file_result_t;

/**
 * @brief Anime identification information
 */
typedef struct {
    /** AniDB anime ID */
    uint64_t anime_id;
    
    /** AniDB episode ID */
    uint64_t episode_id;
    
    /** Anime title */
    char* title;
    
    /** Episode number */
    uint32_t episode_number;
    
    /** Confidence score (0.0 to 1.0) */
    double confidence;
    
    /** Source of identification (0=AniDB, 1=Cache, 2=Filename) */
    int source;
} anidb_anime_info_t;

/**
 * @brief Batch processing options
 */
typedef struct {
    /** Array of hash algorithms to calculate */
    const anidb_hash_algorithm_t* algorithms;
    
    /** Number of algorithms in the array */
    size_t algorithm_count;
    
    /** Maximum concurrent operations */
    size_t max_concurrent;
    
    /** Continue processing on error */
    int continue_on_error;
    
    /** Skip files already in cache */
    int skip_existing;
    
    /** Progress callback (optional) */
    anidb_progress_callback_t progress_callback;
    
    /** Completion callback (optional) */
    anidb_completion_callback_t completion_callback;
    
    /** User data for callbacks */
    void* user_data;
} anidb_batch_options_t;

/**
 * @brief Batch processing result
 */
typedef struct {
    /** Total number of files */
    size_t total_files;
    
    /** Number of successfully processed files */
    size_t successful_files;
    
    /** Number of failed files */
    size_t failed_files;
    
    /** Array of individual file results */
    anidb_file_result_t* results;
    
    /** Total processing time in milliseconds */
    uint64_t total_time_ms;
} anidb_batch_result_t;

/* ========================================================================== */
/*                          Library Initialization                             */
/* ========================================================================== */

/**
 * @brief Initialize the AniDB client library
 * 
 * This function must be called before any other library functions.
 * It initializes global state and checks version compatibility.
 * 
 * @param abi_version Expected ABI version (use ANIDB_ABI_VERSION)
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_init(uint32_t abi_version);

/**
 * @brief Cleanup the AniDB client library
 * 
 * This function should be called when the library is no longer needed.
 * It will clean up all global state and release resources.
 */
void anidb_cleanup(void);

/**
 * @brief Get library version string
 * 
 * @return Version string (do not free)
 */
const char* anidb_get_version(void);

/**
 * @brief Get library ABI version
 * 
 * @return ABI version number
 */
uint32_t anidb_get_abi_version(void);

/* ========================================================================== */
/*                           Client Management                                 */
/* ========================================================================== */

/**
 * @brief Create a new AniDB client instance with default configuration
 * 
 * @param handle Output parameter for the client handle
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_client_create(anidb_client_handle_t* handle);

/**
 * @brief Create a new AniDB client instance with custom configuration
 * 
 * @param config Client configuration (must not be NULL)
 * @param handle Output parameter for the client handle
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_client_create_with_config(
    const anidb_config_t* config,
    anidb_client_handle_t* handle
);

/**
 * @brief Destroy an AniDB client instance
 * 
 * This function releases all resources associated with the client.
 * The handle becomes invalid after this call.
 * 
 * @param handle Client handle to destroy
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_client_destroy(anidb_client_handle_t handle);

/**
 * @brief Get the last error message for a client
 * 
 * @param handle Client handle
 * @param buffer Buffer to store the error message
 * @param buffer_size Size of the buffer
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_client_get_last_error(
    anidb_client_handle_t handle,
    char* buffer,
    size_t buffer_size
);

/* ========================================================================== */
/*                           File Processing                                   */
/* ========================================================================== */

/**
 * @brief Process a single file synchronously
 * 
 * This function blocks until the file processing is complete.
 * 
 * @param handle Client handle
 * @param file_path Path to the file (UTF-8 encoded)
 * @param options Processing options
 * @param result Output parameter for the result (caller must free)
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_process_file(
    anidb_client_handle_t handle,
    const char* file_path,
    const anidb_process_options_t* options,
    anidb_file_result_t** result
);

/**
 * @brief Process a single file asynchronously
 * 
 * This function returns immediately with an operation handle.
 * 
 * @param handle Client handle
 * @param file_path Path to the file (UTF-8 encoded)
 * @param options Processing options
 * @param operation Output parameter for the operation handle
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_process_file_async(
    anidb_client_handle_t handle,
    const char* file_path,
    const anidb_process_options_t* options,
    anidb_operation_handle_t* operation
);

/**
 * @brief Get the status of an async operation
 * 
 * @param operation Operation handle
 * @param status Output parameter for the status
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_operation_get_status(
    anidb_operation_handle_t operation,
    anidb_status_t* status
);

/**
 * @brief Get the result of a completed async operation
 * 
 * @param operation Operation handle
 * @param result Output parameter for the result (caller must free)
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_operation_get_result(
    anidb_operation_handle_t operation,
    anidb_file_result_t** result
);

/**
 * @brief Cancel an async operation
 * 
 * @param operation Operation handle
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_operation_cancel(anidb_operation_handle_t operation);

/**
 * @brief Destroy an operation handle
 * 
 * @param operation Operation handle to destroy
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_operation_destroy(anidb_operation_handle_t operation);

/* ========================================================================== */
/*                           Batch Processing                                  */
/* ========================================================================== */

/**
 * @brief Process multiple files in a batch
 * 
 * @param handle Client handle
 * @param file_paths Array of file paths (UTF-8 encoded)
 * @param file_count Number of files
 * @param options Batch processing options
 * @param result Output parameter for the result (caller must free)
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_process_batch(
    anidb_client_handle_t handle,
    const char** file_paths,
    size_t file_count,
    const anidb_batch_options_t* options,
    anidb_batch_result_t** result
);

/**
 * @brief Process multiple files in a batch asynchronously
 * 
 * @param handle Client handle
 * @param file_paths Array of file paths (UTF-8 encoded)
 * @param file_count Number of files
 * @param options Batch processing options
 * @param batch Output parameter for the batch handle
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_process_batch_async(
    anidb_client_handle_t handle,
    const char** file_paths,
    size_t file_count,
    const anidb_batch_options_t* options,
    anidb_batch_handle_t* batch
);

/**
 * @brief Get the progress of a batch operation
 * 
 * @param batch Batch handle
 * @param completed Output parameter for completed files
 * @param total Output parameter for total files
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_batch_get_progress(
    anidb_batch_handle_t batch,
    size_t* completed,
    size_t* total
);

/**
 * @brief Cancel a batch operation
 * 
 * @param batch Batch handle
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_batch_cancel(anidb_batch_handle_t batch);

/**
 * @brief Destroy a batch handle
 * 
 * @param batch Batch handle to destroy
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_batch_destroy(anidb_batch_handle_t batch);

/* ========================================================================== */
/*                           Hash Calculation                                  */
/* ========================================================================== */

/**
 * @brief Calculate hash for a file
 * 
 * This is a convenience function for calculating a single hash without
 * full file processing.
 * 
 * @param file_path Path to the file (UTF-8 encoded)
 * @param algorithm Hash algorithm to use
 * @param hash_buffer Buffer to store the hash (must be large enough)
 * @param buffer_size Size of the hash buffer
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_calculate_hash(
    const char* file_path,
    anidb_hash_algorithm_t algorithm,
    char* hash_buffer,
    size_t buffer_size
);

/**
 * @brief Calculate hash for memory buffer
 * 
 * @param data Data buffer
 * @param data_size Size of the data
 * @param algorithm Hash algorithm to use
 * @param hash_buffer Buffer to store the hash (must be large enough)
 * @param buffer_size Size of the hash buffer
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_calculate_hash_buffer(
    const uint8_t* data,
    size_t data_size,
    anidb_hash_algorithm_t algorithm,
    char* hash_buffer,
    size_t buffer_size
);

/* ========================================================================== */
/*                           Cache Management                                  */
/* ========================================================================== */

/**
 * @brief Clear the hash cache
 * 
 * @param handle Client handle
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_cache_clear(anidb_client_handle_t handle);

/**
 * @brief Get cache statistics
 * 
 * @param handle Client handle
 * @param total_entries Output parameter for total cache entries
 * @param cache_size_bytes Output parameter for cache size in bytes
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_cache_get_stats(
    anidb_client_handle_t handle,
    size_t* total_entries,
    uint64_t* cache_size_bytes
);

/**
 * @brief Check if a file hash is in cache
 * 
 * @param handle Client handle
 * @param file_path Path to the file (UTF-8 encoded)
 * @param algorithm Hash algorithm
 * @param is_cached Output parameter (1 if cached, 0 if not)
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_cache_check_file(
    anidb_client_handle_t handle,
    const char* file_path,
    anidb_hash_algorithm_t algorithm,
    int* is_cached
);

/* ========================================================================== */
/*                         Anime Identification                                */
/* ========================================================================== */

/**
 * @brief Identify an anime file by hash and size
 * 
 * @param handle Client handle
 * @param ed2k_hash ED2K hash of the file
 * @param file_size File size in bytes
 * @param info Output parameter for anime info (caller must free)
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_identify_file(
    anidb_client_handle_t handle,
    const char* ed2k_hash,
    uint64_t file_size,
    anidb_anime_info_t** info
);

/* ========================================================================== */
/*                           Memory Management                                 */
/* ========================================================================== */

/**
 * @brief Free a string allocated by the library
 * 
 * @param str String to free
 */
void anidb_free_string(char* str);

/**
 * @brief Free a file result structure
 * 
 * @param result Result structure to free
 */
void anidb_free_file_result(anidb_file_result_t* result);

/**
 * @brief Free a batch result structure
 * 
 * @param result Result structure to free
 */
void anidb_free_batch_result(anidb_batch_result_t* result);

/**
 * @brief Free an anime info structure
 * 
 * @param info Info structure to free
 */
void anidb_free_anime_info(anidb_anime_info_t* info);

/* ========================================================================== */
/*                          Callback Management                                */
/* ========================================================================== */

/**
 * @brief Register a callback with the client
 * 
 * Callbacks are executed on a dedicated thread to ensure thread safety.
 * Multiple callbacks of the same type can be registered.
 * 
 * @param handle Client handle
 * @param type Type of callback to register
 * @param callback Callback function pointer
 * @param user_data User data to pass to callback
 * @return Callback ID for unregistration, or 0 on error
 */
uint64_t anidb_register_callback(
    anidb_client_handle_t handle,
    anidb_callback_type_t type,
    void* callback,
    void* user_data
);

/**
 * @brief Unregister a callback
 * 
 * @param handle Client handle
 * @param callback_id ID returned by anidb_register_callback
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_unregister_callback(
    anidb_client_handle_t handle,
    uint64_t callback_id
);

/**
 * @brief Connect to the event system for receiving events
 * 
 * Only one event callback can be connected at a time per client.
 * Events are queued internally and delivered via callback or polling.
 * 
 * @param handle Client handle
 * @param callback Event callback function
 * @param user_data User data to pass to callback
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_event_connect(
    anidb_client_handle_t handle,
    anidb_event_callback_t callback,
    void* user_data
);

/**
 * @brief Disconnect from the event system
 * 
 * @param handle Client handle
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_event_disconnect(anidb_client_handle_t handle);

/**
 * @brief Poll for events without callback
 * 
 * This function retrieves queued events for manual processing.
 * Events are removed from the queue after retrieval.
 * 
 * @param handle Client handle
 * @param events Array to store events
 * @param max_events Maximum number of events to retrieve
 * @param event_count Output parameter for actual events retrieved
 * @return ANIDB_SUCCESS on success, error code otherwise
 */
anidb_result_t anidb_event_poll(
    anidb_client_handle_t handle,
    anidb_event_t* events,
    size_t max_events,
    size_t* event_count
);

/* ========================================================================== */
/*                           Utility Functions                                 */
/* ========================================================================== */

/**
 * @brief Get human-readable error description
 * 
 * @param error Error code
 * @return Error description string (do not free)
 */
const char* anidb_error_string(anidb_result_t error);

/**
 * @brief Get hash algorithm name
 * 
 * @param algorithm Algorithm identifier
 * @return Algorithm name string (do not free)
 */
const char* anidb_hash_algorithm_name(anidb_hash_algorithm_t algorithm);

/**
 * @brief Get required hash buffer size
 * 
 * @param algorithm Hash algorithm
 * @return Required buffer size in bytes (includes null terminator)
 */
size_t anidb_hash_buffer_size(anidb_hash_algorithm_t algorithm);

#ifdef __cplusplus
}
#endif

#endif /* ANIDB_CLIENT_H */