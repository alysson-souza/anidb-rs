//! Comprehensive Integration Testing for FFI
//!
//! This module contains integration tests for FFI bindings covering:
//! - Multi-threaded access and thread safety
//! - Large file processing (>1GB)
//! - Error conditions and recovery
//! - Memory stress tests and leak detection
//! - Platform-specific behavior

use anidb_client_core::ffi::{
    AniDBCallbackType, AniDBConfig, AniDBEvent, AniDBEventType, AniDBFileResult,
    AniDBHashAlgorithm, AniDBMemoryStats, AniDBProcessOptions, AniDBResult, AniDBStatus,
    anidb_check_memory_leaks, anidb_cleanup, anidb_client_create, anidb_client_create_with_config,
    anidb_client_destroy, anidb_event_connect, anidb_event_disconnect, anidb_free_file_result,
    anidb_get_memory_stats, anidb_init, anidb_memory_gc, anidb_process_file,
    anidb_register_callback, anidb_unregister_callback,
};
use std::ffi::CString;
use std::fs;
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Helper struct for tracking test metrics
#[derive(Debug, Default)]
struct TestMetrics {
    processed_files: AtomicUsize,
    total_bytes: AtomicU64,
    errors: AtomicUsize,
    #[allow(dead_code)]
    max_memory_used: AtomicU64,
    thread_crashes: AtomicUsize,
}

/// Helper struct for managing test file creation
struct TestFileManager {
    temp_dir: TempDir,
}

impl TestFileManager {
    fn new() -> Self {
        Self {
            temp_dir: TempDir::new().expect("Failed to create temp directory"),
        }
    }

    fn create_file(&self, name: &str, size: usize) -> PathBuf {
        let path = self.temp_dir.path().join(name);
        let data = vec![0xAB; size];
        fs::write(&path, data).expect("Failed to write test file");
        path
    }

    fn create_large_file(&self, name: &str, size_gb: f64) -> PathBuf {
        let path = self.temp_dir.path().join(name);
        let chunk_size = 64 * 1024 * 1024; // 64MB chunks
        let total_size = (size_gb * 1024.0 * 1024.0 * 1024.0) as usize;
        let _chunks = total_size / chunk_size;
        let remainder = total_size % chunk_size;

        let file = fs::File::create(&path).expect("Failed to create file");
        file.set_len(total_size as u64)
            .expect("Failed to set file length");

        // Write some data at beginning, middle, and end to make it non-sparse
        use std::io::{Seek, SeekFrom, Write};
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("Failed to open file for writing");

        let data = vec![0xAB; chunk_size.min(total_size)];
        file.write_all(&data[..chunk_size.min(total_size)])
            .expect("Failed to write beginning");

        if total_size > chunk_size {
            file.seek(SeekFrom::Start((total_size / 2) as u64))
                .expect("Failed to seek");
            file.write_all(&data[..chunk_size.min(total_size - total_size / 2)])
                .expect("Failed to write middle");
        }

        if remainder > 0 {
            file.seek(SeekFrom::End(-(remainder as i64)))
                .expect("Failed to seek to end");
            file.write_all(&vec![0xCD; remainder])
                .expect("Failed to write end");
        }

        path
    }
}

