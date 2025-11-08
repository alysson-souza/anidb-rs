//! FFI Performance Benchmarks
//!
//! Comprehensive benchmarks for FFI operations to measure and optimize:
//! - Function call overhead
//! - String conversion performance
//! - Memory allocation efficiency
//! - Callback invocation speed
//! - Parallel processing through FFI

use anidb_client_core::ffi::*;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::ffi::{CStr, CString};
use std::hint::black_box;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

/// Benchmark FFI function call overhead
fn benchmark_ffi_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("ffi_overhead");

    // Initialize library
    anidb_init(1);

    // Benchmark simple function calls
    group.bench_function("get_version", |b| {
        b.iter(|| {
            let version = anidb_get_version();
            black_box(version);
        })
    });

    group.bench_function("get_abi_version", |b| {
        b.iter(|| {
            let version = anidb_get_abi_version();
            black_box(version);
        })
    });

    // Benchmark error string lookup
    group.bench_function("error_string_lookup", |b| {
        b.iter(|| {
            let error_str = anidb_error_string(AniDBResult::ErrorProcessing);
            black_box(error_str);
        })
    });

    // Benchmark hash algorithm name lookup
    group.bench_function("hash_algorithm_name", |b| {
        b.iter(|| {
            let name = anidb_hash_algorithm_name(AniDBHashAlgorithm::ED2K);
            black_box(name);
        })
    });

    group.finish();
}

/// Benchmark string conversion overhead
fn benchmark_string_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_conversion");

    let large_string = "x".repeat(1024);
    let test_strings = vec![
        ("small", "Hello, World!"),
        ("path", "/home/user/Documents/anime/series/episode01.mkv"),
        ("unicode", "アニメ エピソード 01 [1080p].mkv"),
        ("large", large_string.as_str()), // 1KB string
    ];

    for (name, test_str) in test_strings {
        let len = test_str.len();
        group.throughput(Throughput::Bytes(len as u64));

        // Benchmark C string creation
        group.bench_with_input(BenchmarkId::new("cstring_new", name), test_str, |b, s| {
            b.iter(|| {
                let c_str = CString::new(s).unwrap();
                black_box(c_str);
            })
        });

        // Benchmark FFI string allocation
        group.bench_with_input(
            BenchmarkId::new("ffi_allocate_string", name),
            test_str,
            |b, s| {
                b.iter(|| {
                    let ptr = anidb_client_core::ffi_memory::ffi_allocate_string(s);
                    anidb_free_string(ptr);
                    black_box(ptr);
                })
            },
        );

        // Benchmark string parsing from C
        let c_str = CString::new(test_str).unwrap();
        group.bench_with_input(
            BenchmarkId::new("parse_c_string", name),
            &c_str,
            |b, c_str| {
                b.iter(|| unsafe {
                    let rust_str = CStr::from_ptr(black_box(c_str.as_ptr())).to_str().unwrap();
                    black_box(rust_str);
                })
            },
        );
    }

    group.finish();
}

/// Benchmark client creation and destruction
fn benchmark_client_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("client_lifecycle");

    // Benchmark default client creation
    group.bench_function("client_create_default", |b| {
        b.iter(|| {
            let mut handle: *mut std::ffi::c_void = ptr::null_mut();
            let result = anidb_client_create(&mut handle);
            assert_eq!(result, AniDBResult::Success);
            anidb_client_destroy(handle);
        })
    });

    // Benchmark client creation with config
    group.bench_function("client_create_with_config", |b| {
        let config = AniDBConfig {
            max_concurrent_files: 4,
            chunk_size: 65536,
            max_memory_usage: 500_000_000,
            enable_debug_logging: 0,
            username: ptr::null(),
            password: ptr::null(),
            client_name: ptr::null(),
            client_version: ptr::null(),
        };

        b.iter(|| {
            let mut handle: *mut std::ffi::c_void = ptr::null_mut();
            let result = anidb_client_create_with_config(&config, &mut handle);
            assert_eq!(result, AniDBResult::Success);
            anidb_client_destroy(handle);
        })
    });

    group.finish();
}

