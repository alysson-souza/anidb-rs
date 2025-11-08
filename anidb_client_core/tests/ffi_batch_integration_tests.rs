//! Batch Processing Integration Tests for FFI
//!
//! Tests batch file processing capabilities through the FFI interface

use anidb_client_core::ffi::{
    AniDBBatchOptions, AniDBBatchResult, AniDBConfig, AniDBHashAlgorithm, AniDBResult,
    anidb_cleanup, anidb_client_create, anidb_client_create_with_config, anidb_client_destroy,
    anidb_init,
};
use std::ffi::CString;
use std::fs;
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Test batch processing with multiple files
#[test]
#[serial_test::serial]
fn test_ffi_batch_processing_basic() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let temp_dir = TempDir::new().unwrap();

    // Create test files
    let file_count = 10;
    let test_files: Vec<PathBuf> = (0..file_count)
        .map(|i| {
            let path = temp_dir.path().join(format!("batch_test_{i}.mkv"));
            let size = (i + 1) * 100 * 1024; // Varying sizes
            fs::write(&path, vec![0xAB; size]).unwrap();
            path
        })
        .collect();

    // Create client with config
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

    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create_with_config(&config, &mut client_handle),
        AniDBResult::Success
    );

    // Prepare batch processing
    let algorithms = [
        AniDBHashAlgorithm::ED2K,
        AniDBHashAlgorithm::CRC32,
        AniDBHashAlgorithm::MD5,
    ];

    // Progress tracking
    let progress_updates = Arc::new(Mutex::new(Vec::<(f32, u64, u64)>::new()));
    let completion_called = Arc::new(AtomicUsize::new(0));

    let progress_clone = Arc::clone(&progress_updates);
    let completion_clone = Arc::clone(&completion_called);

    extern "C" fn batch_progress_callback(
        percentage: f32,
        current_file: u64,
        total_files: u64,
        user_data: *mut std::ffi::c_void,
    ) {
        let data = unsafe { &*(user_data as *const Arc<Mutex<Vec<(f32, u64, u64)>>>) };
        data.lock()
            .unwrap()
            .push((percentage, current_file, total_files));
    }

    extern "C" fn batch_completion_callback(result: AniDBResult, user_data: *mut std::ffi::c_void) {
        let count = unsafe { &*(user_data as *const Arc<AtomicUsize>) };
        count.fetch_add(1, Ordering::Relaxed);
        assert_eq!(result, AniDBResult::Success);
    }

    let progress_ptr =
        &progress_clone as *const Arc<Mutex<Vec<(f32, u64, u64)>>> as *mut std::ffi::c_void;
    let completion_ptr = &completion_clone as *const Arc<AtomicUsize> as *mut std::ffi::c_void;

    let _batch_options = AniDBBatchOptions {
        algorithms: algorithms.as_ptr(),
        algorithm_count: algorithms.len(),
        max_concurrent: 4,
        continue_on_error: 1,
        skip_existing: 0,
        include_patterns: ptr::null(),
        include_pattern_count: 0,
        exclude_patterns: ptr::null(),
        exclude_pattern_count: 0,
        use_defaults: 1,
        progress_callback: Some(batch_progress_callback),
        completion_callback: Some(batch_completion_callback),
        user_data: progress_ptr,
    };

    // Convert file paths to C strings
    let c_paths: Vec<CString> = test_files
        .iter()
        .map(|p| CString::new(p.to_str().unwrap()).unwrap())
        .collect();
    let _c_path_ptrs: Vec<*const i8> = c_paths.iter().map(|s| s.as_ptr()).collect();

    // Execute batch processing
    let start = Instant::now();
    let _batch_result: *mut AniDBBatchResult = ptr::null_mut();

    // Note: The actual batch processing function needs to be implemented in the FFI
    // For now, we'll test the individual components

    // Process files sequentially as a batch simulation
    let mut successful = 0;
    let mut failed = 0;
    let mut file_results = Vec::new();

    for (i, c_path) in c_paths.iter().enumerate() {
        let options = anidb_client_core::ffi::AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let mut result_ptr: *mut anidb_client_core::ffi::AniDBFileResult = ptr::null_mut();
        let result = anidb_client_core::ffi::anidb_process_file(
            client_handle,
            c_path.as_ptr(),
            &options,
            &mut result_ptr,
        );

        if result == AniDBResult::Success {
            successful += 1;
            if !result_ptr.is_null() {
                file_results.push(result_ptr);
            }
        } else {
            failed += 1;
        }

        // Simulate batch progress
        let progress = ((i + 1) as f32 / file_count as f32) * 100.0;
        batch_progress_callback(progress, (i + 1) as u64, file_count as u64, progress_ptr);
    }

    let duration = start.elapsed();

    // Simulate completion callback
    batch_completion_callback(AniDBResult::Success, completion_ptr);

    // Verify results
    assert_eq!(successful, file_count);
    assert_eq!(failed, 0);
    assert_eq!(completion_called.load(Ordering::Relaxed), 1);

    // Check progress updates
    let progress = progress_updates.lock().unwrap();
    assert!(!progress.is_empty());
    assert_eq!(progress.last().unwrap().1, file_count as u64);

    println!(
        "Batch processing completed: {} files in {:.2}s",
        successful,
        duration.as_secs_f64()
    );

    // Clean up file results
    for result_ptr in file_results {
        anidb_client_core::ffi::anidb_free_file_result(result_ptr);
    }

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}