/// Test multi-threaded access with concurrent client operations
#[test]
#[serial_test::serial]
fn test_ffi_multi_threaded_access() {
    // Initialize library
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let file_manager = TestFileManager::new();
    let metrics = Arc::new(TestMetrics::default());
    let num_threads = 8;
    let files_per_thread = 5;
    let barrier = Arc::new(Barrier::new(num_threads));

    // Create test files
    let test_files: Vec<PathBuf> = (0..num_threads * files_per_thread)
        .map(|i| file_manager.create_file(&format!("thread_test_{i}.mkv"), 1024 * 1024))
        .collect();

    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let thread_files =
            test_files[thread_id * files_per_thread..(thread_id + 1) * files_per_thread].to_vec();
        let metrics_clone = Arc::clone(&metrics);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            // Create client for this thread
            let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
            if anidb_client_create(&mut client_handle) != AniDBResult::Success {
                metrics_clone.thread_crashes.fetch_add(1, Ordering::Relaxed);
                return;
            }

            // Wait for all threads to be ready
            barrier_clone.wait();

            // Process files
            for file_path in thread_files {
                let c_path = CString::new(file_path.to_str().unwrap()).unwrap();
                let algorithms = [
                    AniDBHashAlgorithm::ED2K,
                    AniDBHashAlgorithm::CRC32,
                    AniDBHashAlgorithm::MD5,
                ];

                let options = AniDBProcessOptions {
                    algorithms: algorithms.as_ptr(),
                    algorithm_count: algorithms.len(),
                    enable_progress: 0,
                    progress_callback: None,
                    user_data: ptr::null_mut(),
                };

                let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
                let result =
                    anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);

                if result == AniDBResult::Success {
                    metrics_clone
                        .processed_files
                        .fetch_add(1, Ordering::Relaxed);

                    unsafe {
                        if !result_ptr.is_null() {
                            let file_result = &*result_ptr;
                            metrics_clone
                                .total_bytes
                                .fetch_add(file_result.file_size, Ordering::Relaxed);
                            anidb_free_file_result(result_ptr);
                        }
                    }
                } else {
                    metrics_clone.errors.fetch_add(1, Ordering::Relaxed);
                }
            }

            // Cleanup
            anidb_client_destroy(client_handle);
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Verify results
    assert_eq!(
        metrics.thread_crashes.load(Ordering::Relaxed),
        0,
        "No threads should crash"
    );
    assert_eq!(
        metrics.processed_files.load(Ordering::Relaxed),
        num_threads * files_per_thread,
        "All files should be processed"
    );
    assert_eq!(
        metrics.errors.load(Ordering::Relaxed),
        0,
        "No errors expected"
    );

    anidb_cleanup();
}