/// Benchmark memory allocation patterns
fn benchmark_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation");

    // Different allocation sizes
    let sizes = vec![
        ("small", 64),     // Small strings
        ("medium", 1024),  // 1KB - typical paths
        ("large", 16384),  // 16KB - hash results
        ("xlarge", 65536), // 64KB - chunk buffers
    ];

    for (name, size) in sizes {
        group.throughput(Throughput::Bytes(size as u64));

        // Benchmark buffer allocation
        group.bench_with_input(
            BenchmarkId::new("buffer_allocate", name),
            &size,
            |b, &size| {
                use anidb_client_core::ffi_memory::{
                    AllocationType, ffi_allocate_buffer, ffi_release_buffer,
                };

                b.iter(|| {
                    let buffer =
                        ffi_allocate_buffer(black_box(size), AllocationType::Buffer).unwrap();
                    ffi_release_buffer(buffer);
                })
            },
        );

        // Benchmark string allocation of given size
        group.bench_with_input(
            BenchmarkId::new("string_allocate", name),
            &size,
            |b, &size| {
                let s = "x".repeat(size - 1); // -1 for null terminator

                b.iter(|| {
                    let ptr = anidb_client_core::ffi_memory::ffi_allocate_string(black_box(&s));
                    anidb_free_string(ptr);
                    black_box(ptr);
                })
            },
        );
    }

    // Benchmark allocation patterns (sequential vs interleaved)
    group.bench_function("sequential_allocations", |b| {
        use anidb_client_core::ffi_memory::{
            AllocationType, ffi_allocate_buffer, ffi_release_buffer,
        };

        b.iter(|| {
            let mut buffers = Vec::new();

            // Allocate all
            for _ in 0..100 {
                let buffer = ffi_allocate_buffer(1024, AllocationType::Buffer).unwrap();
                buffers.push(buffer);
            }

            // Release all
            for buffer in buffers {
                ffi_release_buffer(buffer);
            }
        })
    });

    group.bench_function("interleaved_allocations", |b| {
        use anidb_client_core::ffi_memory::{
            AllocationType, ffi_allocate_buffer, ffi_release_buffer,
        };

        b.iter(|| {
            for _ in 0..100 {
                let buffer = ffi_allocate_buffer(1024, AllocationType::Buffer).unwrap();
                ffi_release_buffer(buffer);
            }
        })
    });

    group.finish();
}

/// Benchmark callback invocation overhead
fn benchmark_callbacks(c: &mut Criterion) {
    let mut group = c.benchmark_group("callbacks");

    // Counter for callbacks
    static CALLBACK_COUNTER: AtomicU64 = AtomicU64::new(0);

    // Progress callback
    extern "C" fn progress_callback(
        _percentage: f32,
        _bytes_processed: u64,
        _total_bytes: u64,
        _user_data: *mut std::ffi::c_void,
    ) {
        CALLBACK_COUNTER.fetch_add(1, Ordering::Relaxed);
    }

    // Create client
    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    anidb_client_create(&mut handle);

    // Benchmark callback registration
    group.bench_function("callback_register", |b| {
        b.iter(|| {
            let id = anidb_register_callback(
                black_box(handle),
                AniDBCallbackType::Progress,
                progress_callback as *mut std::ffi::c_void,
                ptr::null_mut(),
            );
            anidb_unregister_callback(handle, id);
        })
    });

    // Benchmark callback invocation through file processing
    let test_file = create_test_file(1_048_576); // 1MB file
    let file_path = CString::new(test_file.path().to_str().unwrap()).unwrap();

    group.bench_function("callback_invocation", |b| {
        let algorithms = [AniDBHashAlgorithm::CRC32];
        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 1,
            progress_callback: Some(progress_callback),
            user_data: ptr::null_mut(),
        };

        b.iter(|| {
            CALLBACK_COUNTER.store(0, Ordering::Relaxed);

            let mut result: *mut AniDBFileResult = ptr::null_mut();
            let status = anidb_process_file(
                black_box(handle),
                black_box(file_path.as_ptr()),
                black_box(&options),
                &mut result,
            );

            assert_eq!(status, AniDBResult::Success);
            anidb_free_file_result(result);

            // Ensure callbacks were invoked
            assert!(CALLBACK_COUNTER.load(Ordering::Relaxed) > 0);
        })
    });

    // Cleanup
    anidb_client_destroy(handle);

    group.finish();
}

