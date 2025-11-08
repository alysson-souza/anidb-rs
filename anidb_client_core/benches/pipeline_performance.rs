//! Performance benchmarks for the streaming pipeline architecture

use anidb_client_core::hashing::HashAlgorithm;
use anidb_client_core::pipeline::{
    HashingStage, PipelineConfig, StreamingPipelineBuilder, ValidationStage,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Create a test file with random data
fn create_test_file(size: usize) -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("bench.dat");

    // Create repeating pattern for consistent benchmarks
    let pattern = vec![0xAB; 8192];
    let mut data = Vec::with_capacity(size);
    while data.len() < size {
        let chunk_size = (size - data.len()).min(pattern.len());
        data.extend_from_slice(&pattern[..chunk_size]);
    }

    std::fs::write(&path, data).unwrap();
    (temp_dir, path)
}

fn bench_pipeline_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("pipeline_throughput");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10);

    // Test different file sizes for throughput
    let sizes = vec![
        (1024 * 1024, "1MB"),
        (10 * 1024 * 1024, "10MB"),
        (25 * 1024 * 1024, "25MB"),
    ];

    for (size, label) in sizes {
        let (_temp_dir, test_file) = create_test_file(size);

        group.throughput(Throughput::Bytes(size as u64));

        // Basic pipeline (no stages)
        group.bench_with_input(BenchmarkId::new("basic", label), &test_file, |b, path| {
            b.iter(|| {
                rt.block_on(async {
                    let mut pipeline = StreamingPipelineBuilder::new()
                        .chunk_size(64 * 1024)
                        .build();

                    pipeline.process_file(black_box(path)).await.unwrap()
                })
            });
        });

        // Pipeline with CRC32 hashing
        group.bench_with_input(BenchmarkId::new("crc32", label), &test_file, |b, path| {
            b.iter(|| {
                rt.block_on(async {
                    let hashing = Box::new(HashingStage::new(&[HashAlgorithm::CRC32]));

                    let mut pipeline = StreamingPipelineBuilder::new()
                        .chunk_size(64 * 1024)
                        .add_stage(hashing)
                        .build();

                    pipeline.process_file(black_box(path)).await.unwrap()
                })
            });
        });

        // Pipeline with multiple hashes
        group.bench_with_input(
            BenchmarkId::new("multi_hash", label),
            &test_file,
            |b, path| {
                b.iter(|| {
                    rt.block_on(async {
                        let hashing = Box::new(HashingStage::new(&[
                            HashAlgorithm::CRC32,
                            HashAlgorithm::MD5,
                            HashAlgorithm::SHA1,
                        ]));

                        let mut pipeline = StreamingPipelineBuilder::new()
                            .chunk_size(64 * 1024)
                            .add_stage(hashing)
                            .build();

                        pipeline.process_file(black_box(path)).await.unwrap()
                    })
                });
            },
        );

        // Pipeline with validation and hashing
        group.bench_with_input(
            BenchmarkId::new("validation_hash", label),
            &test_file,
            |b, path| {
                b.iter(|| {
                    rt.block_on(async {
                        let validation = Box::new(ValidationStage::new());
                        let hashing = Box::new(HashingStage::new(&[HashAlgorithm::CRC32]));

                        let mut pipeline = StreamingPipelineBuilder::new()
                            .chunk_size(64 * 1024)
                            .add_stage(validation)
                            .add_stage(hashing)
                            .build();

                        pipeline.process_file(black_box(path)).await.unwrap()
                    })
                });
            },
        );
    }

    group.finish();
}

fn bench_chunk_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("chunk_sizes");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10);

    let (_temp_dir, test_file) = create_test_file(10 * 1024 * 1024); // 10MB file

    let chunk_sizes = vec![
        (4 * 1024, "4KB"),
        (16 * 1024, "16KB"),
        (64 * 1024, "64KB"),
        (256 * 1024, "256KB"),
    ];

    group.throughput(Throughput::Bytes(10 * 1024 * 1024));

    for (chunk_size, label) in chunk_sizes {
        group.bench_with_input(BenchmarkId::new("chunk", label), &test_file, |b, path| {
            b.iter(|| {
                rt.block_on(async {
                    let hashing = Box::new(HashingStage::new(&[HashAlgorithm::CRC32]));

                    let mut pipeline = StreamingPipelineBuilder::new()
                        .chunk_size(chunk_size)
                        .add_stage(hashing)
                        .build();

                    pipeline.process_file(black_box(path)).await.unwrap()
                })
            });
        });
    }

    group.finish();
}

fn bench_memory_configs(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_configs");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10);

    let (_temp_dir, test_file) = create_test_file(25 * 1024 * 1024); // 25MB file

    // Test with different memory limits
    let configs = vec![
        (50 * 1024 * 1024, "50MB"),
        (100 * 1024 * 1024, "100MB"),
        (500 * 1024 * 1024, "500MB"),
    ];

    group.throughput(Throughput::Bytes(25 * 1024 * 1024));

    for (max_memory, label) in configs {
        group.bench_with_input(
            BenchmarkId::new("memory_limit", label),
            &test_file,
            |b, path| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = PipelineConfig {
                            chunk_size: 64 * 1024,
                            parallel_stages: false,
                            max_memory,
                        };

                        let hashing = Box::new(HashingStage::new(&[HashAlgorithm::ED2K]));

                        let mut pipeline = StreamingPipelineBuilder::with_config(config)
                            .add_stage(hashing)
                            .build();

                        pipeline.process_file(black_box(path)).await.unwrap()
                    })
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_pipeline_throughput,
    bench_chunk_sizes,
    bench_memory_configs
);
criterion_main!(benches);