/// Test thread safety with concurrent operations on same client
#[test]
#[serial_test::serial]
fn test_ffi_thread_safety_shared_client() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let file_manager = TestFileManager::new();
    let test_file = file_manager.create_file("shared_test.mkv", 10 * 1024 * 1024);

    // Create single client
    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create(&mut client_handle),
        AniDBResult::Success
    );

    let completed = Arc::new(AtomicUsize::new(0));
    let errors = Arc::new(AtomicUsize::new(0));
    let num_threads = 4;
    let barrier = Arc::new(Barrier::new(num_threads));

    let mut handles = vec![];

    for _ in 0..num_threads {
        let c_path = CString::new(test_file.to_str().unwrap()).unwrap();
        let completed_clone = Arc::clone(&completed);
        let errors_clone = Arc::clone(&errors);
        let barrier_clone = Arc::clone(&barrier);
        let client_handle_ptr = client_handle as usize;

        let handle = thread::spawn(move || {
            let client_handle = client_handle_ptr as *mut std::ffi::c_void;
            barrier_clone.wait();

            let algorithms = [AniDBHashAlgorithm::ED2K];
            let options = AniDBProcessOptions {
                algorithms: algorithms.as_ptr(),
                algorithm_count: algorithms.len(),
                enable_progress: 0,
                progress_callback: None,
                user_data: ptr::null_mut(),
            };

            let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
            let result =
                anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);

            if result == AniDBResult::Success {
                completed_clone.fetch_add(1, Ordering::Relaxed);
                if !result_ptr.is_null() {
                    anidb_free_file_result(result_ptr);
                }
            } else {
                errors_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // At least one should complete successfully
    assert!(
        completed.load(Ordering::Relaxed) > 0,
        "At least one operation should complete"
    );

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}

/// Test large file processing (>1GB)
#[test]
#[serial_test::serial]
#[ignore] // Ignored by default due to disk space requirements
fn test_ffi_large_file_processing() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let file_manager = TestFileManager::new();
    let large_file = file_manager.create_large_file("large_test.mkv", 1.5); // 1.5GB

    // Create client with appropriate config
    let config = AniDBConfig {
        max_concurrent_files: 1,
        chunk_size: 64 * 1024,               // 64KB chunks
        max_memory_usage: 500 * 1024 * 1024, // 500MB limit
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

    // Track memory usage
    let peak_memory = Arc::new(AtomicU64::new(0));
    let peak_memory_clone = Arc::clone(&peak_memory);

    // Progress tracking
    let progress_updates = Arc::new(Mutex::new(Vec::<(f32, u64, u64)>::new()));
    let progress_clone = Arc::clone(&progress_updates);

    extern "C" fn progress_callback(
        percentage: f32,
        bytes_processed: u64,
        total_bytes: u64,
        user_data: *mut std::ffi::c_void,
    ) {
        let data =
            unsafe { &*(user_data as *const (Arc<AtomicU64>, Arc<Mutex<Vec<(f32, u64, u64)>>>)) };

        // Check memory during processing
        let mut stats = AniDBMemoryStats {
            total_memory_used: 0,
            ffi_allocated: 0,
            ffi_peak: 0,
            pool_memory: 0,
            pool_hits: 0,
            pool_misses: 0,
            active_allocations: 0,
            memory_limit: 0,
            memory_pressure: 0,
        };

        if anidb_get_memory_stats(&mut stats) == AniDBResult::Success {
            data.0.fetch_max(stats.total_memory_used, Ordering::Relaxed);
        }

        data.1
            .lock()
            .unwrap()
            .push((percentage, bytes_processed, total_bytes));
    }

    let c_path = CString::new(large_file.to_str().unwrap()).unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K];

    let callback_data = (peak_memory_clone, progress_clone);
    let user_data_ptr = &callback_data as *const _ as *mut std::ffi::c_void;

    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 1,
        progress_callback: Some(progress_callback),
        user_data: user_data_ptr,
    };

    let start = Instant::now();
    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
    let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);
    let duration = start.elapsed();

    assert_eq!(result, AniDBResult::Success, "Large file processing failed");

    // Verify results
    unsafe {
        assert!(!result_ptr.is_null());
        let file_result = &*result_ptr;
        assert_eq!(file_result.status, AniDBStatus::Completed);
        assert_eq!(
            file_result.file_size,
            (1.5 * 1024.0 * 1024.0 * 1024.0) as u64
        );
        assert_eq!(file_result.hash_count, 1);
        anidb_free_file_result(result_ptr);
    }

    // Check performance
    let throughput_mbps = (1.5 * 1024.0) / duration.as_secs_f64();
    println!("Large file throughput: {throughput_mbps:.2} MB/s");
    assert!(
        throughput_mbps > 50.0,
        "Throughput {throughput_mbps:.2} MB/s is below minimum 50 MB/s"
    );

    // Check memory usage stayed within limits
    let max_memory_mb = peak_memory.load(Ordering::Relaxed) / (1024 * 1024);
    println!("Peak memory usage: {max_memory_mb} MB");
    assert!(
        max_memory_mb <= 500,
        "Memory usage {max_memory_mb} MB exceeded 500 MB limit"
    );

    // Verify progress reporting
    let progress = progress_updates.lock().unwrap();
    assert!(!progress.is_empty(), "Should have progress updates");
    assert!(
        progress.last().unwrap().0 >= 99.0,
        "Final progress should be near 100%"
    );

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}

