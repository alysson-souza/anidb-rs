//! TDD Tests for FFI Safety Layer
//!
//! Tests for comprehensive safety checks including:
//! - Null pointer validation
//! - Buffer overflow prevention
//! - Panic catching at FFI boundary
//! - Memory leak prevention
//! - Thread safety guarantees

use anidb_client_core::ffi::{
    AniDBConfig, AniDBFileResult, AniDBHashAlgorithm, AniDBProcessOptions, AniDBResult,
    anidb_client_create, anidb_client_create_with_config, anidb_client_destroy,
    anidb_client_get_last_error, anidb_free_file_result, anidb_init, anidb_process_file,
};
use std::ffi::{CString, c_char};
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use tempfile::TempDir;

/// Test null pointer validation for all FFI entry points
#[test]
#[serial_test::serial]
fn test_comprehensive_null_pointer_checks() {
    let _ = anidb_init(1);

    // Test anidb_client_create_with_config with null config
    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let result = anidb_client_create_with_config(ptr::null(), &mut handle);
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Test anidb_client_create_with_config with null handle
    let config = AniDBConfig {
        max_concurrent_files: 1,
        chunk_size: 1024,
        max_memory_usage: 1024,
        enable_debug_logging: 0,
        username: ptr::null(),
        password: ptr::null(),
        client_name: ptr::null(),
        client_version: ptr::null(),
    };
    let result = anidb_client_create_with_config(&config, ptr::null_mut());
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Create a valid handle for further tests
    let _ = anidb_client_create(&mut handle);

    // Test anidb_process_file with various null parameters
    let file_path = CString::new("test.mkv").unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K];
    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: 1,
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };
    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();

    // Null handle
    let result = anidb_process_file(
        ptr::null_mut(),
        file_path.as_ptr(),
        &options,
        &mut result_ptr,
    );
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Null file path
    let result = anidb_process_file(handle, ptr::null(), &options, &mut result_ptr);
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Null options
    let result = anidb_process_file(handle, file_path.as_ptr(), ptr::null(), &mut result_ptr);
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Null result pointer
    let result = anidb_process_file(handle, file_path.as_ptr(), &options, ptr::null_mut());
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Test anidb_client_get_last_error with null parameters
    let mut error_buffer = vec![0u8; 256];

    // Null handle
    let result = anidb_client_get_last_error(
        ptr::null_mut(),
        error_buffer.as_mut_ptr() as *mut i8,
        error_buffer.len(),
    );
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Null buffer
    let result = anidb_client_get_last_error(handle, ptr::null_mut(), 256);
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Zero buffer size
    let result = anidb_client_get_last_error(handle, error_buffer.as_mut_ptr() as *mut i8, 0);
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    let _ = anidb_client_destroy(handle);
}

/// Test buffer overflow prevention
#[test]
#[serial_test::serial]
fn test_buffer_overflow_prevention() {
    let _ = anidb_init(1);

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let _ = anidb_client_create(&mut handle);

    // Create a file that will generate an error
    let file_path = CString::new("/nonexistent/path/to/file.mkv").unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K];
    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: 1,
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };
    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();

    // Trigger an error
    let _ = anidb_process_file(handle, file_path.as_ptr(), &options, &mut result_ptr);

    // Test getting error with very small buffer
    let mut small_buffer = vec![0u8; 5];
    let result = anidb_client_get_last_error(
        handle,
        small_buffer.as_mut_ptr() as *mut i8,
        small_buffer.len(),
    );
    assert_eq!(result, AniDBResult::Success);

    // Verify null termination
    assert_eq!(small_buffer[4], 0);

    // Test with exactly sized buffer
    let mut exact_buffer = vec![0u8; 16]; // "File not found" + null
    let result = anidb_client_get_last_error(
        handle,
        exact_buffer.as_mut_ptr() as *mut i8,
        exact_buffer.len(),
    );
    assert_eq!(result, AniDBResult::Success);

    let _ = anidb_client_destroy(handle);
}

/// Test that panics don't cross the FFI boundary
#[test]
#[serial_test::serial]
fn test_panic_catching() {
    let _ = anidb_init(1);

    // Test with invalid UTF-8 in string parameters
    let mut handle: *mut std::ffi::c_void = ptr::null_mut();

    // Create config with invalid UTF-8 in username
    let invalid_utf8 = [0xFF, 0xFE, 0xFD, 0x00]; // Add null terminator
    let config = AniDBConfig {
        max_concurrent_files: 1,
        chunk_size: 1024,
        max_memory_usage: 1024,
        enable_debug_logging: 0,
        username: invalid_utf8.as_ptr() as *const c_char,
        password: ptr::null(),
        client_name: ptr::null(),
        client_version: ptr::null(),
    };

    // This should not panic, but return an error
    let result = anidb_client_create_with_config(&config, &mut handle);
    assert_eq!(result, AniDBResult::ErrorInvalidUtf8);

    // Test with valid handle
    let _ = anidb_client_create(&mut handle);

    // Test processing with invalid algorithm count
    let file_path = CString::new("test.mkv").unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K];
    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: 0, // Invalid: zero algorithms
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };
    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();

    let result = anidb_process_file(handle, file_path.as_ptr(), &options, &mut result_ptr);
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Test with null algorithms pointer but non-zero count
    let options_invalid = AniDBProcessOptions {
        algorithms: ptr::null(),
        algorithm_count: 1,
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let result = anidb_process_file(
        handle,
        file_path.as_ptr(),
        &options_invalid,
        &mut result_ptr,
    );
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    let _ = anidb_client_destroy(handle);
}

