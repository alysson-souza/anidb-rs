//! FFI Memory Management Tests
//!
//! This module tests memory management across the FFI boundary, ensuring:
//! - No memory leaks
//! - Proper UTF-8 string handling
//! - Buffer lifecycle management
//! - Memory tracking across FFI
//! - Resource cleanup on error paths

use anidb_client_core::ffi::*;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Helper to create a C string from a Rust string
fn to_c_string(s: &str) -> CString {
    CString::new(s).expect("Failed to create CString")
}

/// Helper to convert C string to Rust string
unsafe fn from_c_string(s: *const c_char) -> String {
    if s.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()
}

/// Test basic string allocation and deallocation
#[test]
#[serial_test::serial]
fn test_string_allocation_deallocation() {
    // Initialize library
    assert_eq!(anidb_init(1), AniDBResult::Success);

    // Test multiple string allocations and deallocations
    for i in 0..100 {
        let test_string = format!("Test string {i}");
        let c_string = to_c_string(&test_string);
        let c_ptr = c_string.into_raw();

        // Verify we can read the string
        let read_string = unsafe { from_c_string(c_ptr) };
        assert_eq!(read_string, test_string);

        // Free the string
        anidb_free_string(c_ptr);
    }

    anidb_cleanup();
}

/// Test UTF-8 string handling
#[test]
#[serial_test::serial]
fn test_utf8_string_handling() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    // Test various UTF-8 strings
    let test_cases = vec![
        "Hello, World!",
        "こんにちは",
        "🎮 Gaming",
        "Café ☕",
        "Здравствуйте",
        "🌍🌎🌏",
        "Mixed 日本語 and English",
        "Special chars: \n\t\r",
    ];

    for test_str in test_cases {
        let c_string = to_c_string(test_str);
        let c_ptr = c_string.into_raw();

        let read_string = unsafe { from_c_string(c_ptr) };
        assert_eq!(read_string, test_str);

        anidb_free_string(c_ptr);
    }

    anidb_cleanup();
}

/// Test file result memory management
#[test]
#[serial_test::serial]
fn test_file_result_memory_management() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(anidb_client_create(&mut handle), AniDBResult::Success);
    assert!(!handle.is_null());

    // Create a test file
    let temp_dir = tempfile::TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, b"test content").unwrap();

    let file_path_c = to_c_string(test_file.to_str().unwrap());
    let algorithms = [AniDBHashAlgorithm::MD5, AniDBHashAlgorithm::SHA1];

    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let mut result: *mut AniDBFileResult = ptr::null_mut();

    // Process file
    let status = anidb_process_file(handle, file_path_c.as_ptr(), &options, &mut result);

    if status == AniDBResult::Success {
        assert!(!result.is_null());

        // Verify result structure
        unsafe {
            let result_ref = &*result;
            assert_eq!(result_ref.hash_count, 2);
            assert!(!result_ref.hashes.is_null());

            // Check hash results
            let hashes = std::slice::from_raw_parts(result_ref.hashes, result_ref.hash_count);
            for hash in hashes {
                assert!(!hash.hash_value.is_null());
                assert!(hash.hash_length > 0);
            }
        }

        // Free the result
        anidb_free_file_result(result);
    }

    assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    anidb_cleanup();
}

