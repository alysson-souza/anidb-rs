//! Memory Performance Benchmarks
//!
//! These benchmarks verify that the buffer management system meets
//! the performance requirements.

use anidb_client_core::buffer::{
    DEFAULT_BUFFER_SIZE, allocate_buffer, get_memory_limit, memory_used, release_buffer,
};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

/// Benchmark buffer allocation and release operations
fn bench_buffer_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_operations");

    // Test different buffer sizes
    for &size in &[1024, 64 * 1024, 1024 * 1024, DEFAULT_BUFFER_SIZE] {
        group.bench_with_input(
            BenchmarkId::new("allocate_release", format_size(size)),
            &size,
            |b, &size| {
                b.iter(|| {
                    let buffer = allocate_buffer(size).unwrap();
                    black_box(&buffer);
                    release_buffer(buffer);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent buffer allocations
fn bench_concurrent_allocations(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("concurrent_allocations");

    for &num_threads in &[1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("concurrent", num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let mut handles = vec![];
                    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

                    for _ in 0..num_threads {
                        let barrier_clone = barrier.clone();
                        let handle = thread::spawn(move || {
                            barrier_clone.wait();

                            // Each thread allocates and releases a buffer
                            let buffer = allocate_buffer(64 * 1024).unwrap();
                            black_box(&buffer);
                            release_buffer(buffer);
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory tracking overhead
fn bench_memory_tracking_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_tracking");

    // Benchmark memory usage query
    group.bench_function("get_memory_used", |b| {
        b.iter(|| {
            black_box(memory_used());
        });
    });

    // Benchmark allocation with tracking vs without
    group.bench_function("with_tracking", |b| {
        b.iter(|| {
            let buffer = allocate_buffer(1024).unwrap();
            black_box(&buffer);
            release_buffer(buffer);
        });
    });

    // Benchmark raw allocation for comparison
    group.bench_function("raw_allocation", |b| {
        b.iter(|| {
            let buffer = vec![0u8; 1024];
            black_box(&buffer);
            drop(buffer);
        });
    });

    group.finish();
}

/// Benchmark memory limit enforcement
fn bench_memory_limit_enforcement(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_limit");

    group.bench_function("check_limit", |b| {
        b.iter(|| {
            // Try to allocate just under the limit
            let size = get_memory_limit() - 1000;
            match allocate_buffer(size) {
                Ok(buffer) => {
                    black_box(&buffer);
                    release_buffer(buffer);
                }
                Err(_) => {
                    // This should not happen in normal circumstances
                }
            }
        });
    });

    group.finish();
}

// Helper function
fn format_size(size: usize) -> String {
    if size >= 1_073_741_824 {
        format!("{}GB", size / 1_073_741_824)
    } else if size >= 1_048_576 {
        format!("{}MB", size / 1_048_576)
    } else if size >= 1_024 {
        format!("{}KB", size / 1_024)
    } else {
        format!("{size}B")
    }
}

criterion_group!(
    benches,
    bench_buffer_operations,
    bench_concurrent_allocations,
    bench_memory_tracking_overhead,
    bench_memory_limit_enforcement
);

criterion_main!(benches);