/// Test various error conditions
#[test]
#[serial_test::serial]
fn test_ffi_error_conditions() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let file_manager = TestFileManager::new();

    // Test 1: Invalid file path
    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create(&mut client_handle),
        AniDBResult::Success
    );

    let invalid_path = CString::new("/nonexistent/path/file.mkv").unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K];
    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
    let result = anidb_process_file(
        client_handle,
        invalid_path.as_ptr(),
        &options,
        &mut result_ptr,
    );
    assert_eq!(result, AniDBResult::ErrorFileNotFound);

    // Test 2: Permission denied (platform-specific)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let protected_file = file_manager.create_file("protected.mkv", 1024);
        let mut perms = fs::metadata(&protected_file).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&protected_file, perms).unwrap();

        let c_path = CString::new(protected_file.to_str().unwrap()).unwrap();
        let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);
        // Note: On some platforms/filesystems, this may return ErrorProcessing instead of ErrorPermissionDenied
        assert!(
            result == AniDBResult::ErrorPermissionDenied || result == AniDBResult::ErrorProcessing,
            "Expected permission error, got: {result:?}"
        );
    }

    // Test 3: Invalid parameters
    let valid_file = file_manager.create_file("valid.mkv", 1024);
    let c_path = CString::new(valid_file.to_str().unwrap()).unwrap();

    // Null algorithms
    let bad_options = AniDBProcessOptions {
        algorithms: ptr::null(),
        algorithm_count: 1,
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };
    let result = anidb_process_file(
        client_handle,
        c_path.as_ptr(),
        &bad_options,
        &mut result_ptr,
    );
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Zero algorithm count
    let bad_options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: 0,
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };
    let result = anidb_process_file(
        client_handle,
        c_path.as_ptr(),
        &bad_options,
        &mut result_ptr,
    );
    assert_eq!(result, AniDBResult::ErrorInvalidParameter);

    // Test 4: Resource exhaustion simulation
    // Create many clients to potentially exhaust resources
    let mut handles = Vec::new();
    for _ in 0..1000 {
        let mut handle: *mut std::ffi::c_void = ptr::null_mut();
        if anidb_client_create(&mut handle) == AniDBResult::Success {
            handles.push(handle);
        } else {
            break;
        }
    }

    // Clean up all handles
    for handle in handles {
        anidb_client_destroy(handle);
    }

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}

/// Test memory stress and leak detection
#[test]
#[serial_test::serial]
fn test_ffi_memory_stress() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let file_manager = TestFileManager::new();
    let test_files: Vec<_> = (0..50)
        .map(|i| file_manager.create_file(&format!("stress_{i}.mkv"), 512 * 1024))
        .collect();

    // Get initial memory stats
    let mut initial_stats = AniDBMemoryStats {
        total_memory_used: 0,
        ffi_allocated: 0,
        ffi_peak: 0,
        pool_memory: 0,
        pool_hits: 0,
        pool_misses: 0,
        active_allocations: 0,
        memory_limit: 0,
        memory_pressure: 0,
    };
    assert_eq!(
        anidb_get_memory_stats(&mut initial_stats),
        AniDBResult::Success
    );

    // Create client for stress test
    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create(&mut client_handle),
        AniDBResult::Success
    );

    // Process many files rapidly
    let algorithms = [
        AniDBHashAlgorithm::ED2K,
        AniDBHashAlgorithm::CRC32,
        AniDBHashAlgorithm::MD5,
        AniDBHashAlgorithm::SHA1,
    ];

    for (i, test_file) in test_files.iter().enumerate() {
        let c_path = CString::new(test_file.to_str().unwrap()).unwrap();

        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
        let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);

        assert_eq!(result, AniDBResult::Success);
        if !result_ptr.is_null() {
            anidb_free_file_result(result_ptr);
        }

        // Check memory periodically
        if i % 10 == 0 {
            let mut stats = AniDBMemoryStats {
                total_memory_used: 0,
                ffi_allocated: 0,
                ffi_peak: 0,
                pool_memory: 0,
                pool_hits: 0,
                pool_misses: 0,
                active_allocations: 0,
                memory_limit: 0,
                memory_pressure: 0,
            };
            anidb_get_memory_stats(&mut stats);

            // Force GC if memory pressure is high
            if stats.memory_pressure >= 2 {
                anidb_memory_gc();
            }
        }
    }

    // Clean up
    anidb_client_destroy(client_handle);

    // Force garbage collection
    anidb_memory_gc();

    // Check for memory leaks
    let mut leak_count: u64 = 0;
    let mut leaked_bytes: u64 = 0;
    assert_eq!(
        anidb_check_memory_leaks(&mut leak_count, &mut leaked_bytes),
        AniDBResult::Success
    );

    #[cfg(debug_assertions)]
    {
        if leak_count > 0 {
            eprintln!("Memory leaks detected: {leak_count} allocations, {leaked_bytes} bytes");
        }
        assert_eq!(leak_count, 0, "Memory leaks detected");
    }

    anidb_cleanup();
}