/// Test batch processing with error handling
#[test]
#[serial_test::serial]
fn test_ffi_batch_error_handling() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let temp_dir = TempDir::new().unwrap();

    // Create mix of valid and invalid files
    let mut test_paths = Vec::new();

    // Valid files
    for i in 0..5 {
        let path = temp_dir.path().join(format!("valid_{i}.mkv"));
        fs::write(&path, vec![0xAB; 1024]).unwrap();
        test_paths.push((path, true));
    }

    // Non-existent files
    for i in 0..3 {
        let path = temp_dir.path().join(format!("missing_{i}.mkv"));
        test_paths.push((path, false));
    }

    // Create client
    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create(&mut client_handle),
        AniDBResult::Success
    );

    let algorithms = [AniDBHashAlgorithm::ED2K];
    let mut successful = 0;
    let mut failed = 0;
    let errors = Arc::new(Mutex::new(Vec::new()));

    // Process with continue_on_error enabled
    for (path, should_succeed) in &test_paths {
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let options = anidb_client_core::ffi::AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let mut result_ptr: *mut anidb_client_core::ffi::AniDBFileResult = ptr::null_mut();
        let result = anidb_client_core::ffi::anidb_process_file(
            client_handle,
            c_path.as_ptr(),
            &options,
            &mut result_ptr,
        );

        if result == AniDBResult::Success {
            successful += 1;
            assert!(*should_succeed, "File should have failed: {path:?}");
            if !result_ptr.is_null() {
                anidb_client_core::ffi::anidb_free_file_result(result_ptr);
            }
        } else {
            failed += 1;
            assert!(!*should_succeed, "File should have succeeded: {path:?}");
            errors.lock().unwrap().push((path.clone(), result));
        }
    }

    // Verify error handling
    assert_eq!(successful, 5);
    assert_eq!(failed, 3);

    let error_list = errors.lock().unwrap();
    for (path, result) in error_list.iter() {
        assert_eq!(*result, AniDBResult::ErrorFileNotFound);
        println!("Expected error for: {path:?}");
    }

    anidb_client_destroy(client_handle);
    anidb_cleanup();
}

