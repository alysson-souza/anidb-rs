//! TDD Tests for FFI Module
//!
//! Following TDD principles: Write FAILING tests first, then implement to make them pass.
//!
//! Tests the Foreign Function Interface for external language bindings.

use anidb_client_core::ffi::{
    // New API functions and types
    AniDBConfig,
    AniDBFileResult,
    AniDBHashAlgorithm,
    AniDBProcessOptions,
    AniDBResult,
    anidb_cleanup,
    anidb_client_create,
    anidb_client_create_with_config,
    anidb_client_destroy,
    anidb_client_get_last_error,
    anidb_error_string,
    anidb_free_file_result,
    anidb_free_string,
    anidb_get_abi_version,
    anidb_get_version,
    anidb_hash_algorithm_name,
    anidb_hash_buffer_size,
    anidb_init,
    anidb_process_file,
};
use std::ffi::{CStr, CString};
use std::ptr;
use tempfile::TempDir;

/// Test library initialization and cleanup
#[test]
#[serial_test::serial]
fn test_library_lifecycle() {
    // Initialize library with correct ABI version
    let result = anidb_init(1);
    assert_eq!(result, AniDBResult::Success);

    // Calling init again should still succeed (idempotent)
    let result = anidb_init(1);
    assert_eq!(result, AniDBResult::Success);

    // Wrong ABI version should fail
    let result = anidb_init(999);
    assert_eq!(result, AniDBResult::ErrorVersionMismatch);

    // Cleanup
    anidb_cleanup();
}

/// Test version functions
#[test]
#[serial_test::serial]
fn test_version_functions() {
    // Get version string
    let version_ptr = anidb_get_version();
    assert!(!version_ptr.is_null());

    unsafe {
        let version = CStr::from_ptr(version_ptr);
        assert_eq!(version.to_str().unwrap(), "0.1.0-alpha");
    }

    // Get ABI version
    let abi_version = anidb_get_abi_version();
    assert_eq!(abi_version, 1);
}

/// Test creating and destroying an AniDB client handle
#[test]
#[serial_test::serial]
fn test_client_handle_lifecycle() {
    // Initialize library
    let _ = anidb_init(1);

    // Create client
    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let result = anidb_client_create(&mut handle);
    assert_eq!(result, AniDBResult::Success);
    assert!(!handle.is_null());

    // Destroy client
    let result = anidb_client_destroy(handle);
    assert_eq!(result, AniDBResult::Success);

    // Using destroyed handle should return error
    let result = anidb_client_destroy(handle);
    assert_eq!(result, AniDBResult::ErrorInvalidHandle);

    // Cleanup
    anidb_cleanup();
}

/// Test creating client with configuration
#[test]
#[serial_test::serial]
fn test_client_creation_with_config() {
    // Initialize library
    let _ = anidb_init(1);

    let config = AniDBConfig {
        max_concurrent_files: 4,
        chunk_size: 64 * 1024,
        max_memory_usage: 500 * 1024 * 1024,
        enable_debug_logging: 0,
        username: ptr::null(),
        password: ptr::null(),
        client_name: ptr::null(),
        client_version: ptr::null(),
    };

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let result = anidb_client_create_with_config(&config, &mut handle);
    assert_eq!(result, AniDBResult::Success);
    assert!(!handle.is_null());

    // Clean up
    let result = anidb_client_destroy(handle);
    assert_eq!(result, AniDBResult::Success);

    anidb_cleanup();
}

/// Test null pointer handling
#[test]
#[serial_test::serial]
fn test_null_pointer_handling() {
    // Initialize library
    let _ = anidb_init(1);

    // Create with null handle pointer should return error
    let result = anidb_client_create(ptr::null_mut());
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Destroy null handle should return error
    let result = anidb_client_destroy(ptr::null_mut());
    assert_eq!(result, AniDBResult::ErrorInvalidHandle);

    // Process file with null parameters should return error
    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let _ = anidb_client_create(&mut handle);

    let result = anidb_process_file(ptr::null_mut(), ptr::null(), ptr::null(), ptr::null_mut());
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Clean up
    let _ = anidb_client_destroy(handle);
    anidb_cleanup();
}

/// Test file processing via FFI
#[test]
#[serial_test::serial]
fn test_file_processing_ffi() {
    // Initialize library
    let _ = anidb_init(1);

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.mkv");
    std::fs::write(&test_file, b"test content for FFI").unwrap();

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let result = anidb_client_create(&mut handle);
    assert_eq!(result, AniDBResult::Success);
    assert!(!handle.is_null());

    let filename = CString::new(test_file.to_str().unwrap()).unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K];

    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
    let result = anidb_process_file(handle, filename.as_ptr(), &options, &mut result_ptr);

    assert_eq!(result, AniDBResult::Success);
    assert!(!result_ptr.is_null());

    // Verify result
    unsafe {
        let file_result = &*result_ptr;
        assert_eq!(file_result.file_size, 20); // "test content for FFI"
        assert_eq!(file_result.hash_count, 1);
        assert!(!file_result.hashes.is_null());

        // Free the result
        anidb_free_file_result(result_ptr);
    }

    // Clean up
    let _ = anidb_client_destroy(handle);
    anidb_cleanup();
}