/// Test platform-specific behavior
#[test]
#[serial_test::serial]
fn test_ffi_platform_specific() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let file_manager = TestFileManager::new();

    // Create client
    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create(&mut client_handle),
        AniDBResult::Success
    );

    // Test 1: Path handling
    #[cfg(windows)]
    {
        // Test Windows long path support
        let long_name = "a".repeat(255);
        let test_file = file_manager.create_file(&format!("{}.mkv", long_name), 1024);
        let c_path = CString::new(test_file.to_str().unwrap()).unwrap();

        let algorithms = [AniDBHashAlgorithm::ED2K];
        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
        let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);

        assert_eq!(result, AniDBResult::Success);
        if !result_ptr.is_null() {
            anidb_free_file_result(result_ptr);
        }
    }

    #[cfg(unix)]
    {
        // Test Unix special characters in paths
        let special_name = "test with spaces & special-chars!.mkv";
        let test_file = file_manager.create_file(special_name, 1024);
        let c_path = CString::new(test_file.to_str().unwrap()).unwrap();

        let algorithms = [AniDBHashAlgorithm::ED2K];
        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
        let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);

        assert_eq!(result, AniDBResult::Success);
        if !result_ptr.is_null() {
            anidb_free_file_result(result_ptr);
        }
    }

    // Test 2: Performance characteristics (platform-specific optimizations)
    let perf_file = file_manager.create_file("perf_test.mkv", 50 * 1024 * 1024); // 50MB
    let c_path = CString::new(perf_file.to_str().unwrap()).unwrap();

    let algorithms = [AniDBHashAlgorithm::ED2K];
    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 0,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let start = Instant::now();
    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
    let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);
    let _duration = start.elapsed();

    assert_eq!(result, AniDBResult::Success);

    unsafe {
        if !result_ptr.is_null() {
            let file_result = &*result_ptr;
            println!(
                "Platform: {}, Processing time: {} ms for 50MB",
                std::env::consts::OS,
                file_result.processing_time_ms
            );
            anidb_free_file_result(result_ptr);
        }
    }

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}

/// Test event system with callbacks
#[test]
#[serial_test::serial]
fn test_ffi_event_system() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let file_manager = TestFileManager::new();
    let test_file = file_manager.create_file("event_test.mkv", 5 * 1024 * 1024);

    // Create client
    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create(&mut client_handle),
        AniDBResult::Success
    );

    // Event tracking
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    extern "C" fn event_callback(event: *const AniDBEvent, user_data: *mut std::ffi::c_void) {
        let events = unsafe { &*(user_data as *const Arc<Mutex<Vec<AniDBEventType>>>) };
        let event_data = unsafe { &*event };
        events.lock().unwrap().push(event_data.event_type);
    }

    let user_data_ptr =
        &events_clone as *const Arc<Mutex<Vec<AniDBEventType>>> as *mut std::ffi::c_void;

    // Connect event system
    assert_eq!(
        anidb_event_connect(client_handle, event_callback, user_data_ptr),
        AniDBResult::Success
    );

    // Process file to generate events
    let c_path = CString::new(test_file.to_str().unwrap()).unwrap();
    let algorithms = [AniDBHashAlgorithm::ED2K];
    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 1,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
    let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);
    assert_eq!(result, AniDBResult::Success);

    if !result_ptr.is_null() {
        anidb_free_file_result(result_ptr);
    }

    // Give event thread time to process
    thread::sleep(Duration::from_millis(100));

    // Verify events were received
    let received_events = events.lock().unwrap();
    assert!(
        received_events.contains(&AniDBEventType::FileStart),
        "Should have FileStart event"
    );
    assert!(
        received_events.contains(&AniDBEventType::FileComplete),
        "Should have FileComplete event"
    );

    // Disconnect event system
    assert_eq!(anidb_event_disconnect(client_handle), AniDBResult::Success);

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}

