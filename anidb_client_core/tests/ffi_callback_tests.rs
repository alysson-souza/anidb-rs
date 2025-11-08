//! TDD Tests for FFI Callback System
//!
//! Following TDD principles: Write FAILING tests first, then implement to make them pass.
//!
//! Tests the comprehensive callback system for progress, errors, completion, and events.

use anidb_client_core::ffi::{
    AniDBCallbackType, AniDBConfig, AniDBEvent, AniDBFileResult, AniDBHashAlgorithm,
    AniDBProcessOptions, AniDBResult, anidb_cleanup, anidb_client_create_with_config,
    anidb_client_destroy, anidb_event_connect, anidb_event_disconnect, anidb_event_poll,
    anidb_free_file_result, anidb_init, anidb_process_file, anidb_register_callback,
    anidb_unregister_callback,
};
use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

/// Test context for callbacks
struct CallbackTestContext {
    progress_called: AtomicBool,
    error_called: AtomicBool,
    #[allow(dead_code)]
    completion_called: AtomicBool,
    event_received: AtomicBool,
    last_percentage: AtomicU32,
    bytes_processed: AtomicU64,
    total_bytes: AtomicU64,
    error_messages: Arc<Mutex<Vec<String>>>,
    events: Arc<Mutex<Vec<AniDBEvent>>>,
}

impl CallbackTestContext {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            progress_called: AtomicBool::new(false),
            error_called: AtomicBool::new(false),
            completion_called: AtomicBool::new(false),
            event_received: AtomicBool::new(false),
            last_percentage: AtomicU32::new(0),
            bytes_processed: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            error_messages: Arc::new(Mutex::new(Vec::new())),
            events: Arc::new(Mutex::new(Vec::new())),
        })
    }
}

/// Progress callback function
extern "C" fn progress_callback(
    percentage: f32,
    bytes_processed: u64,
    total_bytes: u64,
    user_data: *mut std::ffi::c_void,
) {
    if !user_data.is_null() {
        let context = unsafe { &*(user_data as *const CallbackTestContext) };
        context.progress_called.store(true, Ordering::SeqCst);
        context
            .last_percentage
            .store((percentage * 100.0) as u32, Ordering::SeqCst);
        context
            .bytes_processed
            .store(bytes_processed, Ordering::SeqCst);
        context.total_bytes.store(total_bytes, Ordering::SeqCst);
    }
}

/// Error callback function
extern "C" fn error_callback(
    _error_code: AniDBResult,
    error_message: *const std::os::raw::c_char,
    _file_path: *const std::os::raw::c_char,
    user_data: *mut std::ffi::c_void,
) {
    if !user_data.is_null() {
        let context = unsafe { &*(user_data as *const CallbackTestContext) };
        context.error_called.store(true, Ordering::SeqCst);

        if !error_message.is_null() {
            let msg = unsafe { CStr::from_ptr(error_message) }
                .to_string_lossy()
                .to_string();
            context.error_messages.lock().unwrap().push(msg);
        }
    }
}

/// Event callback function
extern "C" fn event_callback(event: *const AniDBEvent, user_data: *mut std::ffi::c_void) {
    if !user_data.is_null() && !event.is_null() {
        let context = unsafe { &*(user_data as *const CallbackTestContext) };
        context.event_received.store(true, Ordering::SeqCst);

        let event_copy = unsafe { (*event).clone() };
        context.events.lock().unwrap().push(event_copy);
    }
}

/// Test basic callback registration and unregistration
#[test]
#[serial_test::serial]
fn test_callback_registration() {
    anidb_init(1);

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let config = AniDBConfig {
        max_concurrent_files: 1,
        chunk_size: 65536,
        max_memory_usage: 0,
        enable_debug_logging: 0,
        username: ptr::null(),
        password: ptr::null(),
        client_name: ptr::null(),
        client_version: ptr::null(),
    };

    assert_eq!(
        anidb_client_create_with_config(&config, &mut handle),
        AniDBResult::Success
    );

    let context = CallbackTestContext::new();
    let context_ptr = Arc::as_ptr(&context) as *mut std::ffi::c_void;

    // Register progress callback
    let callback_id = anidb_register_callback(
        handle,
        AniDBCallbackType::Progress,
        progress_callback as *mut std::ffi::c_void,
        context_ptr,
    );
    assert_ne!(callback_id, 0);

    // Unregister callback
    assert_eq!(
        anidb_unregister_callback(handle, callback_id),
        AniDBResult::Success
    );

    // Unregistering again should fail
    assert_eq!(
        anidb_unregister_callback(handle, callback_id),
        AniDBResult::ErrorInvalidParameter
    );

    assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    anidb_cleanup();
}

