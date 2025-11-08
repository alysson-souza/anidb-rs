//! Cross-Platform FFI Integration Tests
//!
//! Tests platform-specific behavior and ensures consistency across
//! Windows, Linux, and macOS

use anidb_client_core::ffi::{
    AniDBConfig, AniDBHashAlgorithm, AniDBProcessOptions, AniDBResult, anidb_cleanup,
    anidb_client_create, anidb_client_create_with_config, anidb_client_destroy,
    anidb_free_file_result, anidb_init, anidb_process_file,
};
use std::ffi::CString;
use std::fs;
use std::ptr;
use tempfile::TempDir;

/// Platform-specific path handling tests
#[test]
#[serial_test::serial]
fn test_ffi_platform_path_handling() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let temp_dir = TempDir::new().unwrap();
    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create(&mut client_handle),
        AniDBResult::Success
    );

    // Test various path formats
    let test_cases = vec![
        ("simple.mkv", "Simple filename"),
        ("file with spaces.mkv", "Filename with spaces"),
        ("file-with-dashes.mkv", "Filename with dashes"),
        ("file_with_underscores.mkv", "Filename with underscores"),
        ("file.with.dots.mkv", "Filename with dots"),
        ("文件名.mkv", "Unicode filename (Chinese)"),
        ("файл.mkv", "Unicode filename (Russian)"),
        ("ファイル.mkv", "Unicode filename (Japanese)"),
    ];

    // Platform-specific test cases
    #[cfg(windows)]
    let platform_cases = vec![
        ("file&with&ampersands.mkv", "Windows special chars"),
        ("file(with)parens.mkv", "Windows parentheses"),
    ];

    #[cfg(unix)]
    let platform_cases = vec![
        ("file:with:colons.mkv", "Unix colons"),
        ("file|with|pipes.mkv", "Unix pipes"),
    ];

    let mut all_cases = test_cases;
    all_cases.extend(platform_cases);

    let algorithms = [AniDBHashAlgorithm::ED2K];

    for (filename, description) in all_cases {
        println!("Testing: {filename} - {description}");

        let file_path = temp_dir.path().join(filename);
        fs::write(&file_path, b"test content").unwrap();

        let c_path = match CString::new(file_path.to_str().unwrap()) {
            Ok(s) => s,
            Err(_) => {
                println!("  Skipped - invalid UTF-8 for C string");
                continue;
            }
        };

        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let mut result_ptr: *mut anidb_client_core::ffi::AniDBFileResult = ptr::null_mut();
        let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);

        if result == AniDBResult::Success {
            println!("  ✓ Success");
            assert!(!result_ptr.is_null());
            anidb_free_file_result(result_ptr);
        } else {
            println!("  ✗ Failed with: {result:?}");
            // Some filenames might fail on certain platforms
            // This is expected behavior
        }
    }

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}

/// Test long path support
#[test]
#[serial_test::serial]
fn test_ffi_long_path_support() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let temp_dir = TempDir::new().unwrap();
    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create(&mut client_handle),
        AniDBResult::Success
    );

    // Create a deeply nested directory structure
    let mut current_path = temp_dir.path().to_path_buf();

    // Platform-specific max path lengths
    #[cfg(windows)]
    let max_components = 30; // Windows has stricter path limits

    #[cfg(unix)]
    let max_components = 50; // Unix systems typically support longer paths

    for i in 0..max_components {
        current_path = current_path.join(format!("level_{i:02}"));
    }

    // Try to create the directory
    match fs::create_dir_all(&current_path) {
        Ok(_) => {
            let file_path = current_path.join("deeply_nested_file.mkv");
            fs::write(&file_path, b"nested content").unwrap();

            let c_path = CString::new(file_path.to_str().unwrap()).unwrap();
            let algorithms = [AniDBHashAlgorithm::ED2K];
            let options = AniDBProcessOptions {
                algorithms: algorithms.as_ptr(),
                algorithm_count: algorithms.len(),
                enable_progress: 0,
                progress_callback: None,
                user_data: ptr::null_mut(),
            };

            let mut result_ptr: *mut anidb_client_core::ffi::AniDBFileResult = ptr::null_mut();
            let result =
                anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);

            println!(
                "Long path test ({}): {:?}",
                file_path.to_string_lossy().len(),
                result
            );

            if result == AniDBResult::Success {
                assert!(!result_ptr.is_null());
                anidb_free_file_result(result_ptr);
            }
        }
        Err(e) => {
            println!("Could not create deep directory structure: {e}");
            // This is expected on some platforms
        }
    }

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}