/// Benchmark file processing through FFI
fn benchmark_file_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_processing");

    // Create client
    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    anidb_client_create(&mut handle);

    // Test with different file sizes
    let sizes = vec![
        ("small", 1_024),      // 1KB
        ("medium", 1_048_576), // 1MB
        ("large", 10_485_760), // 10MB
    ];

    for (name, size) in sizes {
        let test_file = create_test_file(size);
        let file_path = CString::new(test_file.path().to_str().unwrap()).unwrap();

        group.throughput(Throughput::Bytes(size as u64));

        // Single algorithm
        group.bench_with_input(
            BenchmarkId::new("single_algorithm", name),
            &file_path,
            |b, file_path| {
                let algorithms = [AniDBHashAlgorithm::CRC32];
                let options = AniDBProcessOptions {
                    algorithms: algorithms.as_ptr(),
                    algorithm_count: algorithms.len(),
                    enable_progress: 0,
                    progress_callback: None,
                    user_data: ptr::null_mut(),
                };

                b.iter(|| {
                    let mut result: *mut AniDBFileResult = ptr::null_mut();
                    let status = anidb_process_file(
                        black_box(handle),
                        black_box(file_path.as_ptr()),
                        black_box(&options),
                        &mut result,
                    );

                    assert_eq!(status, AniDBResult::Success);
                    anidb_free_file_result(result);
                })
            },
        );

        // Multiple algorithms
        group.bench_with_input(
            BenchmarkId::new("multi_algorithm", name),
            &file_path,
            |b, file_path| {
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

                b.iter(|| {
                    let mut result: *mut AniDBFileResult = ptr::null_mut();
                    let status = anidb_process_file(
                        black_box(handle),
                        black_box(file_path.as_ptr()),
                        black_box(&options),
                        &mut result,
                    );

                    assert_eq!(status, AniDBResult::Success);
                    anidb_free_file_result(result);
                })
            },
        );
    }

    // Cleanup
    anidb_client_destroy(handle);

    group.finish();
}

/// Benchmark parallel FFI operations
fn benchmark_parallel_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_operations");

    // Test parallel client operations
    let thread_counts = vec![1, 2, 4, 8];

    for thread_count in thread_counts {
        group.bench_with_input(
            BenchmarkId::new("parallel_clients", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let handles = Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
                    let mut threads = Vec::new();

                    // Create clients in parallel
                    for _ in 0..thread_count {
                        let handles_clone = handles.clone();
                        let thread = thread::spawn(move || {
                            let mut handle: *mut std::ffi::c_void = ptr::null_mut();
                            let result = anidb_client_create(&mut handle);
                            assert_eq!(result, AniDBResult::Success);
                            handles_clone.lock().unwrap().push(handle as usize);
                        });
                        threads.push(thread);
                    }

                    for thread in threads {
                        thread.join().unwrap();
                    }

                    // Destroy all clients
                    for handle_usize in handles.lock().unwrap().iter() {
                        anidb_client_destroy(*handle_usize as *mut std::ffi::c_void);
                    }
                })
            },
        );

        // Test parallel file processing
        let test_file = create_test_file(1_048_576); // 1MB
        let file_path = test_file.path().to_str().unwrap().to_string();

        group.bench_with_input(
            BenchmarkId::new("parallel_processing", thread_count),
            &thread_count,
            |b, &thread_count| {
                // Create clients for each thread
                let mut clients = Vec::new();
                for _ in 0..thread_count {
                    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
                    anidb_client_create(&mut handle);
                    clients.push(handle);
                }

                b.iter(|| {
                    let barrier = Arc::new(std::sync::Barrier::new(thread_count));
                    let mut threads = Vec::new();

                    for (_i, &client_handle) in clients.iter().enumerate().take(thread_count) {
                        let path = file_path.clone();
                        let barrier_clone = barrier.clone();
                        let client_handle = client_handle as usize;

                        let thread = thread::spawn(move || {
                            barrier_clone.wait();

                            let client = client_handle as *mut std::ffi::c_void;
                            let c_path = CString::new(path).unwrap();
                            let algorithms = [AniDBHashAlgorithm::CRC32];
                            let options = AniDBProcessOptions {
                                algorithms: algorithms.as_ptr(),
                                algorithm_count: algorithms.len(),
                                enable_progress: 0,
                                progress_callback: None,
                                user_data: ptr::null_mut(),
                            };

                            let mut result: *mut AniDBFileResult = ptr::null_mut();
                            let status =
                                anidb_process_file(client, c_path.as_ptr(), &options, &mut result);

                            assert_eq!(status, AniDBResult::Success);
                            anidb_free_file_result(result);
                        });
                        threads.push(thread);
                    }

                    for thread in threads {
                        thread.join().unwrap();
                    }
                });

                // Cleanup clients
                for handle in clients {
                    anidb_client_destroy(handle);
                }
            },
        );
    }

    group.finish();
}