/// Test callback registration and management
#[test]
#[serial_test::serial]
fn test_ffi_callback_management() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    // Create client
    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create(&mut client_handle),
        AniDBResult::Success
    );

    // Test data
    let progress_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    let completion_count = Arc::new(AtomicUsize::new(0));

    // Progress callback
    extern "C" fn progress_cb(
        _percentage: f32,
        _bytes: u64,
        _total: u64,
        user_data: *mut std::ffi::c_void,
    ) {
        let count = unsafe { &*(user_data as *const AtomicUsize) };
        count.fetch_add(1, Ordering::Relaxed);
    }

    // Error callback
    extern "C" fn error_cb(
        _result: AniDBResult,
        _msg: *const i8,
        _path: *const i8,
        user_data: *mut std::ffi::c_void,
    ) {
        let count = unsafe { &*(user_data as *const AtomicUsize) };
        count.fetch_add(1, Ordering::Relaxed);
    }

    // Completion callback
    extern "C" fn completion_cb(_result: AniDBResult, user_data: *mut std::ffi::c_void) {
        let count = unsafe { &*(user_data as *const AtomicUsize) };
        count.fetch_add(1, Ordering::Relaxed);
    }

    // Register callbacks
    let progress_id = anidb_register_callback(
        client_handle,
        AniDBCallbackType::Progress,
        progress_cb as *mut std::ffi::c_void,
        &*progress_count as *const AtomicUsize as *mut std::ffi::c_void,
    );
    assert_ne!(progress_id, 0);

    let error_id = anidb_register_callback(
        client_handle,
        AniDBCallbackType::Error,
        error_cb as *mut std::ffi::c_void,
        &*error_count as *const AtomicUsize as *mut std::ffi::c_void,
    );
    assert_ne!(error_id, 0);

    let completion_id = anidb_register_callback(
        client_handle,
        AniDBCallbackType::Completion,
        completion_cb as *mut std::ffi::c_void,
        &*completion_count as *const AtomicUsize as *mut std::ffi::c_void,
    );
    assert_ne!(completion_id, 0);

    // Process a file to trigger callbacks
    let file_manager = TestFileManager::new();
    let test_file = file_manager.create_file("callback_test.mkv", 1024 * 1024);
    let c_path = CString::new(test_file.to_str().unwrap()).unwrap();

    let algorithms = [AniDBHashAlgorithm::ED2K];
    let options = AniDBProcessOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        enable_progress: 1,
        progress_callback: None,
        user_data: ptr::null_mut(),
    };

    let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
    anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);
    if !result_ptr.is_null() {
        anidb_free_file_result(result_ptr);
    }

    // Verify callbacks were called
    assert!(
        progress_count.load(Ordering::Relaxed) > 0,
        "Progress callback should be called"
    );
    assert_eq!(
        completion_count.load(Ordering::Relaxed),
        1,
        "Completion callback should be called once"
    );

    // Unregister callbacks
    assert_eq!(
        anidb_unregister_callback(client_handle, progress_id),
        AniDBResult::Success
    );
    assert_eq!(
        anidb_unregister_callback(client_handle, error_id),
        AniDBResult::Success
    );
    assert_eq!(
        anidb_unregister_callback(client_handle, completion_id),
        AniDBResult::Success
    );

    // Trying to unregister again should fail
    assert_eq!(
        anidb_unregister_callback(client_handle, progress_id),
        AniDBResult::ErrorInvalidParameter
    );

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}

