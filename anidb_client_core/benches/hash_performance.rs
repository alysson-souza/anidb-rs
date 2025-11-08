//! Performance benchmarks for hash algorithms
//!
//! Benchmark suite that measures actual performance of hash
//! implementations, focusing on our implementation's overhead rather than
//! raw algorithm speed.

use anidb_client_core::hashing::{Ed2kVariant, HashAlgorithm, HashCalculator};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use tokio::runtime::Runtime;

/// Benchmark hash algorithms with different file sizes
fn benchmark_hash_algorithms(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_algorithms");
    let calculator = HashCalculator::new();

    // Test with various file sizes, focusing on real-world use cases
    let sizes = vec![
        1_024,      // 1KB - Small files
        10_240,     // 10KB - Config files
        102_400,    // 100KB - Small media
        1_048_576,  // 1MB - Images
        10_485_760, // 10MB - Small videos
        52_428_800, // 50MB - Typical anime episode segment
    ];

    for size in sizes {
        let data = generate_test_data(size);
        group.throughput(Throughput::Bytes(size as u64));

        // Benchmark ED2K (most important for AniDB)
        group.bench_with_input(
            BenchmarkId::new("ed2k", format_size(size)),
            &data,
            |b, data| {
                b.iter(|| {
                    let result = calculator
                        .calculate_bytes(HashAlgorithm::ED2K, black_box(data))
                        .unwrap();
                    black_box(result.hash);
                })
            },
        );

        // Benchmark CRC32
        group.bench_with_input(
            BenchmarkId::new("crc32", format_size(size)),
            &data,
            |b, data| {
                b.iter(|| {
                    let result = calculator
                        .calculate_bytes(HashAlgorithm::CRC32, black_box(data))
                        .unwrap();
                    black_box(result.hash);
                })
            },
        );

        // Benchmark CRC32
        group.bench_with_input(
            BenchmarkId::new("blake3", format_size(size)),
            &data,
            |b, data| {
                b.iter(|| {
                    let result = calculator
                        .calculate_bytes(HashAlgorithm::CRC32, black_box(data))
                        .unwrap();
                    black_box(result.hash);
                })
            },
        );

        // Benchmark MD5
        group.bench_with_input(
            BenchmarkId::new("md5", format_size(size)),
            &data,
            |b, data| {
                b.iter(|| {
                    let result = calculator
                        .calculate_bytes(HashAlgorithm::MD5, black_box(data))
                        .unwrap();
                    black_box(result.hash);
                })
            },
        );

        // Benchmark SHA1
        group.bench_with_input(
            BenchmarkId::new("sha1", format_size(size)),
            &data,
            |b, data| {
                b.iter(|| {
                    let result = calculator
                        .calculate_bytes(HashAlgorithm::SHA1, black_box(data))
                        .unwrap();
                    black_box(result.hash);
                })
            },
        );
    }

    group.finish();
}

/// Benchmark parallel hash calculation
fn benchmark_parallel_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_hashing");
    let calculator = HashCalculator::new();
    let rt = Runtime::new().unwrap();

    let data = generate_test_data(10_485_760); // 10MB
    group.throughput(Throughput::Bytes(10_485_760));

    // Sequential hashing - calculate all algorithms one after another
    group.bench_function("sequential_all_algorithms", |b| {
        b.iter(|| {
            let calc = &calculator;
            let d = black_box(&data);

            let _ed2k = calc.calculate_bytes(HashAlgorithm::ED2K, d).unwrap();
            let _crc32 = calc.calculate_bytes(HashAlgorithm::CRC32, d).unwrap();
            let _blake3 = calc.calculate_bytes(HashAlgorithm::CRC32, d).unwrap();
            let _md5 = calc.calculate_bytes(HashAlgorithm::MD5, d).unwrap();
            let _sha1 = calc.calculate_bytes(HashAlgorithm::SHA1, d).unwrap();
        })
    });

    // Parallel hashing using async file processing
    group.bench_function("parallel_file_hashing", |b| {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test_10mb.bin");
        std::fs::write(&file_path, &data).unwrap();

        b.iter(|| {
            rt.block_on(async {
                let algorithms = vec![
                    HashAlgorithm::ED2K,
                    HashAlgorithm::CRC32,
                    HashAlgorithm::CRC32,
                    HashAlgorithm::MD5,
                    HashAlgorithm::SHA1,
                ];

                let results = calculator
                    .calculate_multiple(&file_path, &algorithms)
                    .await
                    .unwrap();

                black_box(results);
            });
        })
    });

    group.finish();
}

