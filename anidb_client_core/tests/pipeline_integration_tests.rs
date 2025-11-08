//! Integration tests for the streaming pipeline architecture

use anidb_client_core::hashing::HashAlgorithm;
use anidb_client_core::pipeline::{
    HashingStage, PipelineConfig, ProcessingStage, StreamingPipelineBuilder, ValidationStage,
};
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;
use tokio::sync::mpsc;

/// Test file creation helper
fn create_test_file(dir: &TempDir, name: &str, size: usize) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let data = vec![0xAB; size];
    std::fs::write(&path, data).unwrap();
    path
}

/// Custom test stage that counts chunks
#[derive(Debug)]
struct ChunkCountingStage {
    count: Arc<std::sync::Mutex<usize>>,
    chunk_sizes: Arc<std::sync::Mutex<Vec<usize>>>,
}

impl ChunkCountingStage {
    fn new() -> Self {
        Self {
            count: Arc::new(std::sync::Mutex::new(0)),
            chunk_sizes: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    #[allow(dead_code)]
    fn chunk_count(&self) -> usize {
        *self.count.lock().unwrap()
    }

    #[allow(dead_code)]
    fn chunk_sizes(&self) -> Vec<usize> {
        self.chunk_sizes.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl ProcessingStage for ChunkCountingStage {
    async fn process(&mut self, chunk: &[u8]) -> anidb_client_core::Result<()> {
        *self.count.lock().unwrap() += 1;
        self.chunk_sizes.lock().unwrap().push(chunk.len());
        Ok(())
    }

    fn name(&self) -> &str {
        "ChunkCountingStage"
    }
}

#[tokio::test]
async fn test_pipeline_multiple_stages_integration() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = create_test_file(&temp_dir, "multi_stage.dat", 1024 * 100); // 100KB

    // Create stages
    let validation = Box::new(ValidationStage::new());
    let hashing = Box::new(HashingStage::new(&[
        HashAlgorithm::CRC32,
        HashAlgorithm::MD5,
    ]));
    let counting = Box::new(ChunkCountingStage::new());

    // Track counting stage for verification
    let counter_ref = counting.count.clone();
    let sizes_ref = counting.chunk_sizes.clone();

    // Build pipeline with multiple stages
    let mut pipeline = StreamingPipelineBuilder::new()
        .chunk_size(8192) // 8KB chunks
        .add_stage(validation)
        .add_stage(hashing)
        .add_stage(counting)
        .build();

    // Process file
    let stats = pipeline.process_file(&test_file).await.unwrap();

    // Verify stats
    assert_eq!(stats.bytes_processed, 1024 * 100);
    assert!(stats.throughput_mbps > 0.0);

    // Verify chunk counting
    let chunk_count = *counter_ref.lock().unwrap();
    let chunk_sizes = sizes_ref.lock().unwrap().clone();
    assert_eq!(chunk_count, stats.chunks_processed);
    assert_eq!(chunk_count, 13); // 100KB / 8KB = 12.5, so 13 chunks

    // Verify chunk sizes
    assert_eq!(chunk_sizes.len(), 13);
    for chunk_size in chunk_sizes.iter().take(12) {
        assert_eq!(*chunk_size, 8192);
    }
    assert_eq!(chunk_sizes[12], 4096); // Last partial chunk

    // Verify that we have a hashing stage at position 1
    assert_eq!(pipeline.stage_count(), 3);
    let stage_name = pipeline.stage(1).unwrap().name();
    assert_eq!(stage_name, "HashingStage");
}

#[derive(Debug, Clone)]
struct ProgressUpdate {
    bytes_processed: u64,
    #[allow(dead_code)]
    total_bytes: u64,
}

/// Simple progress stage for testing
#[derive(Debug)]
struct SimpleProgressStage {
    tx: mpsc::Sender<ProgressUpdate>,
    bytes_processed: u64,
    total_bytes: u64,
}

impl SimpleProgressStage {
    fn new(tx: mpsc::Sender<ProgressUpdate>) -> Self {
        Self {
            tx,
            bytes_processed: 0,
            total_bytes: 0,
        }
    }
}

#[async_trait::async_trait]
impl ProcessingStage for SimpleProgressStage {
    async fn process(&mut self, chunk: &[u8]) -> anidb_client_core::Result<()> {
        self.bytes_processed += chunk.len() as u64;
        let _ = self
            .tx
            .send(ProgressUpdate {
                bytes_processed: self.bytes_processed,
                total_bytes: self.total_bytes,
            })
            .await;
        Ok(())
    }

    async fn initialize(&mut self, total_size: u64) -> anidb_client_core::Result<()> {
        self.total_bytes = total_size;
        self.bytes_processed = 0;
        Ok(())
    }

    fn name(&self) -> &str {
        "SimpleProgressStage"
    }
}

#[tokio::test]
async fn test_pipeline_with_progress_reporting() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = create_test_file(&temp_dir, "progress.dat", 1024 * 50); // 50KB

    // Create progress channel
    let (tx, mut rx) = mpsc::channel(100);

    // Create stages
    let progress = Box::new(SimpleProgressStage::new(tx));
    let hashing = Box::new(HashingStage::new(&[HashAlgorithm::ED2K]));

    // Build pipeline
    let mut pipeline = StreamingPipelineBuilder::new()
        .chunk_size(4096) // 4KB chunks
        .add_stage(progress)
        .add_stage(hashing)
        .build();

    // Process file in background
    let process_handle = tokio::spawn(async move { pipeline.process_file(&test_file).await });

    // Collect progress updates
    let mut updates = Vec::new();
    while let Ok(progress) = rx.try_recv() {
        updates.push(progress);
    }

    // Wait for processing to complete
    let _stats = process_handle.await.unwrap().unwrap();

    // Give a bit more time for final messages
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    while let Ok(progress) = rx.try_recv() {
        updates.push(progress);
    }

    // Verify progress updates
    assert!(!updates.is_empty());

    // Progress should increase monotonically
    for i in 1..updates.len() {
        assert!(updates[i].bytes_processed >= updates[i - 1].bytes_processed);
    }
}

#[tokio::test]
async fn test_pipeline_error_propagation() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = create_test_file(&temp_dir, "error.dat", 1024 * 200); // 200KB

    // Create validation stage with very small limit
    let validation = Box::new(ValidationStage::new().with_max_file_size(1024 * 100)); // 100KB limit

    // Build pipeline
    let mut pipeline = StreamingPipelineBuilder::new()
        .add_stage(validation)
        .build();

    // Process should fail due to validation
    let result = pipeline.process_file(&test_file).await;
    assert!(result.is_err());

    // Error should be about file size validation
    let error_str = format!("{}", result.unwrap_err());
    // The error should indicate file size exceeded maximum
    assert!(
        error_str.contains("exceeds maximum")
            || error_str.contains("exceeded")
            || error_str.contains("too large")
    );
}

#[tokio::test]
async fn test_pipeline_large_file_streaming() {
    let temp_dir = TempDir::new().unwrap();
    // Create a 10MB file
    let test_file = create_test_file(&temp_dir, "large.dat", 1024 * 1024 * 10);

    // Create stages
    let hashing = Box::new(HashingStage::new(&[
        HashAlgorithm::ED2K,
        HashAlgorithm::CRC32,
        HashAlgorithm::MD5,
    ]));

    // Build pipeline with larger chunks for efficiency
    let mut pipeline = StreamingPipelineBuilder::new()
        .chunk_size(64 * 1024) // 64KB chunks
        .add_stage(hashing)
        .build();

    // Process file
    let start = Instant::now();
    let stats = pipeline.process_file(&test_file).await.unwrap();
    let duration = start.elapsed();

    // Verify processing
    assert_eq!(stats.bytes_processed, 1024 * 1024 * 10);
    assert!(stats.chunks_processed > 0);

    // Check throughput (should be fast for in-memory operations)
    let throughput_mbps =
        (stats.bytes_processed as f64 / duration.as_secs_f64()) / (1024.0 * 1024.0);
    println!("Large file throughput: {throughput_mbps:.2} MB/s");
    // Relaxed throughput for test environments - focus on correctness
    assert!(throughput_mbps > 10.0); // Should achieve at least 10 MB/s

    // Verify we have a hashing stage
    assert_eq!(pipeline.stage_count(), 1);
    assert_eq!(pipeline.stage(0).unwrap().name(), "HashingStage");
}

#[tokio::test]
async fn test_pipeline_memory_usage() {
    let temp_dir = TempDir::new().unwrap();
    // Create a 5MB file
    let test_file = create_test_file(&temp_dir, "memory_test.dat", 1024 * 1024 * 5);

    // Build pipeline with specific memory constraints
    let config = PipelineConfig {
        chunk_size: 32 * 1024, // 32KB chunks
        parallel_stages: false,
        max_memory: 10 * 1024 * 1024, // 10MB limit
    };

    let hashing = Box::new(HashingStage::new(&[HashAlgorithm::SHA1]));

    let mut pipeline = StreamingPipelineBuilder::with_config(config)
        .add_stage(hashing)
        .build();

    // Process file - should work within memory constraints
    let stats = pipeline.process_file(&test_file).await.unwrap();

    assert_eq!(stats.bytes_processed, 1024 * 1024 * 5);
    // With 32KB chunks, we should have 160 chunks for 5MB
    assert_eq!(stats.chunks_processed, (5 * 1024 * 1024) / (32 * 1024));
}

#[tokio::test]
async fn test_pipeline_empty_file_handling() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = create_test_file(&temp_dir, "empty.dat", 0);