/// Test concurrent batch processing
#[test]
#[serial_test::serial]
fn test_ffi_concurrent_batch_processing() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let temp_dir = TempDir::new().unwrap();
    let num_batches = 4;
    let files_per_batch = 5;

    // Create test files for each batch
    let mut batch_files = Vec::new();
    for batch in 0..num_batches {
        let mut files = Vec::new();
        for file in 0..files_per_batch {
            let path = temp_dir.path().join(format!("batch{batch}_file{file}.mkv"));
            fs::write(&path, vec![0xAB; 512 * 1024]).unwrap();
            files.push(path);
        }
        batch_files.push(files);
    }

    // Create multiple clients for concurrent batches
    let mut handles = Vec::new();

    for (batch_id, files) in batch_files.into_iter().enumerate() {
        let handle = std::thread::spawn(move || {
            let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
            assert_eq!(
                anidb_client_create(&mut client_handle),
                AniDBResult::Success
            );

            let algorithms = [AniDBHashAlgorithm::ED2K, AniDBHashAlgorithm::CRC32];
            let mut processed = 0;

            for file in files {
                let c_path = CString::new(file.to_str().unwrap()).unwrap();
                let options = anidb_client_core::ffi::AniDBProcessOptions {
                    algorithms: algorithms.as_ptr(),
                    algorithm_count: algorithms.len(),
                    enable_progress: 0,
                    progress_callback: None,
                    user_data: ptr::null_mut(),
                };

                let mut result_ptr: *mut anidb_client_core::ffi::AniDBFileResult = ptr::null_mut();
                let result = anidb_client_core::ffi::anidb_process_file(
                    client_handle,
                    c_path.as_ptr(),
                    &options,
                    &mut result_ptr,
                );

                if result == AniDBResult::Success {
                    processed += 1;
                    if !result_ptr.is_null() {
                        anidb_client_core::ffi::anidb_free_file_result(result_ptr);
                    }
                }
            }

            anidb_client_destroy(client_handle);

            println!("Batch {batch_id} processed {processed} files");
            processed
        });

        handles.push(handle);
    }

    // Wait for all batches to complete
    let mut total_processed = 0;
    for handle in handles {
        total_processed += handle.join().expect("Batch thread panicked");
    }

    assert_eq!(
        total_processed,
        num_batches * files_per_batch,
        "All files should be processed"
    );

    anidb_cleanup();
}

