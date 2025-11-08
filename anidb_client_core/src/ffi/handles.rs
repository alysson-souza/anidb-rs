//! Handle registry and lifecycle management for FFI
//!
//! This module manages the lifecycle of FFI handles, including client,
//! operation, and batch states with their associated registries.

use crate::ffi::helpers::{c_str_to_string, generate_handle_id, validate_mut_ptr, validate_ptr};
use crate::ffi::types::{
    AniDBCallbackType, AniDBConfig, AniDBEvent, AniDBEventCallback, AniDBResult, AniDBStatus,
};
use crate::ffi_catch_panic;
use crate::{ClientConfig, Error, FileProcessor};
use std::collections::{HashMap, VecDeque};
use std::ffi::{CString, c_void};
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::{Arc, Mutex, RwLock};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

/// Callback registration information
pub(crate) struct CallbackRegistration {
    pub callback_type: AniDBCallbackType,
    pub callback_ptr: *mut c_void,
    pub user_data: *mut c_void,
}

// Ensure CallbackRegistration is Send + Sync by using raw pointers
unsafe impl Send for CallbackRegistration {}
unsafe impl Sync for CallbackRegistration {}

/// Event queue entry
pub(crate) struct EventEntry {
    pub event: AniDBEvent,
    // Store owned strings to ensure they remain valid
    #[allow(dead_code)]
    pub file_path: Option<CString>,
    #[allow(dead_code)]
    pub hash_value: Option<CString>,
    #[allow(dead_code)]
    pub endpoint: Option<CString>,
    #[allow(dead_code)]
    pub context: Option<CString>,
}

// EventEntry is Send because we own all the data
unsafe impl Send for EventEntry {}

/// Internal client state
pub(crate) struct ClientState {
    #[allow(dead_code)]
    pub config: ClientConfig,
    pub file_processor: Arc<FileProcessor>,
    pub runtime: Arc<Runtime>,
    pub last_error: Option<String>,
    #[allow(dead_code)]
    pub reference_count: AtomicUsize,

    // Callback management
    pub callbacks: Arc<Mutex<HashMap<u64, CallbackRegistration>>>,
    pub next_callback_id: Arc<AtomicU64>,

    // Event system
    pub event_callback: Arc<Mutex<Option<(AniDBEventCallback, usize)>>>, // Store user_data as usize
    pub event_queue: Arc<Mutex<VecDeque<EventEntry>>>,
    pub event_thread_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    pub event_sender: Arc<Mutex<Option<mpsc::UnboundedSender<EventEntry>>>>,
}

/// Internal operation state
#[allow(dead_code)]
pub(crate) struct OperationState {
    pub status: AniDBStatus,
    pub result: Option<crate::FileProcessingResult>,
    pub error: Option<Error>,
}

/// Internal batch state
#[allow(dead_code)]
pub(crate) struct BatchState {
    pub total_files: usize,
    pub completed_files: AtomicUsize,
    pub results: Mutex<Vec<Result<crate::FileProcessingResult, Error>>>,
    pub status: AniDBStatus,
}

// Handle registries
lazy_static::lazy_static! {
    pub(crate) static ref CLIENTS: RwLock<HashMap<usize, Arc<Mutex<ClientState>>>> = RwLock::new(HashMap::new());
    pub(crate) static ref OPERATIONS: RwLock<HashMap<usize, Arc<Mutex<OperationState>>>> = RwLock::new(HashMap::new());
    pub(crate) static ref BATCHES: RwLock<HashMap<usize, Arc<Mutex<BatchState>>>> = RwLock::new(HashMap::new());
    pub(crate) static ref NEXT_HANDLE_ID: AtomicUsize = AtomicUsize::new(1);
    pub(crate) static ref INITIALIZED: AtomicUsize = AtomicUsize::new(0);
}

/// Create a new AniDB client instance with default configuration
#[unsafe(no_mangle)]
pub extern "C" fn anidb_client_create(handle: *mut *mut c_void) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_mut_ptr(handle) {
            return AniDBResult::ErrorInvalidParameter;
        }

        let config = ClientConfig::default();
        create_client_with_config(config, handle)
    })
}