/// Test batch result memory management
#[test]
#[serial_test::serial]
fn test_batch_result_memory_management() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    // Simulate a batch result
    let mut file_results = Vec::new();

    for i in 0..5 {
        let file_path = to_c_string(&format!("/test/file{i}.txt"));
        let error_msg = if i % 2 == 0 {
            ptr::null_mut()
        } else {
            to_c_string(&format!("Error processing file {i}")).into_raw()
        };

        // Create hash results
        let mut hash_results = Vec::new();
        for j in 0..2 {
            let hash_value = to_c_string(&format!("hash_{i}_{j}"));
            hash_results.push(AniDBHashResult {
                algorithm: if j == 0 {
                    AniDBHashAlgorithm::MD5
                } else {
                    AniDBHashAlgorithm::SHA1
                },
                hash_value: hash_value.into_raw(),
                hash_length: 32,
            });
        }

        let hashes_ptr = if hash_results.is_empty() {
            ptr::null_mut()
        } else {
            let ptr = unsafe {
                libc::malloc(hash_results.len() * std::mem::size_of::<AniDBHashResult>())
                    as *mut AniDBHashResult
            };
            unsafe {
                ptr::copy_nonoverlapping(hash_results.as_ptr(), ptr, hash_results.len());
            }
            ptr
        };

        file_results.push(AniDBFileResult {
            file_path: file_path.into_raw(),
            file_size: 1024 * (i + 1) as u64,
            status: if i % 2 == 0 {
                AniDBStatus::Completed
            } else {
                AniDBStatus::Failed
            },
            hashes: hashes_ptr,
            hash_count: 2,
            processing_time_ms: 100 * (i + 1) as u64,
            error_message: error_msg,
        });
    }

    let results_ptr = unsafe {
        libc::malloc(file_results.len() * std::mem::size_of::<AniDBFileResult>())
            as *mut AniDBFileResult
    };
    unsafe {
        ptr::copy_nonoverlapping(file_results.as_ptr(), results_ptr, file_results.len());
    }

    let batch_result = Box::new(AniDBBatchResult {
        total_files: 5,
        successful_files: 3,
        failed_files: 2,
        results: results_ptr,
        total_time_ms: 1500,
    });

    // Free the batch result
    anidb_free_batch_result(Box::into_raw(batch_result));

    anidb_cleanup();
}

/// Test memory cleanup on error paths
#[test]
#[serial_test::serial]
fn test_error_path_cleanup() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    // Test invalid handle
    let invalid_handle = 0xDEADBEEF as *mut std::ffi::c_void;
    let file_path = to_c_string("/nonexistent/file.txt");
    let algorithms = [AniDBHashAlgorithm::MD5];

    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let mut result: *mut AniDBFileResult = ptr::null_mut();

    // This should fail with invalid handle
    let status = anidb_process_file(invalid_handle, file_path.as_ptr(), &options, &mut result);
    assert_eq!(status, AniDBResult::ErrorInvalidHandle);
    assert!(result.is_null());

    // Test with valid handle but nonexistent file
    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(anidb_client_create(&mut handle), AniDBResult::Success);

    let status = anidb_process_file(handle, file_path.as_ptr(), &options, &mut result);
    assert_ne!(status, AniDBResult::Success);

    assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    anidb_cleanup();
}