/// Test progress callbacks during file processing
#[test]
#[serial_test::serial]
fn test_progress_callbacks() {
    anidb_init(1);

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.bin");
    std::fs::write(&test_file, vec![0u8; 1024 * 1024]).unwrap(); // 1MB file

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let config = AniDBConfig {
        max_concurrent_files: 1,
        chunk_size: 65536,
        max_memory_usage: 0,
        enable_debug_logging: 0,
        username: ptr::null(),
        password: ptr::null(),
        client_name: ptr::null(),
        client_version: ptr::null(),
    };

    assert_eq!(
        anidb_client_create_with_config(&config, &mut handle),
        AniDBResult::Success
    );

    let context = CallbackTestContext::new();
    let context_ptr = Arc::as_ptr(&context) as *mut std::ffi::c_void;

    // Register progress callback
    let callback_id = anidb_register_callback(
        handle,
        AniDBCallbackType::Progress,
        progress_callback as *mut std::ffi::c_void,
        context_ptr,
    );
    assert_ne!(callback_id, 0);

    // Process file with progress callbacks
    let file_path_cstr = CString::new(test_file.to_str().unwrap()).unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K];
    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 1,
        progress_callback: None, // Using registered callback instead
        user_data: ptr::null_mut(),
    };

    let mut result: *mut AniDBFileResult = ptr::null_mut();
    assert_eq!(
        anidb_process_file(handle, file_path_cstr.as_ptr(), &options, &mut result),
        AniDBResult::Success
    );

    // Verify progress was reported
    assert!(context.progress_called.load(Ordering::SeqCst));
    assert_eq!(context.total_bytes.load(Ordering::SeqCst), 1024 * 1024);
    assert_eq!(context.bytes_processed.load(Ordering::SeqCst), 1024 * 1024);

    // Clean up
    if !result.is_null() {
        anidb_free_file_result(result);
    }
    assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    anidb_cleanup();
}

/// Test error callbacks
#[test]
#[serial_test::serial]
fn test_error_callbacks() {
    anidb_init(1);

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let config = AniDBConfig {
        max_concurrent_files: 1,
        chunk_size: 65536,
        max_memory_usage: 0,
        enable_debug_logging: 0,
        username: ptr::null(),
        password: ptr::null(),
        client_name: ptr::null(),
        client_version: ptr::null(),
    };

    assert_eq!(
        anidb_client_create_with_config(&config, &mut handle),
        AniDBResult::Success
    );

    let context = CallbackTestContext::new();
    let context_ptr = Arc::as_ptr(&context) as *mut std::ffi::c_void;

    // Register error callback
    let callback_id = anidb_register_callback(
        handle,
        AniDBCallbackType::Error,
        error_callback as *mut std::ffi::c_void,
        context_ptr,
    );
    assert_ne!(callback_id, 0);

    // Try to process non-existent file
    let file_path_cstr = CString::new("/this/file/does/not/exist.bin").unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K];
    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let mut result: *mut AniDBFileResult = ptr::null_mut();
    let process_result = anidb_process_file(handle, file_path_cstr.as_ptr(), &options, &mut result);

    // Should fail with file not found
    assert_eq!(process_result, AniDBResult::ErrorFileNotFound);

    // Verify error callback was called
    assert!(context.error_called.load(Ordering::SeqCst));
    assert!(!context.error_messages.lock().unwrap().is_empty());

    assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    anidb_cleanup();
}

/// Test event system with multiple events
#[test]
#[serial_test::serial]
fn test_event_system() {
    anidb_init(1);

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let config = AniDBConfig {
        max_concurrent_files: 1,
        chunk_size: 65536,
        max_memory_usage: 0,
        enable_debug_logging: 0,
        username: ptr::null(),
        password: ptr::null(),
        client_name: ptr::null(),
        client_version: ptr::null(),
    };

    assert_eq!(
        anidb_client_create_with_config(&config, &mut handle),
        AniDBResult::Success
    );

    let context = CallbackTestContext::new();
    let context_ptr = Arc::as_ptr(&context) as *mut std::ffi::c_void;

    // Connect to event system
    assert_eq!(
        anidb_event_connect(handle, event_callback, context_ptr),
        AniDBResult::Success
    );

    // Process a file to generate events
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.bin");
    std::fs::write(&test_file, vec![0u8; 1024]).unwrap();

    let file_path_cstr = CString::new(test_file.to_str().unwrap()).unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K];
    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 1,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let mut result: *mut AniDBFileResult = ptr::null_mut();
    anidb_process_file(handle, file_path_cstr.as_ptr(), &options, &mut result);

    // Poll for events
    let mut event_buffer = vec![AniDBEvent::default(); 10];
    let mut event_count: usize = 0;
    assert_eq!(
        anidb_event_poll(
            handle,
            event_buffer.as_mut_ptr(),
            event_buffer.len(),
            &mut event_count,
        ),
        AniDBResult::Success
    );

    // Should have received some events
    assert!(event_count > 0);

    // Disconnect from event system
    assert_eq!(anidb_event_disconnect(handle), AniDBResult::Success);

    // Clean up
    if !result.is_null() {
        anidb_free_file_result(result);
    }
    assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    anidb_cleanup();
}