/// Create a new AniDB client instance with custom configuration
#[unsafe(no_mangle)]
pub extern "C" fn anidb_client_create_with_config(
    config: *const AniDBConfig,
    handle: *mut *mut c_void,
) -> AniDBResult {
    ffi_catch_panic!({
        // Validate parameters
        if !validate_ptr(config) || !validate_mut_ptr(handle) {
            return AniDBResult::ErrorInvalidParameter;
        }

        // Safe config access
        let ffi_config = unsafe { &*config };

        // Parse configuration with validation

        let username = if ffi_config.username.is_null() {
            None
        } else {
            match c_str_to_string(ffi_config.username) {
                Ok(s) => Some(s),
                Err(e) => return e,
            }
        };

        let password = if ffi_config.password.is_null() {
            None
        } else {
            match c_str_to_string(ffi_config.password) {
                Ok(s) => Some(s),
                Err(e) => return e,
            }
        };

        let client_name = if ffi_config.client_name.is_null() {
            None
        } else {
            match c_str_to_string(ffi_config.client_name) {
                Ok(s) => Some(s),
                Err(e) => return e,
            }
        };

        let client_version = if ffi_config.client_version.is_null() {
            None
        } else {
            match c_str_to_string(ffi_config.client_version) {
                Ok(s) => Some(s),
                Err(e) => return e,
            }
        };

        // Validate numeric parameters
        let max_concurrent = ffi_config.max_concurrent_files.clamp(1, 100);
        let chunk_size = ffi_config.chunk_size.clamp(1024, 10 * 1024 * 1024);
        let max_memory = ffi_config
            .max_memory_usage
            .clamp(10 * 1024 * 1024, 2 * 1024 * 1024 * 1024); // 10MB to 2GB

        let client_config = ClientConfig {
            max_concurrent_files: max_concurrent,
            chunk_size,
            max_memory_usage: max_memory,
            username,
            password,
            client_name,
            client_version,
        };

        create_client_with_config(client_config, handle)
    })
}

/// Internal helper to create client with config
pub(crate) fn create_client_with_config(
    config: ClientConfig,
    handle: *mut *mut c_void,
) -> AniDBResult {
    // Set the global memory limit based on config
    crate::buffer::set_memory_limit(config.max_memory_usage);

    // Create runtime
    let runtime = match Runtime::new() {
        Ok(rt) => Arc::new(rt),
        Err(_) => return AniDBResult::ErrorProcessing,
    };

    // Create file processor
    let file_processor = Arc::new(FileProcessor::new(config.clone()));

    let state = ClientState {
        config,
        file_processor,
        runtime,
        last_error: None,
        reference_count: AtomicUsize::new(1),
        callbacks: Arc::new(Mutex::new(HashMap::new())),
        next_callback_id: Arc::new(AtomicU64::new(1)),
        event_callback: Arc::new(Mutex::new(None)),
        event_queue: Arc::new(Mutex::new(VecDeque::new())),
        event_thread_handle: Arc::new(Mutex::new(None)),
        event_sender: Arc::new(Mutex::new(None)),
    };

    let handle_id = generate_handle_id();
    let client_arc = Arc::new(Mutex::new(state));

    // Store in registry
    CLIENTS.write().unwrap().insert(handle_id, client_arc);

    unsafe {
        *handle = handle_id as *mut c_void;
    }

    AniDBResult::Success
}

/// Destroy an AniDB client instance
#[unsafe(no_mangle)]
pub extern "C" fn anidb_client_destroy(handle: *mut c_void) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_mut_ptr(handle) {
            return AniDBResult::ErrorInvalidHandle;
        }

        let handle_id = handle as usize;

        // Validate handle ID is reasonable
        if handle_id == 0 || handle_id > usize::MAX / 2 {
            return AniDBResult::ErrorInvalidHandle;
        }

        // Remove from registry with proper error handling
        match CLIENTS.write() {
            Ok(mut clients) => match clients.remove(&handle_id) {
                Some(_) => AniDBResult::Success,
                None => AniDBResult::ErrorInvalidHandle,
            },
            Err(_) => AniDBResult::ErrorBusy,
        }
    })
}