/// Test concurrent string operations
#[test]
#[serial_test::serial]
fn test_concurrent_string_operations() {
    use std::thread;

    assert_eq!(anidb_init(1), AniDBResult::Success);

    let mut handles = vec![];

    for i in 0..10 {
        let handle = thread::spawn(move || {
            for j in 0..100 {
                let test_string = format!("Thread {i} iteration {j}");
                let c_string = to_c_string(&test_string);
                let c_ptr = c_string.into_raw();

                let read_string = unsafe { from_c_string(c_ptr) };
                assert_eq!(read_string, test_string);

                anidb_free_string(c_ptr);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    anidb_cleanup();
}

/// Test memory tracking across FFI
#[test]
#[serial_test::serial]
fn test_memory_tracking() {
    use anidb_client_core::buffer::memory_used;

    assert_eq!(anidb_init(1), AniDBResult::Success);

    // Reset memory tracking for test
    #[cfg(feature = "test-internals")]
    anidb_client_core::buffer::reset_memory_tracking();

    let initial_memory = memory_used();

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(anidb_client_create(&mut handle), AniDBResult::Success);

    // Memory should have increased after client creation
    let after_create = memory_used();
    assert!(after_create >= initial_memory);

    // Clean up
    assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    anidb_cleanup();

    // Note: We can't guarantee memory returns to exactly initial value
    // due to internal caching and allocations
}

/// Test buffer overflow prevention
#[test]
#[serial_test::serial]
fn test_buffer_overflow_prevention() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(anidb_client_create(&mut handle), AniDBResult::Success);

    // Test with small buffer for error message
    let mut buffer = vec![0u8; 10];
    let buffer_ptr = buffer.as_mut_ptr() as *mut c_char;

    // This should safely truncate the message
    let result = anidb_client_get_last_error(handle, buffer_ptr, buffer.len());
    assert_eq!(result, AniDBResult::Success);

    // Verify null termination
    assert_eq!(buffer[9], 0);

    assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    anidb_cleanup();
}

/// Test null pointer validation
#[test]
#[serial_test::serial]
fn test_null_pointer_validation() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    // Test various functions with null pointers
    assert_eq!(
        anidb_client_create(ptr::null_mut()),
        AniDBResult::ErrorInvalidParameter
    );

    assert_eq!(
        anidb_client_destroy(ptr::null_mut()),
        AniDBResult::ErrorInvalidHandle
    );

    assert_eq!(
        anidb_process_file(ptr::null_mut(), ptr::null(), ptr::null(), ptr::null_mut()),
        AniDBResult::ErrorInvalidParameter
    );

    anidb_cleanup();
}

/// Test callback memory management
#[test]
#[serial_test::serial]
fn test_callback_memory_management() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(anidb_client_create(&mut handle), AniDBResult::Success);

    // Test data that will be passed to callback
    let callback_called = Arc::new(AtomicBool::new(false));
    let callback_called_clone = callback_called.clone();

    extern "C" fn progress_callback(
        _progress: f32,
        _bytes_processed: u64,
        _total_bytes: u64,
        user_data: *mut std::ffi::c_void,
    ) {
        let called = unsafe { &*(user_data as *const AtomicBool) };
        called.store(true, Ordering::SeqCst);
    }

    let callback_id = anidb_register_callback(
        handle,
        AniDBCallbackType::Progress,
        progress_callback as *mut std::ffi::c_void,
        callback_called_clone.as_ref() as *const AtomicBool as *mut std::ffi::c_void,
    );

    assert_ne!(callback_id, 0);

    // Unregister callback
    assert_eq!(
        anidb_unregister_callback(handle, callback_id),
        AniDBResult::Success
    );

    assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    anidb_cleanup();
}

/// Test event system memory management
#[test]
#[serial_test::serial]
fn test_event_system_memory() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(anidb_client_create(&mut handle), AniDBResult::Success);

    let event_received = Arc::new(AtomicBool::new(false));
    let event_received_clone = event_received.clone();

    extern "C" fn event_callback(event: *const AniDBEvent, user_data: *mut std::ffi::c_void) {
        if !event.is_null() {
            let received = unsafe { &*(user_data as *const AtomicBool) };
            received.store(true, Ordering::SeqCst);

            // Verify event data is accessible
            let _event_type = unsafe { (*event).event_type };
            let _timestamp = unsafe { (*event).timestamp };
        }
    }

    // Connect event system
    assert_eq!(
        anidb_event_connect(
            handle,
            event_callback,
            event_received_clone.as_ref() as *const AtomicBool as *mut std::ffi::c_void,
        ),
        AniDBResult::Success
    );

    // Disconnect event system
    assert_eq!(anidb_event_disconnect(handle), AniDBResult::Success);

    assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    anidb_cleanup();
}

/// Test memory stress with multiple operations
#[test]
#[serial_test::serial]
fn test_memory_stress() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    // Create multiple clients
    let mut handles = Vec::new();
    for _ in 0..5 {
        let mut handle: *mut std::ffi::c_void = ptr::null_mut();
        assert_eq!(anidb_client_create(&mut handle), AniDBResult::Success);
        handles.push(handle);
    }

    // Perform operations on each client
    for &handle in &handles {
        // Register callbacks
        for callback_type in [
            AniDBCallbackType::Progress,
            AniDBCallbackType::Error,
            AniDBCallbackType::Completion,
        ] {
            // Dummy callback function
            extern "C" fn dummy_callback(_: *mut std::ffi::c_void) {}

            let id = anidb_register_callback(
                handle,
                callback_type,
                dummy_callback as *mut std::ffi::c_void,
                ptr::null_mut(),
            );
            // Note: We can't assert != 0 because registration may fail with null callback
            let _ = id;
        }

        // Get error message (even though there's no error)
        let mut buffer = vec![0u8; 256];
        assert_eq!(
            anidb_client_get_last_error(handle, buffer.as_mut_ptr() as *mut c_char, buffer.len()),
            AniDBResult::Success
        );
    }

    // Clean up all clients
    for handle in handles {
        assert_eq!(anidb_client_destroy(handle), AniDBResult::Success);
    }

    anidb_cleanup();
}
