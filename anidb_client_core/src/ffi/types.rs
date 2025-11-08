//! C-compatible type definitions for FFI
//!
//! This module contains all the C-compatible types used in the FFI layer,
//! including enums, structs, and type aliases for callbacks.

use std::ffi::c_char;
use std::ptr;

/* ========================================================================== */
/*                              Type Definitions                               */
/* ========================================================================== */

/// FFI result codes matching the C header
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AniDBResult {
    Success = 0,
    ErrorInvalidHandle = 1,
    ErrorInvalidParameter = 2,
    ErrorFileNotFound = 3,
    ErrorProcessing = 4,
    ErrorOutOfMemory = 5,
    ErrorIo = 6,
    ErrorNetwork = 7,
    ErrorCancelled = 8,
    ErrorInvalidUtf8 = 9,
    ErrorVersionMismatch = 10,
    ErrorTimeout = 11,
    ErrorPermissionDenied = 12,
    ErrorBusy = 13,
    ErrorUnknown = 99,
}

/// Hash algorithm identifiers matching the C header
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AniDBHashAlgorithm {
    ED2K = 1,
    CRC32 = 2,
    MD5 = 3,
    SHA1 = 4,
    TTH = 5,
}

/// Processing status codes
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AniDBStatus {
    Pending = 0,
    Processing = 1,
    Completed = 2,
    Failed = 3,
    Cancelled = 4,
}

/// Callback types that can be registered
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AniDBCallbackType {
    Progress = 1,
    Error = 2,
    Completion = 3,
    Event = 4,
}

/// Event types for the event callback system
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AniDBEventType {
    FileStart = 1,
    FileComplete = 2,
    HashStart = 3,
    HashComplete = 4,
    // CacheHit = 5,      // Deprecated: Cache moved to CLI layer
    // CacheMiss = 6,     // Deprecated: Cache moved to CLI layer
    NetworkStart = 7,
    NetworkComplete = 8,
    MemoryWarning = 9,
}

/* ========================================================================== */
/*                            Structure Definitions                            */
/* ========================================================================== */

/// Client configuration structure matching C header
#[repr(C)]
pub struct AniDBConfig {
    pub max_concurrent_files: usize,
    pub chunk_size: usize,
    pub max_memory_usage: usize,
    pub enable_debug_logging: i32,
    pub username: *const c_char,
    pub password: *const c_char,
    pub client_name: *const c_char,
    pub client_version: *const c_char,
}

/// File processing options matching C header
#[repr(C)]
pub struct AniDBProcessOptions {
    pub algorithms: *const AniDBHashAlgorithm,
    pub algorithm_count: usize,
    pub enable_progress: i32,
    pub progress_callback: Option<extern "C" fn(f32, u64, u64, *mut std::ffi::c_void)>,
    pub user_data: *mut std::ffi::c_void,
}

/// Hash result structure
#[repr(C)]
pub struct AniDBHashResult {
    pub algorithm: AniDBHashAlgorithm,
    pub hash_value: *mut c_char,
    pub hash_length: usize,
}

/// File processing result
#[repr(C)]
pub struct AniDBFileResult {
    pub file_path: *mut c_char,
    pub file_size: u64,
    pub status: AniDBStatus,
    pub hashes: *mut AniDBHashResult,
    pub hash_count: usize,
    pub processing_time_ms: u64,
    pub error_message: *mut c_char,
}

/// Anime identification information
#[repr(C)]
pub struct AniDBAnimeInfo {
    pub anime_id: u64,
    pub episode_id: u64,
    pub title: *mut c_char,
    pub episode_number: u32,
    pub source: i32,
}

/// Batch processing options
#[repr(C)]
pub struct AniDBBatchOptions {
    pub algorithms: *const AniDBHashAlgorithm,
    pub algorithm_count: usize,
    pub max_concurrent: usize,
    pub continue_on_error: i32,
    pub skip_existing: i32,
    pub include_patterns: *const *const c_char,
    pub include_pattern_count: usize,
    pub exclude_patterns: *const *const c_char,
    pub exclude_pattern_count: usize,
    pub use_defaults: i32,
    pub progress_callback: Option<extern "C" fn(f32, u64, u64, *mut std::ffi::c_void)>,
    pub completion_callback: Option<extern "C" fn(AniDBResult, *mut std::ffi::c_void)>,
    pub user_data: *mut std::ffi::c_void,
}

/// Batch processing result
#[repr(C)]
pub struct AniDBBatchResult {
    pub total_files: usize,
    pub successful_files: usize,
    pub failed_files: usize,
    pub results: *mut AniDBFileResult,
    pub total_time_ms: u64,
}

/// Event data union for different event types
#[repr(C)]
#[derive(Clone, Copy)]
pub union AniDBEventData {
    pub file: FileEventData,
    pub hash: HashEventData,
    pub network: NetworkEventData,
    pub memory: MemoryEventData,
}

/// File event data
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FileEventData {
    pub file_path: *const c_char,
    pub file_size: u64,
}

/// Hash event data
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HashEventData {
    pub algorithm: AniDBHashAlgorithm,
    pub hash_value: *const c_char,
}

/// Network event data
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NetworkEventData {
    pub endpoint: *const c_char,
    pub status_code: i32,
}

/// Memory event data
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemoryEventData {
    pub current_usage: u64,
    pub max_usage: u64,
}

/// Event structure for event callbacks
#[repr(C)]
#[derive(Clone)]
pub struct AniDBEvent {
    pub event_type: AniDBEventType,
    pub timestamp: u64,
    pub data: AniDBEventData,
    pub context: *const c_char,
}

// AniDBEvent contains raw pointers but they're only used for FFI
unsafe impl Send for AniDBEvent {}

impl Default for AniDBEvent {
    fn default() -> Self {
        Self {
            event_type: AniDBEventType::FileStart,
            timestamp: 0,
            data: AniDBEventData {
                memory: MemoryEventData {
                    current_usage: 0,
                    max_usage: 0,
                },
            },
            context: ptr::null(),
        }
    }
}

/// Callback function types
pub type AniDBProgressCallback = extern "C" fn(f32, u64, u64, *mut std::ffi::c_void);
pub type AniDBErrorCallback =
    extern "C" fn(AniDBResult, *const c_char, *const c_char, *mut std::ffi::c_void);
pub type AniDBCompletionCallback = extern "C" fn(AniDBResult, *mut std::ffi::c_void);
pub type AniDBEventCallback = extern "C" fn(*const AniDBEvent, *mut std::ffi::c_void);