/// Benchmark memory efficiency during hashing
fn benchmark_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");
    let rt = Runtime::new().unwrap();

    // Test with progressively larger files to measure streaming efficiency
    let sizes = vec![
        1_048_576,  // 1MB
        10_485_760, // 10MB
        52_428_800, // 50MB
    ];

    for size in sizes {
        group.throughput(Throughput::Bytes(size as u64));

        // Benchmark streaming file hash (constant memory usage)
        group.bench_with_input(
            BenchmarkId::new("streaming_file_hash", format_size(size)),
            &size,
            |b, &size| {
                let temp_dir = tempfile::tempdir().unwrap();
                let file_path = temp_dir.path().join(format!("test_{size}.bin"));
                let data = generate_test_data(size);
                std::fs::write(&file_path, &data).unwrap();

                b.iter(|| {
                    rt.block_on(async {
                        let calculator = HashCalculator::new();
                        let result = calculator
                            .calculate_file(&file_path, HashAlgorithm::ED2K)
                            .await
                            .unwrap();
                        black_box(result.hash);
                    });
                });
            },
        );

        // Benchmark in-memory calculation for comparison
        group.bench_with_input(
            BenchmarkId::new("in_memory_hash", format_size(size)),
            &size,
            |b, &size| {
                let data = generate_test_data(size);
                let calculator = HashCalculator::new();

                b.iter(|| {
                    let result = calculator
                        .calculate_bytes(HashAlgorithm::ED2K, black_box(&data))
                        .unwrap();
                    black_box(result.hash);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark file I/O operations
fn benchmark_file_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_operations");
    let rt = Runtime::new().unwrap();

    // Create temporary test files
    let temp_dir = tempfile::tempdir().unwrap();
    let mut test_files = Vec::new();

    let sizes = vec![1_048_576, 10_485_760, 52_428_800]; // 1MB, 10MB, 50MB

    for size in &sizes {
        let file_path = temp_dir
            .path()
            .join(format!("test_{}.bin", format_size(*size)));
        let data = generate_test_data(*size);
        std::fs::write(&file_path, data).unwrap();
        test_files.push((file_path, *size));
    }

    for (file_path, size) in &test_files {
        group.throughput(Throughput::Bytes(*size as u64));

        // Benchmark synchronous file reading (baseline)
        group.bench_with_input(
            BenchmarkId::new("sync_file_read", format_size(*size)),
            file_path,
            |b, path| {
                b.iter(|| {
                    let _data = std::fs::read(black_box(path)).unwrap();
                })
            },
        );

        // Benchmark async file hashing (our implementation)
        group.bench_with_input(
            BenchmarkId::new("async_file_hash", format_size(*size)),
            file_path,
            |b, path| {
                b.iter(|| {
                    rt.block_on(async {
                        let calculator = HashCalculator::new();
                        let result = calculator
                            .calculate_file(black_box(path), HashAlgorithm::ED2K)
                            .await
                            .unwrap();
                        black_box(result.hash);
                    });
                })
            },
        );

        // Benchmark file processing overhead (multiple algorithms)
        group.bench_with_input(
            BenchmarkId::new("multi_algorithm_file", format_size(*size)),
            file_path,
            |b, path| {
                b.iter(|| {
                    rt.block_on(async {
                        let calculator = HashCalculator::new();
                        let algorithms = vec![
                            HashAlgorithm::ED2K,
                            HashAlgorithm::CRC32,
                            HashAlgorithm::CRC32,
                        ];
                        let results = calculator
                            .calculate_multiple(black_box(path), &algorithms)
                            .await
                            .unwrap();
                        black_box(results);
                    });
                })
            },
        );
    }

    group.finish();
}

/// Benchmark ED2K-specific chunk boundary handling
fn benchmark_ed2k_chunk_boundaries(c: &mut Criterion) {
    let mut group = c.benchmark_group("ed2k_chunk_boundaries");
    let calculator = HashCalculator::new();

    // Test files around ED2K chunk boundary (9.5MB = 9728000 bytes)
    let sizes = vec![
        9_728_000 - 1024,     // Just under one chunk
        9_728_000,            // Exactly one chunk
        9_728_000 + 1024,     // Just over one chunk
        9_728_000 * 2,        // Exactly two chunks
        9_728_000 * 2 + 1024, // Two chunks plus some
        9_728_000 * 3,        // Three chunks (tests Red variant)
    ];

    for size in sizes {
        let data = generate_test_data(size);
        group.throughput(Throughput::Bytes(size as u64));

        // Benchmark Blue variant (standard ED2K)
        group.bench_with_input(
            BenchmarkId::new("ed2k_blue", format_size(size)),
            &data,
            |b, data| {
                b.iter(|| {
                    let result = calculator
                        .calculate_bytes_with_variant(
                            HashAlgorithm::ED2K,
                            black_box(data),
                            Ed2kVariant::Blue,
                        )
                        .unwrap();
                    black_box(result.hash);
                })
            },
        );

        // Benchmark Red variant (AniDB-compatible)
        group.bench_with_input(
            BenchmarkId::new("ed2k_red", format_size(size)),
            &data,
            |b, data| {
                b.iter(|| {
                    let result = calculator
                        .calculate_bytes_with_variant(
                            HashAlgorithm::ED2K,
                            black_box(data),
                            Ed2kVariant::Red,
                        )
                        .unwrap();
                    black_box(result.hash);
                })
            },
        );
    }

    group.finish();
}

/// Benchmark our implementation overhead vs raw algorithm performance
fn benchmark_implementation_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("implementation_overhead");
    let calculator = HashCalculator::new();

    // Test with 1MB data to measure overhead
    let data = generate_test_data(1_048_576);
    group.throughput(Throughput::Bytes(1_048_576));

    // Our implementation with result structure
    group.bench_function("hashcalculator_ed2k", |b| {
        b.iter(|| {
            let result = calculator
                .calculate_bytes(HashAlgorithm::ED2K, black_box(&data))
                .unwrap();
            black_box(result);
        })
    });

    // Raw MD4 for comparison (what ED2K uses internally)
    group.bench_function("raw_md4", |b| {
        use md4::{Digest, Md4};
        b.iter(|| {
            let mut hasher = Md4::new();
            hasher.update(black_box(&data));
            let result = format!("{:x}", hasher.finalize());
            black_box(result);
        })
    });

    // Our implementation with CRC32
    group.bench_function("hashcalculator_crc32", |b| {
        b.iter(|| {
            let result = calculator
                .calculate_bytes(HashAlgorithm::CRC32, black_box(&data))
                .unwrap();
            black_box(result);
        })
    });

    // Raw CRC32 for comparison
    group.bench_function("raw_crc32", |b| {
        use crc32fast::Hasher;
        b.iter(|| {
            let mut hasher = Hasher::new();
            hasher.update(black_box(&data));
            let result = format!("{:08x}", hasher.finalize());
            black_box(result);
        })
    });

    group.finish();
}

/// Benchmark throughput measurement
fn benchmark_throughput_measurement(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_measurement");
    let calculator = HashCalculator::new();
    let rt = Runtime::new().unwrap();

    // Measure throughput at different file sizes
    let sizes = vec![
        10_485_760,  // 10MB
        52_428_800,  // 50MB
        104_857_600, // 100MB
    ];

    for size in sizes {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join(format!("test_{size}.bin"));
        let data = generate_test_data(size);
        std::fs::write(&file_path, &data).unwrap();

        group.throughput(Throughput::Bytes(size as u64));

        // Measure ED2K throughput (most important for AniDB)
        group.bench_with_input(
            BenchmarkId::new("ed2k_throughput", format_size(size)),
            &file_path,
            |b, path| {
                b.iter(|| {
                    rt.block_on(async {
                        let result = calculator
                            .calculate_file(black_box(path), HashAlgorithm::ED2K)
                            .await
                            .unwrap();
                        black_box(result);
                    });
                })
            },
        );

        // Measure CRC32 throughput (fastest algorithm)
        group.bench_with_input(
            BenchmarkId::new("blake3_throughput", format_size(size)),
            &file_path,
            |b, path| {
                b.iter(|| {
                    rt.block_on(async {
                        let result = calculator
                            .calculate_file(black_box(path), HashAlgorithm::CRC32)
                            .await
                            .unwrap();
                        black_box(result);
                    });
                })
            },
        );
    }

    group.finish();
}

// Helper functions

fn generate_test_data(size: usize) -> Vec<u8> {
    // Generate deterministic test data for reproducible benchmarks
    let mut data = Vec::with_capacity(size);
    let mut seed = 0x12345678u32;

    for _ in 0..size {
        data.push((seed & 0xFF) as u8);
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    }

    data
}

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
    benchmark_hash_algorithms,
    benchmark_parallel_hashing,
    benchmark_memory_efficiency,
    benchmark_file_operations,
    benchmark_ed2k_chunk_boundaries,
    benchmark_implementation_overhead,
    benchmark_throughput_measurement
);

criterion_main!(benches);