/// Test batch processing with memory constraints
#[test]
#[serial_test::serial]
fn test_ffi_batch_memory_constraints() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let temp_dir = TempDir::new().unwrap();

    // Create files that will stress memory when processed concurrently
    let file_count = 20;
    let file_size = 10 * 1024 * 1024; // 10MB each
    let test_files: Vec<PathBuf> = (0..file_count)
        .map(|i| {
            let path = temp_dir.path().join(format!("memory_test_{i}.mkv"));
            fs::write(&path, vec![0xAB; file_size]).unwrap();
            path
        })
        .collect();

    // Create client with strict memory limit
    let config = AniDBConfig {
        max_concurrent_files: 2, // Limit concurrent processing
        chunk_size: 64 * 1024,
        max_memory_usage: 100 * 1024 * 1024, // 100MB limit (tight for 20x10MB files)
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

    // Track memory usage during batch processing
    let peak_memory = Arc::new(AtomicUsize::new(0));
    let algorithms = [
        AniDBHashAlgorithm::ED2K,
        AniDBHashAlgorithm::MD5,
        AniDBHashAlgorithm::SHA1,
    ];

    // Process files with memory monitoring
    for (i, file) in test_files.iter().enumerate() {
        // Check memory before processing
        let mut stats = anidb_client_core::ffi::AniDBMemoryStats {
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
        anidb_client_core::ffi::anidb_get_memory_stats(&mut stats);

        if stats.memory_pressure >= 2 {
            // High memory pressure - trigger GC
            anidb_client_core::ffi::anidb_memory_gc();
            println!("Triggered GC at file {i} due to memory pressure");
        }

        peak_memory.fetch_max(stats.total_memory_used as usize, Ordering::Relaxed);

        // Process file
        let c_path = CString::new(file.to_str().unwrap()).unwrap();
        let options = anidb_client_core::ffi::AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let mut result_ptr: *mut anidb_client_core::ffi::AniDBFileResult = ptr::null_mut();
        let result = anidb_client_core::ffi::anidb_process_file(
            client_handle,
            c_path.as_ptr(),
            &options,
            &mut result_ptr,
        );

        assert_eq!(result, AniDBResult::Success, "File {i} processing failed");

        if !result_ptr.is_null() {
            anidb_client_core::ffi::anidb_free_file_result(result_ptr);
        }
    }

    // Verify memory stayed within limits
    let max_memory_mb = peak_memory.load(Ordering::Relaxed) / (1024 * 1024);
    println!("Peak memory during batch: {max_memory_mb} MB");
    assert!(
        max_memory_mb <= 100,
        "Memory usage {max_memory_mb} MB exceeded 100 MB limit"
    );

    anidb_client_destroy(client_handle);
    // Note: We don't call anidb_cleanup() here to avoid interfering with other tests
    // that might be running in parallel. The cleanup will happen when the process exits.
}

/// Test batch processing multiple times without relying on core-level caching.
///
/// Hash caching now lives in higher-layer clients (e.g., CLI). This test simply ensures that
/// repeated processing of the same files through the FFI remains stable and deterministic.
#[test]
#[serial_test::serial]
fn test_ffi_batch_repeat_processing_without_cache() {
    assert_eq!(anidb_init(1), AniDBResult::Success);

    let temp_dir = TempDir::new().unwrap();

    // Create test files that are large enough to show a difference in processing time
    let test_files: Vec<PathBuf> = (0..5)
        .map(|i| {
            let path = temp_dir.path().join(format!("cached_test_{i}.mkv"));
            // Create 1MB files to ensure processing time is measurable
            let content = vec![0xAB; 1024 * 1024];
            fs::write(&path, content).unwrap();
            path
        })
        .collect();

    // Create client with caching enabled
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

    let mut client_handle: *mut std::ffi::c_void = ptr::null_mut();
    assert_eq!(
        anidb_client_create_with_config(&config, &mut client_handle),
        AniDBResult::Success
    );

    let algorithms = [AniDBHashAlgorithm::ED2K];

    // First pass: Process all files to populate cache
    let mut first_pass_time = Duration::default();
    for file in &test_files {
        let c_path = CString::new(file.to_str().unwrap()).unwrap();
        let options = anidb_client_core::ffi::AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let start = Instant::now();
        let mut result_ptr: *mut anidb_client_core::ffi::AniDBFileResult = ptr::null_mut();
        let result = anidb_client_core::ffi::anidb_process_file(
            client_handle,
            c_path.as_ptr(),
            &options,
            &mut result_ptr,
        );
        first_pass_time += start.elapsed();

        assert_eq!(result, AniDBResult::Success);
        if !result_ptr.is_null() {
            anidb_client_core::ffi::anidb_free_file_result(result_ptr);
        }
    }

    // Second pass: Process the same files again to ensure repeated runs remain stable
    let mut second_pass_time = Duration::default();

    for file in &test_files {
        let c_path = CString::new(file.to_str().unwrap()).unwrap();
        let options = anidb_client_core::ffi::AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        let start = Instant::now();
        let mut result_ptr: *mut anidb_client_core::ffi::AniDBFileResult = ptr::null_mut();
        let result = anidb_client_core::ffi::anidb_process_file(
            client_handle,
            c_path.as_ptr(),
            &options,
            &mut result_ptr,
        );
        let elapsed = start.elapsed();
        second_pass_time += elapsed;

        assert_eq!(result, AniDBResult::Success);

        if !result_ptr.is_null() {
            anidb_client_core::ffi::anidb_free_file_result(result_ptr);
        }
    }

    println!(
        "First pass: {:.2}ms, Second pass: {:.2}ms (cache handled by CLI)",
        first_pass_time.as_secs_f64() * 1000.0,
        second_pass_time.as_secs_f64() * 1000.0,
    );

    anidb_client_destroy(client_handle);
    // Note: We don't call anidb_cleanup() here to avoid interfering with other tests
    // that might be running in parallel. The cleanup will happen when the process exits.
}