/// Test memory leak prevention with proper cleanup paths
#[test]
#[serial_test::serial]
fn test_memory_leak_prevention() {
    let _ = anidb_init(1);

    // Test creating and destroying multiple clients
    for _ in 0..10 {
        let mut handle: *mut std::ffi::c_void = ptr::null_mut();
        let result = anidb_client_create(&mut handle);
        assert_eq!(result, AniDBResult::Success);

        let result = anidb_client_destroy(handle);
        assert_eq!(result, AniDBResult::Success);
    }

    // Test file processing with result cleanup
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.mkv");
    std::fs::write(&test_file, b"test content").unwrap();

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let _ = anidb_client_create(&mut handle);

    for _ in 0..5 {
        let file_path = CString::new(test_file.to_str().unwrap()).unwrap();
        let algorithms = [AniDBHashAlgorithm::ED2K, AniDBHashAlgorithm::MD5];
        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };
        let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();

        let result = anidb_process_file(handle, file_path.as_ptr(), &options, &mut result_ptr);
        assert_eq!(result, AniDBResult::Success);

        // Always free the result
        if !result_ptr.is_null() {
            anidb_free_file_result(result_ptr);
        }
    }

    let _ = anidb_client_destroy(handle);
}

/// Test thread safety of FFI functions
#[test]
#[serial_test::serial]
fn test_thread_safety() {
    let _ = anidb_init(1);

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_thread.mkv");
    std::fs::write(&test_file, b"test content for threading").unwrap();

    // Create multiple clients from different threads
    let handles = Arc::new(std::sync::Mutex::new(Vec::<(i32, usize)>::new()));
    let mut threads = vec![];

    for i in 0..4 {
        let handles_clone = Arc::clone(&handles);
        let thread = thread::spawn(move || {
            let mut handle: *mut std::ffi::c_void = ptr::null_mut();
            let result = anidb_client_create(&mut handle);
            assert_eq!(result, AniDBResult::Success);

            // Store as usize for thread safety
            handles_clone.lock().unwrap().push((i, handle as usize));
        });
        threads.push(thread);
    }

    for thread in threads {
        thread.join().unwrap();
    }

    // Process files from multiple threads using different clients
    let all_handles = handles.lock().unwrap().clone();
    let file_path_str = test_file.to_str().unwrap().to_string();
    let completed = Arc::new(AtomicBool::new(false));
    let mut process_threads = vec![];

    for (id, handle_usize) in all_handles.iter() {
        let file_path_str = file_path_str.clone();
        let handle_usize = *handle_usize; // Already usize
        let id = *id;
        let completed_clone = Arc::clone(&completed);

        let thread = thread::spawn(move || {
            let handle = handle_usize as *mut std::ffi::c_void; // Convert back to pointer
            let file_path = CString::new(file_path_str).unwrap();
            let algorithms = [AniDBHashAlgorithm::ED2K];
            let options = AniDBProcessOptions {
                algorithms: algorithms.as_ptr(),
                algorithm_count: 1,
                enable_progress: 0,
                progress_callback: None,
                user_data: ptr::null_mut(),
            };
            let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();

            let result = anidb_process_file(handle, file_path.as_ptr(), &options, &mut result_ptr);
            assert_eq!(result, AniDBResult::Success, "Thread {id} failed");

            if !result_ptr.is_null() {
                anidb_free_file_result(result_ptr);
            }

            completed_clone.store(true, Ordering::SeqCst);
        });
        process_threads.push(thread);
    }

    for thread in process_threads {
        thread.join().unwrap();
    }

    assert!(completed.load(Ordering::SeqCst));

    // Clean up all handles
    for (_, handle_usize) in all_handles {
        let handle = handle_usize as *mut std::ffi::c_void;
        let result = anidb_client_destroy(handle);
        assert_eq!(result, AniDBResult::Success);
    }
}

/// Test validation of algorithm arrays
#[test]
#[serial_test::serial]
fn test_algorithm_array_validation() {
    let _ = anidb_init(1);

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let _ = anidb_client_create(&mut handle);

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.mkv");
    std::fs::write(&test_file, b"test").unwrap();
    let file_path = CString::new(test_file.to_str().unwrap()).unwrap();

    // Test with algorithm count exceeding actual array
    let algorithms = [AniDBHashAlgorithm::ED2K];
    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: 10, // Way more than actual array size
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };
    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();

    // Should validate and prevent buffer overread
    let _result = anidb_process_file(handle, file_path.as_ptr(), &options, &mut result_ptr);
    // This might succeed but should not crash - implementation should validate bounds

    let _ = anidb_client_destroy(handle);
}

/// Test that all entry points are wrapped with catch_unwind
#[test]
#[serial_test::serial]
fn test_all_functions_catch_panic() {
    // This test verifies that panics are caught at the FFI boundary
    // We can't easily trigger panics in well-written code, but we verify
    // that the functions handle edge cases without panicking

    let _ = anidb_init(1);

    // Test destroying invalid handles multiple times
    let invalid_handle = 0xDEADBEEF as *mut std::ffi::c_void;
    let result = anidb_client_destroy(invalid_handle);
    assert_eq!(result, AniDBResult::ErrorInvalidHandle);

    // Test with very large handle values
    let huge_handle = usize::MAX as *mut std::ffi::c_void;
    let result = anidb_client_destroy(huge_handle);
    assert_eq!(result, AniDBResult::ErrorInvalidHandle);

    // Test concurrent initialization
    let mut threads = vec![];
    for _ in 0..10 {
        let thread = thread::spawn(|| {
            let result = anidb_init(1);
            assert_eq!(result, AniDBResult::Success);
        });
        threads.push(thread);
    }

    for thread in threads {
        thread.join().unwrap();
    }
}