/// Benchmark memory pressure scenarios
fn benchmark_memory_pressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pressure");

    use anidb_client_core::ffi_memory::{AllocationType, ffi_allocate_buffer, ffi_release_buffer};

    // Benchmark allocation under memory pressure
    group.bench_function("allocation_under_pressure", |b| {
        // Pre-allocate to create pressure
        let mut pressure_buffers = Vec::new();
        for _ in 0..100 {
            if let Ok(buffer) = ffi_allocate_buffer(65536, AllocationType::Buffer) {
                pressure_buffers.push(buffer);
            }
        }

        b.iter(|| {
            // Try to allocate under pressure
            if let Ok(buffer) = ffi_allocate_buffer(4096, AllocationType::Buffer) {
                ffi_release_buffer(buffer);
            }
        });

        // Release pressure buffers
        for buffer in pressure_buffers {
            ffi_release_buffer(buffer);
        }
    });

    // Benchmark garbage collection
    group.bench_function("memory_gc", |b| {
        // Create some allocations
        let mut buffers = Vec::new();
        for _ in 0..50 {
            if let Ok(buffer) = ffi_allocate_buffer(16384, AllocationType::Buffer) {
                buffers.push(buffer);
            }
        }

        // Release half
        for _ in 0..25 {
            if let Some(buffer) = buffers.pop() {
                ffi_release_buffer(buffer);
            }
        }

        b.iter(|| {
            anidb_memory_gc();
        });

        // Cleanup
        for buffer in buffers {
            ffi_release_buffer(buffer);
        }
    });

    group.finish();
}

/// Benchmark result structure creation and cleanup
fn benchmark_result_structures(c: &mut Criterion) {
    let mut group = c.benchmark_group("result_structures");

    // Create client
    let mut handle: *mut std::ffi::c_void = ptr::null_mut();
    anidb_client_create(&mut handle);

    // Small file result
    let small_file = create_test_file(1024);
    let small_path = CString::new(small_file.path().to_str().unwrap()).unwrap();

    group.bench_function("small_result_lifecycle", |b| {
        let algorithms = [AniDBHashAlgorithm::CRC32];
        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        b.iter(|| {
            let mut result: *mut AniDBFileResult = ptr::null_mut();
            anidb_process_file(handle, small_path.as_ptr(), &options, &mut result);
            anidb_free_file_result(result);
        })
    });

    // Large result with multiple hashes
    let large_file = create_test_file(1_048_576);
    let large_path = CString::new(large_file.path().to_str().unwrap()).unwrap();

    group.bench_function("large_result_lifecycle", |b| {
        let algorithms = [
            AniDBHashAlgorithm::ED2K,
            AniDBHashAlgorithm::CRC32,
            AniDBHashAlgorithm::MD5,
            AniDBHashAlgorithm::SHA1,
            AniDBHashAlgorithm::TTH,
        ];
        let options = AniDBProcessOptions {
            algorithms: algorithms.as_ptr(),
            algorithm_count: algorithms.len(),
            enable_progress: 0,
            progress_callback: None,
            user_data: ptr::null_mut(),
        };

        b.iter(|| {
            let mut result: *mut AniDBFileResult = ptr::null_mut();
            anidb_process_file(handle, large_path.as_ptr(), &options, &mut result);
            anidb_free_file_result(result);
        })
    });

    // Cleanup
    anidb_client_destroy(handle);

    group.finish();
}

// Helper functions

fn create_test_file(size: usize) -> tempfile::NamedTempFile {
    use std::io::Write;

    let mut file = tempfile::NamedTempFile::new().unwrap();
    let data = vec![0u8; size];
    file.write_all(&data).unwrap();
    file.flush().unwrap();
    file
}

criterion_group!(
    benches,
    benchmark_ffi_overhead,
    benchmark_string_conversion,
    benchmark_client_lifecycle,
    benchmark_memory_allocation,
    benchmark_callbacks,
    benchmark_file_processing,
    benchmark_parallel_operations,
    benchmark_memory_pressure,
    benchmark_result_structures
);

criterion_main!(benches);