/// Test error handling and error messages
#[test]
#[serial_test::serial]
fn test_error_handling() {
    // Initialize library
    let _ = anidb_init(1);

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let _ = anidb_client_create(&mut handle);

    // Try to process non-existent file
    let filename = CString::new("/non/existent/file.mkv").unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K];

    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
    let result = anidb_process_file(handle, filename.as_ptr(), &options, &mut result_ptr);

    assert_eq!(result, AniDBResult::ErrorFileNotFound);

    // Get last error message
    let mut error_buffer = vec![0u8; 256];
    let result = anidb_client_get_last_error(
        handle,
        error_buffer.as_mut_ptr() as *mut i8,
        error_buffer.len(),
    );
    assert_eq!(result, AniDBResult::Success);

    // Clean up
    let _ = anidb_client_destroy(handle);
    anidb_cleanup();
}

/// Test utility functions
#[test]
#[serial_test::serial]
fn test_utility_functions() {
    // Test error string function
    let error_str = anidb_error_string(AniDBResult::ErrorFileNotFound);
    assert!(!error_str.is_null());
    unsafe {
        let error_cstr = CStr::from_ptr(error_str);
        assert_eq!(error_cstr.to_str().unwrap(), "File not found");
    }

    // Test hash algorithm name
    let algo_name = anidb_hash_algorithm_name(AniDBHashAlgorithm::ED2K);
    assert!(!algo_name.is_null());
    unsafe {
        let algo_cstr = CStr::from_ptr(algo_name);
        assert_eq!(algo_cstr.to_str().unwrap(), "ED2K");
    }

    // Test hash buffer size
    let buffer_size = anidb_hash_buffer_size(AniDBHashAlgorithm::ED2K);
    assert_eq!(buffer_size, 33); // 32 hex chars + null terminator

    let buffer_size = anidb_hash_buffer_size(AniDBHashAlgorithm::SHA1);
    assert_eq!(buffer_size, 41); // 40 hex chars + null terminator
}

/// Test progress callback functionality
#[test]
#[serial_test::serial]
fn test_progress_callback() {
    use std::sync::{Arc, Mutex};

    // Initialize library
    let _ = anidb_init(1);

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_progress.mkv");

    // Create a larger file to see progress
    let data = vec![0u8; 10 * 1024 * 1024]; // 10MB
    std::fs::write(&test_file, &data).unwrap();

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let _ = anidb_client_create(&mut handle);

    // Progress tracking
    let progress_data = Arc::new(Mutex::new(Vec::<f32>::new()));
    let progress_clone = Arc::clone(&progress_data);

    extern "C" fn progress_callback(
        percentage: f32,
        _bytes_processed: u64,
        _total_bytes: u64,
        user_data: *mut std::ffi::c_void,
    ) {
        let data = unsafe { &*(user_data as *const Arc<Mutex<Vec<f32>>>) };
        data.lock().unwrap().push(percentage);
    }

    let filename = CString::new(test_file.to_str().unwrap()).unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K, AniDBHashAlgorithm::MD5];

    let user_data_ptr = &progress_clone as *const Arc<Mutex<Vec<f32>>> as *mut std::ffi::c_void;

    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 1,
        progress_callback: Some(progress_callback),
        user_data: user_data_ptr,
    };

    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
    let result = anidb_process_file(handle, filename.as_ptr(), &options, &mut result_ptr);

    assert_eq!(result, AniDBResult::Success);

    // Give the callback thread a moment to process final messages
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Check that we received progress updates
    let progress_values = progress_data.lock().unwrap();
    assert!(
        !progress_values.is_empty(),
        "Should have received progress updates"
    );
    assert!(
        progress_values.last().unwrap() >= &99.0,
        "Final progress should be near 100%"
    );

    // Free result
    if !result_ptr.is_null() {
        anidb_free_file_result(result_ptr);
    }

    // Clean up
    let _ = anidb_client_destroy(handle);
    anidb_cleanup();
}

/// Test multiple hash algorithms
#[test]
#[serial_test::serial]
fn test_multiple_hash_algorithms() {
    // Initialize library
    let _ = anidb_init(1);

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_multi_hash.mkv");
    std::fs::write(&test_file, b"test content for multiple hashes").unwrap();

    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    let _ = anidb_client_create(&mut handle);

    let filename = CString::new(test_file.to_str().unwrap()).unwrap();
    let algorithms = [
        AniDBHashAlgorithm::ED2K,
        AniDBHashAlgorithm::CRC32,
        AniDBHashAlgorithm::MD5,
        AniDBHashAlgorithm::SHA1,
    ];

    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
    let result = anidb_process_file(handle, filename.as_ptr(), &options, &mut result_ptr);

    assert_eq!(result, AniDBResult::Success);
    assert!(!result_ptr.is_null());

    // Verify we got all requested hashes
    unsafe {
        let file_result = &*result_ptr;
        assert_eq!(file_result.hash_count, 4);
        assert!(!file_result.hashes.is_null());

        // Check each hash
        let hashes = std::slice::from_raw_parts(file_result.hashes, file_result.hash_count);
        for hash in hashes {
            assert!(!hash.hash_value.is_null());
            assert!(hash.hash_length > 0);
        }

        // Free the result
        anidb_free_file_result(result_ptr);
    }

    // Clean up
    let _ = anidb_client_destroy(handle);
    anidb_cleanup();
}

/// Test free functions for memory management
#[test]
#[serial_test::serial]
fn test_memory_management() {
    // Test freeing a null string (should not crash)
    anidb_free_string(ptr::null_mut());

    // Test freeing a null file result (should not crash)
    anidb_free_file_result(ptr::null_mut());

    // Test creating and freeing a string
    let test_str = CString::new("test string").unwrap();
    let raw_ptr = test_str.into_raw();
    anidb_free_string(raw_ptr);
}