/// Test callback with context data
#[test]
#[serial_test::serial]
fn test_callback_context() {
    anidb_init(1);

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let config = AniDBConfig {
        max_concurrent_files: 1,
        chunk_size: 65536,
        max_memory_usage: 0,
        enable_debug_logging: 0,
        username: ptr::null(),
        password: ptr::null(),
        client_name: ptr::null(),
        client_version: ptr::null(),
    };

    assert_eq!(
        anidb_client_create_with_config(&config, &mut handle),
        AniDBResult::Success
    );

    // Custom context structure
    struct CustomContext {
        id: u32,
        name: String,
        counter: AtomicU32,
    }

    let custom_context = Arc::new(CustomContext {
        id: 42,
        name: "Test Context".to_string(),
        counter: AtomicU32::new(0),
    });

    // Custom callback that uses context
    extern "C" fn custom_progress_callback(
        _percentage: f32,
        _bytes_processed: u64,
        _total_bytes: u64,
        user_data: *mut std::ffi::c_void,
    ) {
        if !user_data.is_null() {
            let context = unsafe { &*(user_data as *const CustomContext) };
            context.counter.fetch_add(1, Ordering::SeqCst);
            assert_eq!(context.id, 42);
            assert_eq!(context.name, "Test Context");
        }
    }

    let context_ptr = Arc::as_ptr(&custom_context) as *mut std::ffi::c_void;

    // Register callback with custom context
    let callback_id = anidb_register_callback(
        handle,
        AniDBCallbackType::Progress,
        custom_progress_callback as *mut std::ffi::c_void,
        context_ptr,
    );
    assert_ne!(callback_id, 0);

    // Process file to trigger callbacks
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.bin");
    std::fs::write(&test_file, vec![0u8; 65536]).unwrap();

    let file_path_cstr = CString::new(test_file.to_str().unwrap()).unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K];
    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 1,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let mut result: *mut AniDBFileResult = ptr::null_mut();
    anidb_process_file(handle, file_path_cstr.as_ptr(), &options, &mut result);

    // Verify callback was called
    assert!(custom_context.counter.load(Ordering::SeqCst) > 0);

    // Clean up
    if !result.is_null() {
        anidb_free_file_result(result);
    }
    assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    anidb_cleanup();
}

/// Test thread safety of callbacks
#[test]
#[serial_test::serial]
fn test_callback_thread_safety() {
    anidb_init(1);

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let config = AniDBConfig {
        max_concurrent_files: 4,
        chunk_size: 65536,
        max_memory_usage: 0,
        enable_debug_logging: 0,
        username: ptr::null(),
        password: ptr::null(),
        client_name: ptr::null(),
        client_version: ptr::null(),
    };

    assert_eq!(
        anidb_client_create_with_config(&config, &mut handle),
        AniDBResult::Success
    );

    let context = CallbackTestContext::new();
    let context_ptr = Arc::as_ptr(&context) as *mut std::ffi::c_void;

    // Register all callback types
    let _progress_id = anidb_register_callback(
        handle,
        AniDBCallbackType::Progress,
        progress_callback as *mut std::ffi::c_void,
        context_ptr,
    );
    let _error_id = anidb_register_callback(
        handle,
        AniDBCallbackType::Error,
        error_callback as *mut std::ffi::c_void,
        context_ptr,
    );

    // Connect to event system
    anidb_event_connect(handle, event_callback, context_ptr);

    // Process multiple files to test callback thread safety
    let temp_dir = TempDir::new().unwrap();

    for i in 0..4 {
        let test_file = temp_dir.path().join(format!("test{i}.bin"));
        std::fs::write(&test_file, vec![0u8; 256 * 1024]).unwrap(); // 256KB files

        let file_path_cstr = CString::new(test_file.to_str().unwrap()).unwrap();
        let algorithms = [AniDBHashAlgorithm::ED2K, AniDBHashAlgorithm::CRC32];
        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 1,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let mut result: *mut AniDBFileResult = ptr::null_mut();
        let process_result =
            anidb_process_file(handle, file_path_cstr.as_ptr(), &options, &mut result);

        if !result.is_null() {
            anidb_free_file_result(result);
        }

        assert_eq!(process_result, AniDBResult::Success);
    }

    // Verify callbacks were called
    assert!(context.progress_called.load(Ordering::SeqCst));

    // Disconnect and clean up
    anidb_event_disconnect(handle);
    assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    anidb_cleanup();
}