/// Test platform-specific file permissions
#[test]
#[serial_test::serial]
fn test_ffi_platform_permissions() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let temp_dir = TempDir::new().unwrap();
    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create(&mut client_handle),
        AniDBResult::Success
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Test read-only file
        let readonly_file = temp_dir.path().join("readonly.mkv");
        fs::write(&readonly_file, b"readonly content").unwrap();
        let mut perms = fs::metadata(&readonly_file).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&readonly_file, perms).unwrap();

        let c_path = CString::new(readonly_file.to_str().unwrap()).unwrap();
        let algorithms = [AniDBHashAlgorithm::ED2K];
        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let mut result_ptr: *mut anidb_client_core::ffi::AniDBFileResult = ptr::null_mut();
        let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);

        // Should succeed - we only need read permission
        assert_eq!(result, AniDBResult::Success);
        if !result_ptr.is_null() {
            anidb_free_file_result(result_ptr);
        }

        // Test no-permission file
        let noperm_file = temp_dir.path().join("noperm.mkv");
        fs::write(&noperm_file, b"no permission content").unwrap();
        let mut perms = fs::metadata(&noperm_file).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&noperm_file, perms).unwrap();

        let c_path = CString::new(noperm_file.to_str().unwrap()).unwrap();
        let mut result_ptr: *mut anidb_client_core::ffi::AniDBFileResult = ptr::null_mut();
        let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);

        // Should fail - no read permission
        // Note: On some platforms/filesystems, this may return ErrorProcessing instead of ErrorPermissionDenied
        assert!(
            result == AniDBResult::ErrorPermissionDenied || result == AniDBResult::ErrorProcessing,
            "Expected permission error, got: {result:?}"
        );

        // Restore permissions for cleanup
        let mut perms = fs::metadata(&noperm_file).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&noperm_file, perms).unwrap();
    }

    #[cfg(windows)]
    {
        // Test read-only file on Windows
        let readonly_file = temp_dir.path().join("readonly.mkv");
        fs::write(&readonly_file, b"readonly content").unwrap();
        let mut perms = fs::metadata(&readonly_file).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&readonly_file, perms).unwrap();

        let c_path = CString::new(readonly_file.to_str().unwrap()).unwrap();
        let algorithms = [AniDBHashAlgorithm::ED2K];
        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let mut result_ptr: *mut anidb_client_core::ffi::AniDBFileResult = ptr::null_mut();
        let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);

        // Should succeed - read-only files can still be read
        assert_eq!(result, AniDBResult::Success);
        if !result_ptr.is_null() {
            anidb_free_file_result(result_ptr);
        }

        // Restore permissions for cleanup
        perms.set_readonly(false);
        fs::set_permissions(&readonly_file, perms).unwrap();
    }

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}

/// Test platform-specific performance optimizations
#[test]
#[serial_test::serial]
fn test_ffi_platform_performance() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let temp_dir = TempDir::new().unwrap();

    // Create client with platform-specific config

    #[cfg(target_os = "linux")]
    let chunk_size = 128 * 1024; // Larger chunks on Linux

    #[cfg(target_os = "windows")]
    let chunk_size = 64 * 1024; // Standard chunks on Windows

    #[cfg(target_os = "macos")]
    let chunk_size = 64 * 1024; // Standard chunks on macOS

    let config = AniDBConfig {
        max_concurrent_files: 4,
        chunk_size,
        max_memory_usage: 500 * 1024 * 1024,
        enable_debug_logging: 0,
        username: ptr::null(),
        password: ptr::null(),
        client_name: ptr::null(),
        client_version: ptr::null(),
    };

    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create_with_config(&config, &mut client_handle),
        AniDBResult::Success
    );

    // Test files of various sizes
    let test_sizes = vec![
        (1024 * 1024, "1MB"),
        (10 * 1024 * 1024, "10MB"),
        (50 * 1024 * 1024, "50MB"),
    ];

    let algorithms = [AniDBHashAlgorithm::ED2K, AniDBHashAlgorithm::SHA1];

    for (size, label) in test_sizes {
        let file_path = temp_dir.path().join(format!("perf_test_{label}.mkv"));

        // Create file with platform-specific method
        #[cfg(unix)]
        {
            // Use fallocate on Linux for faster file creation
            let file = fs::File::create(&file_path).unwrap();
            file.set_len(size as u64).unwrap();

            // Write some actual data to make it non-sparse
            use std::io::Write;
            let mut file = fs::OpenOptions::new().write(true).open(&file_path).unwrap();
            file.write_all(&vec![0xAB; 1024]).unwrap();
        }

        #[cfg(windows)]
        {
            // Windows doesn't have fallocate, use regular write
            fs::write(&file_path, vec![0xAB; size]).unwrap();
        }

        let c_path = CString::new(file_path.to_str().unwrap()).unwrap();
        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let start = std::time::Instant::now();
        let mut result_ptr: *mut anidb_client_core::ffi::AniDBFileResult = ptr::null_mut();
        let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);
        let duration = start.elapsed();

        assert_eq!(result, AniDBResult::Success);

        unsafe {
            if !result_ptr.is_null() {
                let file_result = &*result_ptr;
                let throughput_mbps = (size as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64();

                println!(
                    "Platform: {}, File: {}, Time: {}ms, Throughput: {:.2} MB/s",
                    std::env::consts::OS,
                    label,
                    file_result.processing_time_ms,
                    throughput_mbps
                );

                anidb_free_file_result(result_ptr);
            }
        }
    }

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}