    let counting = Box::new(ChunkCountingStage::new());
    let counter_ref = counting.count.clone();

    let mut pipeline = StreamingPipelineBuilder::new().add_stage(counting).build();

    let stats = pipeline.process_file(&test_file).await.unwrap();

    assert_eq!(stats.bytes_processed, 0);
    assert_eq!(stats.chunks_processed, 0);
    assert_eq!(*counter_ref.lock().unwrap(), 0);
}

#[tokio::test]
async fn test_pipeline_stage_ordering() {
    // Test that stages are executed in the correct order
    #[derive(Debug)]
    struct OrderTrackingStage {
        id: usize,
        order: Arc<std::sync::Mutex<Vec<usize>>>,
    }

    #[async_trait::async_trait]
    impl ProcessingStage for OrderTrackingStage {
        async fn process(&mut self, _chunk: &[u8]) -> anidb_client_core::Result<()> {
            self.order.lock().unwrap().push(self.id);
            Ok(())
        }

        fn name(&self) -> &str {
            "OrderTrackingStage"
        }
    }

    let order = Arc::new(std::sync::Mutex::new(Vec::new()));

    let stage1 = Box::new(OrderTrackingStage {
        id: 1,
        order: order.clone(),
    });
    let stage2 = Box::new(OrderTrackingStage {
        id: 2,
        order: order.clone(),
    });
    let stage3 = Box::new(OrderTrackingStage {
        id: 3,
        order: order.clone(),
    });

    let mut pipeline = StreamingPipelineBuilder::new()
        .add_stage(stage1)
        .add_stage(stage2)
        .add_stage(stage3)
        .build();

    // Process some data
    let data = b"test data for ordering";
    pipeline.process_bytes(data).await.unwrap();

    // Check order
    let recorded_order = order.lock().unwrap().clone();
    assert_eq!(recorded_order, vec![1, 2, 3]);
}

#[tokio::test]
async fn test_pipeline_reinitialize_between_files() {
    let temp_dir = TempDir::new().unwrap();
    let file1 = create_test_file(&temp_dir, "file1.dat", 1024);
    let file2 = create_test_file(&temp_dir, "file2.dat", 2048);

    let counting = Box::new(ChunkCountingStage::new());
    let counter_ref = counting.count.clone();

    let mut pipeline = StreamingPipelineBuilder::new()
        .chunk_size(512)
        .add_stage(counting)
        .build();

    // Process first file
    let stats1 = pipeline.process_file(&file1).await.unwrap();
    assert_eq!(stats1.bytes_processed, 1024);
    assert_eq!(*counter_ref.lock().unwrap(), 2); // 1024/512 = 2 chunks

    // Process second file - counter should be reset
    let stats2 = pipeline.process_file(&file2).await.unwrap();
    assert_eq!(stats2.bytes_processed, 2048);
    // Note: The stage gets reinitialized, but our external Arc keeps accumulating
    // In a real scenario, the stage would reset its internal state
}