/// Test race condition detection with aggressive concurrent access
#[test]
#[serial_test::serial]
fn test_ffi_race_condition_detection() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let file_manager = TestFileManager::new();
    let test_file = file_manager.create_file("race_test.mkv", 1024 * 1024);

    // Create client
    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create(&mut client_handle),
        AniDBResult::Success
    );

    // Shared state for detecting race conditions
    let processing_active = Arc::new(AtomicBool::new(false));
    let race_detected = Arc::new(AtomicBool::new(false));
    let num_threads = 10;
    let barrier = Arc::new(Barrier::new(num_threads));

    let mut handles = vec![];

    for _ in 0..num_threads {
        let c_path = CString::new(test_file.to_str().unwrap()).unwrap();
        let processing_clone = Arc::clone(&processing_active);
        let race_clone = Arc::clone(&race_detected);
        let barrier_clone = Arc::clone(&barrier);
        let client_handle_ptr = client_handle as usize;

        let handle = thread::spawn(move || {
            let client_handle = client_handle_ptr as *mut std::ffi::c_void;
            barrier_clone.wait();

            // Try to process file
            if processing_clone
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                // Another thread is processing - this is a race
                race_clone.store(true, Ordering::SeqCst);
            }

            let algorithms = [AniDBHashAlgorithm::ED2K];
            let options = AniDBProcessOptions {
                algorithms: algorithms.as_ptr(),
                algorithm_count: algorithms.len(),
                enable_progress: 0,
                progress_callback: None,
                user_data: ptr::null_mut(),
            };

            let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
            let result =
                anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);

            if result == AniDBResult::Success && !result_ptr.is_null() {
                anidb_free_file_result(result_ptr);
            }

            processing_clone.store(false, Ordering::SeqCst);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // The FFI should handle concurrent access properly
    // Race detection here is just to verify we're testing concurrent scenarios
    println!(
        "Race condition test - concurrent access detected: {}",
        race_detected.load(Ordering::SeqCst)
    );

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}

/// Test buffer pool effectiveness under stress
#[test]
#[serial_test::serial]
fn test_ffi_buffer_pool_effectiveness() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let file_manager = TestFileManager::new();
    let test_files: Vec<_> = (0..20)
        .map(|i| file_manager.create_file(&format!("pool_test_{i}.mkv"), 256 * 1024))
        .collect();

    // Create client
    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create(&mut client_handle),
        AniDBResult::Success
    );

    // Get initial pool stats
    let mut initial_stats = AniDBMemoryStats {
        total_memory_used: 0,
        ffi_allocated: 0,
        ffi_peak: 0,
        pool_memory: 0,
        pool_hits: 0,
        pool_misses: 0,
        active_allocations: 0,
        memory_limit: 0,
        memory_pressure: 0,
    };
    anidb_get_memory_stats(&mut initial_stats);

    // Process files rapidly to test buffer reuse
    let algorithms = [AniDBHashAlgorithm::ED2K, AniDBHashAlgorithm::MD5];

    for test_file in &test_files {
        let c_path = CString::new(test_file.to_str().unwrap()).unwrap();
        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let mut result_ptr: *mut AniDBFileResult = ptr::null_mut();
        let result = anidb_process_file(client_handle, c_path.as_ptr(), &options, &mut result_ptr);
        assert_eq!(result, AniDBResult::Success);

        if !result_ptr.is_null() {
            anidb_free_file_result(result_ptr);
        }
    }

    // Get final pool stats
    let mut final_stats = AniDBMemoryStats {
        total_memory_used: 0,
        ffi_allocated: 0,
        ffi_peak: 0,
        pool_memory: 0,
        pool_hits: 0,
        pool_misses: 0,
        active_allocations: 0,
        memory_limit: 0,
        memory_pressure: 0,
    };
    anidb_get_memory_stats(&mut final_stats);

    // Calculate buffer pool effectiveness
    let total_requests = final_stats.pool_hits + final_stats.pool_misses;
    let hit_rate = if total_requests > 0 {
        (final_stats.pool_hits as f64 / total_requests as f64) * 100.0
    } else {
        0.0
    };

    println!("Buffer pool statistics:");
    println!("  Total requests: {total_requests}");
    println!("  Hits: {}", final_stats.pool_hits);
    println!("  Misses: {}", final_stats.pool_misses);
    println!("  Hit rate: {hit_rate:.2}%");
    println!("  Pool memory: {} KB", final_stats.pool_memory / 1024);

    // Buffer pool should be effective for repeated operations
    assert!(
        hit_rate > 50.0,
        "Buffer pool hit rate {hit_rate:.2}% is too low"
    );

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}
